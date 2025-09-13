use crate::Queue;
use crate::bindings;
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
        let mut executable = bindings::hsa_executable_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_executable_create_alt(
                bindings::hsa_profile_t_HSA_PROFILE_FULL,
                bindings::hsa_default_float_rounding_mode_t_HSA_DEFAULT_FLOAT_ROUNDING_MODE_DEFAULT,
                ptr::null(),
                &mut executable,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(Executable {
            handle: executable,
            code_object_reader: None,
        })
    }

    pub fn load_code_object(&mut self, agent: &Agent, code_object: &[u8]) -> Result<()> {
        let mut reader = bindings::hsa_code_object_reader_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_code_object_reader_create_from_memory(
                code_object.as_ptr() as *const c_void,
                code_object.len(),
                &mut reader,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::CodeObjectLoadFailed);
            }

            let load_status = bindings::hsa_executable_load_agent_code_object(
                self.handle,
                agent.handle,
                reader,
                ptr::null(),
                ptr::null_mut(),
            );

            if load_status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                bindings::hsa_code_object_reader_destroy(reader);
                return Err(HsaError::CodeObjectLoadFailed);
            }
        }

        self.code_object_reader = Some(reader);
        Ok(())
    }

    pub fn freeze(&self) -> Result<()> {
        unsafe {
            let status = bindings::hsa_executable_freeze(self.handle, ptr::null());
            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }
        Ok(())
    }

    pub fn get_kernel_symbol(&self, name: &str, agent: &Agent) -> Result<KernelSymbol> {
        let c_name = CString::new(name).map_err(|_| HsaError::InvalidArgument)?;
        let mut symbol = bindings::hsa_executable_symbol_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_executable_get_symbol_by_name(
                self.handle,
                c_name.as_ptr(),
                &agent.handle,
                &mut symbol,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::KernelNotFound);
            }
        }

        Ok(KernelSymbol { handle: symbol })
    }
}

impl Drop for Executable {
    fn drop(&mut self) {
        if let Some(reader) = self.code_object_reader {
            unsafe {
                bindings::hsa_code_object_reader_destroy(reader);
            }
        }
        unsafe {
            bindings::hsa_executable_destroy(self.handle);
        }
    }
}

pub struct KernelSymbol {
    handle: bindings::hsa_executable_symbol_t,
}

impl KernelSymbol {
    pub fn kernel_object(&self) -> Result<u64> {
        let mut kernel_object = 0u64;

        unsafe {
            let status = bindings::hsa_executable_symbol_get_info(
                self.handle,
                bindings::hsa_executable_symbol_info_t_HSA_EXECUTABLE_SYMBOL_INFO_KERNEL_OBJECT,
                &mut kernel_object as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(kernel_object)
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
        let queue_ptr = queue.get();

        // Get packet index
        let packet_id = queue.add_write_index(1);

        // Get packet pointer
        let packet_ptr = unsafe {
            let base = queue_ptr.base_address as *mut bindings::hsa_kernel_dispatch_packet_t;
            &mut *base.add((packet_id % queue_ptr.size as u64) as usize)
        };

        // Clear packet
        unsafe {
            std::ptr::write_bytes(packet_ptr, 0, 1);
        }

        // Setup dimensions (2D by default)
        packet_ptr.setup = (2u16) << bindings::hsa_kernel_dispatch_packet_setup_t_HSA_KERNEL_DISPATCH_PACKET_SETUP_DIMENSIONS;

        // Setup header
        packet_ptr.header = (bindings::hsa_packet_type_t_HSA_PACKET_TYPE_KERNEL_DISPATCH as u16)
            << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_TYPE
            | (bindings::hsa_fence_scope_t_HSA_FENCE_SCOPE_SYSTEM as u16)
                << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_ACQUIRE_FENCE_SCOPE
            | (bindings::hsa_fence_scope_t_HSA_FENCE_SCOPE_SYSTEM as u16)
                << bindings::hsa_packet_header_t_HSA_PACKET_HEADER_RELEASE_FENCE_SCOPE;

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

        // Submit packet
        queue.store_write_index(packet_id + 1);

        // Ring doorbell
        unsafe {
            bindings::hsa_signal_store_relaxed(queue_ptr.doorbell_signal, packet_id as i64);
        }

        Ok(())
    }
}
