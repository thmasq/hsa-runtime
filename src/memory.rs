use crate::bindings;
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
                return Err(HsaError::from_status(status));
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
                return Err(HsaError::from_status(status));
            }
        }
        Ok(flags)
    }

    pub fn allocate(&self, size: usize) -> Result<Memory> {
        let mut ptr = ptr::null_mut();
        unsafe {
            let status = bindings::hsa_memory_allocate(self.handle, size, &mut ptr);

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::MemoryAllocationFailed);
            }
        }

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
            return Ok(());
        }

        let agent_handles: Vec<_> = agents.iter().map(|a| a.handle).collect();

        unsafe {
            let status = bindings::hsa_amd_agents_allow_access(
                agent_handles.len() as u32,
                agent_handles.as_ptr(),
                ptr::null(),
                self.ptr,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::from_status(status));
            }
        }

        Ok(())
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        unsafe {
            bindings::hsa_memory_free(self.ptr);
        }
    }
}
