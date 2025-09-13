use crate::bindings;
use crate::{Agent, HsaError, Result};
use std::ptr;

pub struct Queue {
    ptr: *mut bindings::hsa_queue_t,
}

impl Queue {
    pub fn create(agent: &Agent, size: u32) -> Result<Self> {
        let mut queue_ptr = ptr::null_mut();

        unsafe {
            let status = bindings::hsa_queue_create(
                agent.handle,
                size,
                bindings::hsa_queue_type_t_HSA_QUEUE_TYPE_MULTI,
                None,
                ptr::null_mut(),
                0,
                0,
                &mut queue_ptr,
            );

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::QueueCreationFailed);
            }
        }

        Ok(Queue { ptr: queue_ptr })
    }

    pub fn as_ptr(&self) -> *mut bindings::hsa_queue_t {
        self.ptr
    }

    pub fn get(&self) -> &bindings::hsa_queue_t {
        unsafe { &*self.ptr }
    }

    pub fn add_write_index(&self, value: u64) -> u64 {
        unsafe { bindings::hsa_queue_add_write_index_relaxed(self.ptr, value) }
    }

    pub fn store_write_index(&self, value: u64) {
        unsafe {
            bindings::hsa_queue_store_write_index_relaxed(self.ptr, value);
        }
    }

    pub fn load_read_index(&self) -> u64 {
        unsafe { bindings::hsa_queue_load_read_index_scacquire(self.ptr) }
    }

    pub fn load_write_index(&self) -> u64 {
        unsafe { bindings::hsa_queue_load_write_index_scacquire(self.ptr) }
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        unsafe {
            bindings::hsa_queue_destroy(self.ptr);
        }
    }
}

unsafe impl Send for Queue {}
unsafe impl Sync for Queue {}
