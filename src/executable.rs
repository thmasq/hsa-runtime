use crate::Queue;
use crate::bindings;
use crate::error::{log_debug, log_error, log_info};
use crate::{Agent, HsaError, Result, Signal};
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;

pub struct Executable {
    handle: bindings::hsa_executable_t,
    code_object_reader: Option<bindings::hsa_code_object_reader_t>,
}

impl Executable {
    pub fn create() -> Result<Self> {
        log_debug("Creating HSA executable");

        let mut executable = bindings::hsa_executable_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_executable_create_alt(
                bindings::hsa_profile_t_HSA_PROFILE_FULL,
                bindings::hsa_default_float_rounding_mode_t_HSA_DEFAULT_FLOAT_ROUNDING_MODE_NEAR,
                ptr::null(),
                &mut executable,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to create executable");
                log_error(&format!("Executable creation failed: {}", error));
                return Err(error);
            }
        }

        log_debug(&format!(
            "Created executable with handle: 0x{:x}",
            executable.handle
        ));

        Ok(Executable {
            handle: executable,
            code_object_reader: None,
        })
    }

    pub fn load_code_object(&mut self, agent: &Agent, code_object: &[u8]) -> Result<()> {
        log_info(&format!(
            "Loading code object ({} bytes) for agent 0x{:x}",
            code_object.len(),
            agent.handle.handle
        ));

        // Validate input
        if code_object.is_empty() {
            return Err(HsaError::InvalidArgument(
                "Code object is empty".to_string(),
            ));
        }

        // Create code object reader from memory
        let mut reader = bindings::hsa_code_object_reader_t { handle: 0 };

        unsafe {
            log_debug("Creating code object reader from memory");
            let status = bindings::hsa_code_object_reader_create_from_memory(
                code_object.as_ptr() as *const c_void,
                code_object.len(),
                &mut reader,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    "Failed to create code object reader from memory",
                );
                log_error(&format!("Code object reader creation failed: {}", error));
                return Err(HsaError::CodeObjectReaderFailed(error.to_string()));
            }

            log_debug(&format!(
                "Created code object reader with handle: 0x{:x}",
                reader.handle
            ));

            // Load agent code object
            log_debug("Loading agent code object into executable");
            let mut loaded_code_object = bindings::hsa_loaded_code_object_t { handle: 0 };

            let load_status = bindings::hsa_executable_load_agent_code_object(
                self.handle,
                agent.handle,
                reader,
                ptr::null(),
                &mut loaded_code_object,
            );

            if load_status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                // Clean up the reader before returning error
                log_debug("Cleaning up code object reader due to load failure");
                bindings::hsa_code_object_reader_destroy(reader);

                let error = HsaError::from_status_with_context(
                    load_status,
                    "Failed to load agent code object",
                );
                log_error(&format!("Agent code object load failed: {}", error));

                // Provide additional context for common errors
                let detailed_error = match load_status {
                    bindings::hsa_status_t_HSA_STATUS_ERROR_INCOMPATIBLE_ARGUMENTS => {
                        format!(
                            "{}\n  Possible causes:\n  - Code object ISA incompatible with agent\n  - Machine model mismatch\n  - Profile mismatch\n  - Floating-point mode mismatch",
                            error
                        )
                    }
                    bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_CODE_OBJECT => {
                        format!(
                            "{}\n  Possible causes:\n  - Corrupted code object\n  - Invalid file format\n  - Unsupported code object version",
                            error
                        )
                    }
                    bindings::hsa_status_t_HSA_STATUS_ERROR_OUT_OF_RESOURCES => {
                        format!(
                            "{}\n  Possible causes:\n  - Insufficient GPU memory\n  - Too many loaded code objects",
                            error
                        )
                    }
                    _ => error.to_string(),
                };

                return Err(HsaError::CodeObjectLoadFailed(detailed_error));
            }

            log_info(&format!(
                "Successfully loaded code object (handle: 0x{:x})",
                loaded_code_object.handle
            ));
        }

        self.code_object_reader = Some(reader);
        Ok(())
    }

    pub fn freeze(&self) -> Result<()> {
        log_debug("Freezing executable");

        unsafe {
            let status = bindings::hsa_executable_freeze(self.handle, ptr::null());
            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to freeze executable");
                log_error(&format!("Executable freeze failed: {}", error));

                // Provide additional context for common freeze errors
                let detailed_error = match status {
                    bindings::hsa_status_t_HSA_STATUS_ERROR_VARIABLE_UNDEFINED => {
                        format!(
                            "{}\n  One or more variables are undefined. All external variables must be defined before freezing.",
                            error
                        )
                    }
                    bindings::hsa_status_t_HSA_STATUS_ERROR_FROZEN_EXECUTABLE => {
                        format!("{}\n  Executable is already frozen.", error)
                    }
                    _ => error.to_string(),
                };

                return Err(HsaError::ExecutableFreezeFailed(detailed_error));
            }
        }

        log_info("Executable frozen successfully");
        Ok(())
    }

    pub fn get_kernel_symbol(&self, name: &str, agent: &Agent) -> Result<KernelSymbol> {
        log_debug(&format!("Looking for kernel symbol: '{}'", name));

        let c_name = CString::new(name)
            .map_err(|_| HsaError::InvalidArgument(format!("Invalid kernel name: '{}'", name)))?;

        let mut symbol = bindings::hsa_executable_symbol_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_executable_get_symbol_by_name(
                self.handle,
                c_name.as_ptr(),
                &agent.handle,
                &mut symbol,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    &format!("Failed to find kernel symbol '{}'", name),
                );
                log_error(&format!("Kernel symbol lookup failed: {}", error));

                // Provide helpful suggestions
                let detailed_error = match status {
                    bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_SYMBOL_NAME => {
                        format!("{}", error)
                    }
                    _ => error.to_string(),
                };

                return Err(HsaError::KernelNotFound(detailed_error));
            }
        }

        log_debug(&format!(
            "Found kernel symbol '{}' with handle: 0x{:x}",
            name, symbol.handle
        ));
        Ok(KernelSymbol { handle: symbol })
    }

    pub fn list_symbols(&self, agent: &Agent) -> Result<Vec<String>> {
        log_debug("Listing all symbols in executable");

        let mut symbols = Vec::new();

        unsafe {
            let status = bindings::hsa_executable_iterate_agent_symbols(
                self.handle,
                agent.handle,
                Some(collect_symbol_names_callback),
                &mut symbols as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(status, "Failed to iterate symbols");
                log_error(&format!("Symbol iteration failed: {}", error));
                return Err(error);
            }
        }

        log_info(&format!("Found {} symbols in executable", symbols.len()));
        for (i, symbol) in symbols.iter().enumerate() {
            log_debug(&format!("  Symbol {}: {}", i, symbol));
        }

        Ok(symbols)
    }
}

impl Drop for Executable {
    fn drop(&mut self) {
        log_debug("Dropping executable");

        if let Some(reader) = self.code_object_reader {
            unsafe {
                bindings::hsa_code_object_reader_destroy(reader);
            }
        }

        unsafe {
            let status = bindings::hsa_executable_destroy(self.handle);
            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                log_error(&format!(
                    "Failed to destroy executable: {}",
                    HsaError::from_status(status)
                ));
            }
        }
    }
}

pub struct KernelSymbol {
    handle: bindings::hsa_executable_symbol_t,
}

impl KernelSymbol {
    pub fn kernel_object(&self) -> Result<u64> {
        log_debug("Getting kernel object handle from symbol");

        let mut kernel_object = 0u64;

        unsafe {
            let status = bindings::hsa_executable_symbol_get_info(
                self.handle,
                bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_OBJECT,
                &mut kernel_object as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    "Failed to get kernel object from symbol",
                );
                log_error(&format!("Kernel object retrieval failed: {}", error));
                return Err(error);
            }
        }

        log_debug(&format!("Kernel object handle: 0x{:x}", kernel_object));
        Ok(kernel_object)
    }

    pub fn get_kernarg_segment_size(&self) -> Result<u32> {
        let mut size = 0u32;

        unsafe {
            let status = bindings::hsa_executable_symbol_get_info(
                self.handle,
                bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_KERNARG_SEGMENT_SIZE,
                &mut size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(size)
    }

    pub fn get_group_segment_size(&self) -> Result<u32> {
        let mut size = 0u32;

        unsafe {
            let status = bindings::hsa_executable_symbol_get_info(
                self.handle,
                bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_GROUP_SEGMENT_SIZE,
                &mut size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(size)
    }

    pub fn get_private_segment_size(&self) -> Result<u32> {
        let mut size = 0u32;

        unsafe {
            let status = bindings::hsa_executable_symbol_get_info(
                self.handle,
                bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_PRIVATE_SEGMENT_SIZE,
                &mut size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(size)
    }
}

pub struct KernelDispatch {
    pub kernel_object: u64,
    pub kernarg_address: *mut c_void,
    pub workgroup_size: (u16, u16, u16),
    pub grid_size: (u32, u32, u32),
    pub private_segment_size: u32,
    pub group_segment_size: u32,
}

impl KernelDispatch {
    pub fn dispatch(&self, queue: &Queue, completion_signal: &Signal) -> Result<()> {
        log_info(&format!(
            "Dispatching kernel - Grid: {}x{}x{}, Workgroup: {}x{}x{}",
            self.grid_size.0,
            self.grid_size.1,
            self.grid_size.2,
            self.workgroup_size.0,
            self.workgroup_size.1,
            self.workgroup_size.2
        ));

        let queue_ptr = queue.get();

        // Get packet index
        let packet_id = queue.add_write_index(1);
        log_debug(&format!("Allocated packet ID: {}", packet_id));

        // Get packet pointer
        let packet_ptr = unsafe {
            let base = queue_ptr.base_address as *mut bindings::hsa_kernel_dispatch_packet_t;
            &mut *base.add((packet_id % queue_ptr.size as u64) as usize)
        };

        // Clear packet
        unsafe {
            std::ptr::write_bytes(packet_ptr, 0, 1);
        }

        // Determine dimensions
        let dimensions = if self.grid_size.2 > 1 {
            3
        } else if self.grid_size.1 > 1 {
            2
        } else {
            1
        };

        log_debug(&format!("Using {} dimensions", dimensions));

        // Setup dimensions
        packet_ptr.setup = (dimensions as u16) << bindings::hsa_kernel_dispatch_packet_setup_t_HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS;

        // Setup header with proper memory fencing
        packet_ptr.header = (bindings::hsa_packet_type_t_HSA_PACKET_TYPE_KERNEL_DISPATCH as u16)
            << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_TYPE
            | (bindings::hsa_fence_scope_t_HSA_FENCE_SCOPE_SYSTEM as u16)
                << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_SCACQUIRE_FENCE_SCOPE
            | (bindings::hsa_fence_scope_t_HSA_FENCE_SCOPE_SYSTEM as u16)
                << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_SCRELEASE_FENCE_SCOPE;

        // Set workgroup and grid sizes
        packet_ptr.workgroup_size_x = self.workgroup_size.0;
        packet_ptr.workgroup_size_y = self.workgroup_size.1;
        packet_ptr.workgroup_size_z = self.workgroup_size.2;
        packet_ptr.grid_size_x = self.grid_size.0;
        packet_ptr.grid_size_y = self.grid_size.1;
        packet_ptr.grid_size_z = self.grid_size.2;

        // Set kernel object and arguments
        packet_ptr.kernel_object = self.kernel_object;
        packet_ptr.kernarg_address = self.kernarg_address;
        packet_ptr.private_segment_size = self.private_segment_size;
        packet_ptr.group_segment_size = self.group_segment_size;
        packet_ptr.completion_signal = completion_signal.handle();

        log_debug(&format!(
            "Packet configured: kernel_object=0x{:x}, kernarg_address={:p}",
            self.kernel_object, self.kernarg_address
        ));

        // Submit packet
        queue.store_write_index(packet_id + 1);
        log_debug(&format!("Updated queue write index to {}", packet_id + 1));

        // Ring doorbell
        unsafe {
            bindings::hsa_signal_store_relaxed(queue_ptr.doorbell_signal, packet_id as i64);
        }
        log_debug(&format!("Doorbell rung with packet ID: {}", packet_id));

        log_info("Kernel dispatch completed successfully");
        Ok(())
    }
}

// Callback function to collect symbol names
unsafe extern "C" fn collect_symbol_names_callback(
    _exec: bindings::hsa_executable_t,
    _agent: bindings::hsa_agent_t,
    symbol: bindings::hsa_executable_symbol_t,
    data: *mut c_void,
) -> bindings::hsa_status_t {
    let symbols = unsafe { &mut *(data as *mut Vec<String>) };

    // Get symbol name length
    let mut name_length = 0u32;
    let mut status = unsafe {
        bindings::hsa_executable_symbol_get_info(
            symbol,
            bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_NAME_LENGTH,
            &mut name_length as *mut _ as *mut c_void,
        )
    };

    if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
        return status;
    }

    // Get symbol name
    let mut name_buffer = vec![0u8; (name_length + 1) as usize];
    status = unsafe {
        bindings::hsa_executable_symbol_get_info(
            symbol,
            bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_NAME,
            name_buffer.as_mut_ptr() as *mut c_void,
        )
    };

    if status == bindings::hsa_status_t_HSA_STATUS_SUCCESS {
        if let Ok(name) = std::ffi::CStr::from_bytes_with_nul(&name_buffer) {
            if let Ok(name_str) = name.to_str() {
                symbols.push(name_str.to_string());
            }
        }
    }

    bindings::hsa_status_t_HSA_STATUS_SUCCESS
}
