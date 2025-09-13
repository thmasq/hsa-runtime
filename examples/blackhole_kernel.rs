use hsa::KernelDispatch;
use hsa::{Executable, HsaError};
use hsa::{HsaContext, Result, Signal};
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
    // Initialize HSA context
    let ctx = HsaContext::new()?;
    println!("HSA Context initialized");
    println!("GPU Agent found: {:?}", ctx.agent.device_type()?);

    // Load kernel binary
    let kernel_path = Path::new("libblackhole-kernel.so.o");
    let kernel_binary = fs::read(kernel_path).expect("Failed to read kernel file");
    println!("Loaded kernel binary: {} bytes", kernel_binary.len());

    // Create executable and load code object
    let mut executable = Executable::create()?;
    executable.load_code_object(&ctx.agent, &kernel_binary)?;
    executable.freeze()?;
    println!("Executable created and frozen");

    // Get kernel symbol - try different naming conventions
    let kernel_symbol = executable
        .get_kernel_symbol("trace_geodesics.kd", &ctx.agent)
        .or_else(|_| executable.get_kernel_symbol("trace_geodesics", &ctx.agent))
        .or_else(|_| {
            executable.get_kernel_symbol("__device_kernel__trace_geodesics", &ctx.agent)
        })?;

    let kernel_object = kernel_symbol.kernel_object()?;
    println!("Kernel object handle: 0x{:x}", kernel_object);

    // Set up dimensions
    let width = 1920u32;
    let height = 1080u32;
    let max_steps = 1000u32;
    let output_size = (width * height * 4) as usize; // RGBA output

    // Allocate memory buffers
    let coarse_region = ctx
        .coarse_grained_region
        .ok_or(HsaError::MemoryRegionNotFound)?;

    let output_buffer = coarse_region.allocate(output_size)?;
    output_buffer.allow_access(&[ctx.agent])?;
    println!("Allocated output buffer: {} bytes", output_size);

    let camera_buffer = coarse_region.allocate(std::mem::size_of::<CameraData>())?;
    camera_buffer.allow_access(&[ctx.agent])?;

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

    // Allocate kernargs
    let kernarg_region = ctx
        .kernarg_region
        .or(ctx.fine_grained_region)
        .ok_or(HsaError::MemoryRegionNotFound)?;

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

    // Create completion signal
    let completion_signal = Signal::create(1)?;

    // Set up dispatch parameters
    let workgroup_size = 16u32;
    let grid_x = ((width + workgroup_size - 1) / workgroup_size) * workgroup_size;
    let grid_y = ((height + workgroup_size - 1) / workgroup_size) * workgroup_size;

    let dispatch = KernelDispatch {
        kernel_object,
        kernarg_address: kernargs_buffer.as_ptr(),
        workgroup_size: (workgroup_size as u16, workgroup_size as u16, 1),
        grid_size: (grid_x, grid_y, 1),
        private_segment_size: 0,
        group_segment_size: 2048,
    };

    // Get queue
    let queue = ctx.queue.as_ref().ok_or(HsaError::QueueCreationFailed)?;

    println!("Dispatching kernel...");
    println!("  Grid size: {}x{}", grid_x, grid_y);
    println!("  Workgroup size: {}x{}", workgroup_size, workgroup_size);

    // Dispatch kernel
    dispatch.dispatch(queue, &completion_signal)?;

    // Wait for completion
    let wait_result = completion_signal.wait_eq(0, u64::MAX);
    if wait_result != 0 {
        return Err(HsaError::ExecutionFailed);
    }

    println!("Kernel execution completed!");

    // Read first few pixels to verify output
    let output_slice = output_buffer.as_slice();
    let pixels = unsafe { std::slice::from_raw_parts(output_slice.as_ptr() as *const u32, 10) };

    println!("First 10 pixels (RGBA as u32):");
    for (i, pixel) in pixels.iter().enumerate() {
        let r = (pixel >> 24) & 0xFF;
        let g = (pixel >> 16) & 0xFF;
        let b = (pixel >> 8) & 0xFF;
        let a = pixel & 0xFF;
        println!("  Pixel {}: R={}, G={}, B={}, A={}", i, r, g, b, a);
    }

    use std::io::Write;
    let mut file = fs::File::create("blackhole_output.raw").unwrap();
    let _ = file.write_all(output_slice);
    println!("Output saved to blackhole_output.raw");

    Ok(())
}
