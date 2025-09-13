use crate::bindings;
use crate::error::{log_debug, log_error};
use crate::{Agent, HsaError, Result};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;

#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    pub(crate) handle: bindings::hsa_region_t,
}

impl MemoryRegion {
    pub fn segment(&self) -> Result<bindings::hsa_region_segment_t> {
        let mut segment = 0u32;
        unsafe {
            let status = bindings::hsa_region_get_info(
                self.handle,
                bindings::hsa_region_info_t_HSA_REGION_INFO_SEGMENT,
                &mut segment as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status_with_context(
                    status,
                    "Failed to get memory region segment",
                ));
            }
        }
        Ok(segment)
    }

    pub fn global_flags(&self) -> Result<u32> {
        let mut flags = 0u32;
        unsafe {
            let status = bindings::hsa_region_get_info(
                self.handle,
                bindings::hsa_region_info_t_HSA_REGION_INFO_GLOBAL_FLAGS,
                &mut flags as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status_with_context(
                    status,
                    "Failed to get memory region global flags",
                ));
            }
        }
        Ok(flags)
    }

    pub fn size(&self) -> Result<usize> {
        let mut size = 0usize;
        unsafe {
            let status = bindings::hsa_region_get_info(
                self.handle,
                bindings::hsa_region_info_t_HSA_REGION_INFO_SIZE,
                &mut size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status_with_context(
                    status,
                    "Failed to get memory region size",
                ));
            }
        }
        Ok(size)
    }

    pub fn max_alloc_size(&self) -> Result<usize> {
        let mut max_size = 0usize;
        unsafe {
            let status = bindings::hsa_region_get_info(
                self.handle,
                bindings::hsa_region_info_t_HSA_REGION_INFO_ALLOC_MAX_SIZE,
                &mut max_size as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status_with_context(
                    status,
                    "Failed to get memory region max allocation size",
                ));
            }
        }
        Ok(max_size)
    }

    pub fn runtime_alloc_allowed(&self) -> Result<bool> {
        let mut allowed = false;
        unsafe {
            let status = bindings::hsa_region_get_info(
                self.handle,
                bindings::hsa_region_info_t_HSA_REGION_INFO_RUNTIME_ALLOC_ALLOWED,
                &mut allowed as *mut _ as *mut c_void,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status_with_context(
                    status,
                    "Failed to get memory region allocation permission",
                ));
            }
        }
        Ok(allowed)
    }

    pub fn allocate(&self, size: usize) -> Result<Memory> {
        log_debug(&format!(
            "Allocating {} bytes from memory region 0x{:x}",
            size, self.handle.handle
        ));

        // Check if allocation is allowed
        if !self.runtime_alloc_allowed()? {
            return Err(HsaError::MemoryAllocationFailed(
                "Runtime allocation not allowed for this memory region".to_string(),
            ));
        }

        // Check if size exceeds maximum
        let max_size = self.max_alloc_size()?;
        if size > max_size {
            return Err(HsaError::MemoryAllocationFailed(format!(
                "Requested size {} exceeds maximum allocation size {} for this region",
                size, max_size
            )));
        }

        let mut ptr = ptr::null_mut();
        unsafe {
            let status = bindings::hsa_memory_allocate(self.handle, size, &mut ptr);

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    &format!("Failed to allocate {} bytes from memory region", size),
                );
                log_error(&format!("Memory allocation failed: {}", error));
                return Err(HsaError::MemoryAllocationFailed(error.to_string()));
            }
        }

        log_debug(&format!(
            "Successfully allocated {} bytes at address {:p}",
            size, ptr
        ));

        Ok(Memory {
            ptr,
            size,
            _phantom: PhantomData,
        })
    }
}

pub struct Memory {
    ptr: *mut c_void,
    size: usize,
    _phantom: PhantomData<[u8]>,
}

impl Memory {
    pub fn as_ptr(&self) -> *mut c_void {
        self.ptr
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.ptr as *const u8, self.size) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.ptr as *mut u8, self.size) }
    }

    pub fn allow_access(&self, agents: &[Agent]) -> Result<()> {
        if agents.is_empty() {
            log_debug("No agents specified for memory access - allowing default access");
            return Ok(());
        }

        let agent_handles: Vec<_> = agents.iter().map(|a| a.handle).collect();
        log_debug(&format!(
            "Allowing memory access for {} agents",
            agent_handles.len()
        ));

        unsafe {
            let status = bindings::hsa_amd_agents_allow_access(
                agent_handles.len() as u32,
                agent_handles.as_ptr(),
                ptr::null(),
                self.ptr,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    "Failed to allow memory access for agents",
                );
                log_error(&format!("Memory access permission failed: {}", error));
                return Err(error);
            }
        }

        log_debug("Memory access permissions set successfully");
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            log_debug(&format!(
                "Freeing memory at address {:p} ({} bytes)",
                self.ptr, self.size
            ));
            unsafe {
                let status = bindings::hsa_memory_free(self.ptr);
                if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                    log_error(&format!(
                        "Failed to free memory: {}",
                        HsaError::from_status(status)
                    ));
                }
            }
        }
    }
}

unsafe impl Send for Memory {}
unsafe impl Sync for Memory {}
