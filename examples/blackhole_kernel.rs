use hsa::KernelDispatch;
use hsa::error::{log_debug, log_error, log_info};
use hsa::{Executable, HsaError};
use hsa::{HsaContext, Result, Signal};
use std::env;
use std::fs;
use std::path::Path;

// Camera data structure matching the kernel's expected format
#[repr(C)]
#[derive(Debug, Clone)]
struct CameraData {
    pos: [f32; 4],     // position + padding
    right: [f32; 4],   // right vector + padding
    up: [f32; 4],      // up vector + padding
    forward: [f32; 4], // forward vector + padding
    tan_half_fov: f32,
    aspect: f32,
    moving: u32,
    _pad: u32,
}

// Disk parameters for accretion disk
#[repr(C)]
#[derive(Debug, Clone)]
struct DiskData {
    r1: f32,        // Inner radius
    r2: f32,        // Outer radius
    num: f32,       // Number parameter
    thickness: f32, // Disk thickness
}

// Kernel arguments structure
#[repr(C)]
struct KernelArgs {
    output_ptr: u64,
    camera_ptr: u64,
    disk_ptr: u64,
    width: u32,
    height: u32,
    max_steps: u32,
    _padding: u32,
}

fn main() -> Result<()> {
    // Enable debug logging if requested
    if env::var("HSA_DEBUG").is_ok() {
        log_info("Debug logging enabled (HSA_DEBUG=1)");
    }

    log_info("Starting blackhole kernel example");

    // Initialize HSA context
    log_info("Initializing HSA context...");
    let ctx = match HsaContext::new() {
        Ok(ctx) => {
            log_info("HSA Context initialized successfully");
            ctx
        }
        Err(e) => {
            log_error(&format!("Failed to initialize HSA context: {}", e));
            return Err(e);
        }
    };

    let device_type = ctx.agent.device_type()?;
    log_info(&format!("GPU Agent found: {:?}", device_type));

    // Check available memory regions
    let regions = ctx.agent.iterate_memory_regions()?;
    log_debug(&format!("Found {} memory regions", regions.len()));

    for (i, region) in regions.iter().enumerate() {
        let segment = region.segment()?;
        log_debug(&format!("  Region {}: segment = 0x{:x}", i, segment));
    }

    // Load kernel binary
    let kernel_path = Path::new("libblackhole-kernel.so.o");

    if !kernel_path.exists() {
        let error_msg = format!("Kernel file not found: {}", kernel_path.display());
        log_error(&error_msg);
        return Err(HsaError::InvalidArgument(error_msg));
    }

    let kernel_binary = match fs::read(kernel_path) {
        Ok(data) => {
            log_info(&format!("Loaded kernel binary: {} bytes", data.len()));

            // Log first few bytes for debugging
            if data.len() >= 16 {
                let preview: Vec<String> =
                    data[..16].iter().map(|b| format!("{:02x}", b)).collect();
                log_debug(&format!("Kernel binary header: {}", preview.join(" ")));
            }

            data
        }
        Err(e) => {
            log_error(&format!("Failed to read kernel file: {}", e));
            return Err(HsaError::InvalidArgument(format!(
                "Cannot read kernel file: {}",
                e
            )));
        }
    };

    // Create executable and load code object
    log_info("Creating and configuring executable...");
    let mut executable = Executable::create()?;

    log_info("Loading code object into executable...");
    if let Err(e) = executable.load_code_object(&ctx.agent, &kernel_binary) {
        log_error("Code object loading failed, attempting to diagnose...");

        // Try to provide more diagnostic information
        let agent_device_type = ctx.agent.device_type()?;
        log_error(&format!("Agent device type: {:?}", agent_device_type));

        // Return the detailed error
        return Err(e);
    }

    log_info("Freezing executable...");
    executable.freeze()?;
    log_info("Executable created and frozen successfully");

    // List all available symbols for debugging
    log_info("Discovering available kernel symbols...");
    match executable.list_symbols(&ctx.agent) {
        Ok(symbols) => {
            if symbols.is_empty() {
                log_error("No symbols found in executable!");
                return Err(HsaError::KernelNotFound(
                    "No symbols found in code object".to_string(),
                ));
            } else {
                log_info(&format!("Found {} symbols:", symbols.len()));
                for (i, symbol) in symbols.iter().enumerate() {
                    log_info(&format!("  {}: {}", i, symbol));
                }
            }
        }
        Err(e) => {
            log_error(&format!("Failed to list symbols: {}", e));
        }
    }

    // Try to find kernel symbol with various naming patterns
    let kernel_names = [
        "trace_geodesics.kd",
        "trace_geodesics",
        "__device_kernel__trace_geodesics",
        "_Z16trace_geodesicsPvS_S_jjj", // Mangled name pattern
        "blackhole_kernel",
    ];

    let mut kernel_symbol = None;
    for name in &kernel_names {
        log_debug(&format!("Trying kernel name: '{}'", name));
        match executable.get_kernel_symbol(name, &ctx.agent) {
            Ok(symbol) => {
                log_info(&format!("Found kernel symbol: '{}'", name));
                kernel_symbol = Some(symbol);
                break;
            }
            Err(e) => {
                log_debug(&format!("Kernel name '{}' not found: {}", name, e));
            }
        }
    }

    let kernel_symbol = kernel_symbol.ok_or_else(|| {
        HsaError::KernelNotFound(format!(
            "Could not find kernel with any of these names: {:?}\nCheck the symbols listed above.",
            kernel_names
        ))
    })?;

    let kernel_object = kernel_symbol.kernel_object()?;
    log_info(&format!("Kernel object handle: 0x{:x}", kernel_object));

    // Get kernel requirements
    let kernarg_size = kernel_symbol.get_kernarg_segment_size()?;
    let group_size = kernel_symbol.get_group_segment_size()?;
    let private_size = kernel_symbol.get_private_segment_size()?;

    log_info(&format!("Kernel requirements:"));
    log_info(&format!("  Kernarg segment size: {} bytes", kernarg_size));
    log_info(&format!("  Group segment size: {} bytes", group_size));
    log_info(&format!("  Private segment size: {} bytes", private_size));

    // Set up dimensions
    let width = 1920u32;
    let height = 1080u32;
    let max_steps = 1000u32;
    let output_size = (width * height * 4) as usize; // RGBA output

    log_info(&format!("Image dimensions: {}x{}", width, height));
    log_info(&format!("Output buffer size: {} bytes", output_size));

    // Allocate memory buffers
    let coarse_region = ctx
        .coarse_grained_region
        .ok_or(HsaError::MemoryRegionNotFound)?;

    log_debug("Allocating output buffer...");
    let output_buffer = coarse_region.allocate(output_size)?;
    output_buffer.allow_access(&[ctx.agent])?;
    log_debug(&format!(
        "Output buffer allocated at: {:p}",
        output_buffer.as_ptr()
    ));

    log_debug("Allocating camera buffer...");
    let camera_buffer = coarse_region.allocate(std::mem::size_of::<CameraData>())?;
    camera_buffer.allow_access(&[ctx.agent])?;

    log_debug("Allocating disk buffer...");
    let disk_buffer = coarse_region.allocate(std::mem::size_of::<DiskData>())?;
    disk_buffer.allow_access(&[ctx.agent])?;

    // Initialize camera
    let camera = CameraData {
        pos: [0.0, 0.0, -50.0, 0.0],
        right: [1.0, 0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0, 0.0],
        forward: [0.0, 0.0, 1.0, 0.0],
        tan_half_fov: 0.5773503, // tan(30 degrees)
        aspect: width as f32 / height as f32,
        moving: 0,
        _pad: 0,
    };

    unsafe {
        let camera_ptr = camera_buffer.as_ptr() as *mut CameraData;
        *camera_ptr = camera;
    }
    log_debug("Camera data initialized");

    // Initialize disk parameters (Sagittarius A* scale)
    let saga_rs = 1.269e10_f32; // Schwarzschild radius
    let disk = DiskData {
        r1: saga_rs * 2.2, // Inner radius
        r2: saga_rs * 5.2, // Outer radius
        num: 2.0,
        thickness: 1e9,
    };

    unsafe {
        let disk_ptr = disk_buffer.as_ptr() as *mut DiskData;
        *disk_ptr = disk;
    }
    log_debug("Disk parameters initialized");

    // Allocate kernargs
    let kernarg_region = ctx
        .kernarg_region
        .or(ctx.fine_grained_region)
        .ok_or(HsaError::MemoryRegionNotFound)?;

    let expected_kernarg_size = std::mem::size_of::<KernelArgs>();
    log_debug(&format!(
        "Expected kernarg size: {} bytes",
        expected_kernarg_size
    ));

    if kernarg_size != 0 && kernarg_size as usize != expected_kernarg_size {
        log_error(&format!(
            "Kernarg size mismatch! Expected: {}, Kernel requires: {}",
            expected_kernarg_size, kernarg_size
        ));
    }

    let kernargs_buffer = kernarg_region.allocate(std::mem::size_of::<KernelArgs>())?;

    // Set up kernel arguments
    let kernargs = KernelArgs {
        output_ptr: output_buffer.as_ptr() as u64,
        camera_ptr: camera_buffer.as_ptr() as u64,
        disk_ptr: disk_buffer.as_ptr() as u64,
        width,
        height,
        max_steps,
        _padding: 0,
    };

    unsafe {
        let kernargs_ptr = kernargs_buffer.as_ptr() as *mut KernelArgs;
        *kernargs_ptr = kernargs;
    }
    log_debug(&format!(
        "Kernel arguments configured at: {:p}",
        kernargs_buffer.as_ptr()
    ));

    // Create completion signal
    let completion_signal = Signal::create(1)?;
    log_debug("Completion signal created");

    // Set up dispatch parameters
    let workgroup_size = 16u32;
    let grid_x = ((width + workgroup_size - 1) / workgroup_size) * workgroup_size;
    let grid_y = ((height + workgroup_size - 1) / workgroup_size) * workgroup_size;

    log_info(&format!("Dispatch configuration:"));
    log_info(&format!("  Grid size: {}x{}", grid_x, grid_y));
    log_info(&format!(
        "  Workgroup size: {}x{}",
        workgroup_size, workgroup_size
    ));
    log_info(&format!("  Total work items: {}", grid_x * grid_y));

    let dispatch = KernelDispatch {
        kernel_object,
        kernarg_address: kernargs_buffer.as_ptr(),
        workgroup_size: (workgroup_size as u16, workgroup_size as u16, 1),
        grid_size: (grid_x, grid_y, 1),
        private_segment_size: private_size,
        group_segment_size: group_size.max(2048), // Use kernel requirement or minimum
    };

    // Get queue
    let queue = ctx.queue.as_ref().ok_or(HsaError::QueueCreationFailed(
        "No queue available".to_string(),
    ))?;

    log_info("Dispatching kernel...");

    // Dispatch kernel
    dispatch.dispatch(queue, &completion_signal)?;

    log_info("Waiting for kernel completion...");

    // Wait for completion with timeout
    let start_time = std::time::Instant::now();
    let wait_result = completion_signal.wait_eq(0, u64::MAX);
    let elapsed = start_time.elapsed();

    if wait_result != 0 {
        log_error(&format!(
            "Kernel execution failed or timed out (signal value: {})",
            wait_result
        ));
        return Err(HsaError::ExecutionFailed(format!(
            "Signal wait failed: {}",
            wait_result
        )));
    }

    log_info(&format!(
        "Kernel execution completed in {:.2}ms!",
        elapsed.as_millis()
    ));

    // Read first few pixels to verify output
    let output_slice = output_buffer.as_slice();
    let pixels = unsafe { std::slice::from_raw_parts(output_slice.as_ptr() as *const u32, 10) };

    log_info("Output verification - First 10 pixels (RGBA as u32):");
    let mut non_zero_count = 0;
    for (i, pixel) in pixels.iter().enumerate() {
        let r = (pixel >> 24) & 0xFF;
        let g = (pixel >> 16) & 0xFF;
        let b = (pixel >> 8) & 0xFF;
        let a = pixel & 0xFF;
        log_info(&format!(
            "  Pixel {}: R={}, G={}, B={}, A={}",
            i, r, g, b, a
        ));

        if *pixel != 0 {
            non_zero_count += 1;
        }
    }

    if non_zero_count == 0 {
        log_error("Warning: All sampled pixels are zero - kernel may not have executed properly");
    } else {
        log_info(&format!(
            "Output contains data ({}/{} non-zero pixels in sample)",
            non_zero_count,
            pixels.len()
        ));
    }

    // Save output
    use std::io::Write;
    match fs::File::create("blackhole_output.raw") {
        Ok(mut file) => {
            if let Err(e) = file.write_all(output_slice) {
                log_error(&format!("Failed to write output file: {}", e));
            } else {
                log_info("Output saved to blackhole_output.raw");
            }
        }
        Err(e) => {
            log_error(&format!("Failed to create output file: {}", e));
        }
    }

    log_info("Blackhole kernel example completed successfully!");
    Ok(())
}
