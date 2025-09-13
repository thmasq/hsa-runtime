use crate::bindings;
use crate::error::{log_debug, log_error, log_info};
use crate::{Agent, HsaError, Result};
use std::ptr;

pub struct Queue {
    ptr: *mut bindings::hsa_queue_t,
}

impl Queue {
    pub fn create(agent: &Agent, size: u32) -> Result<Self> {
        log_info(&format!(
            "Creating queue with size {} for agent 0x{:x}",
            size, agent.handle.handle
        ));

        // Validate size is power of 2
        if size == 0 || (size & (size - 1)) != 0 {
            return Err(HsaError::QueueCreationFailed(format!(
                "Queue size {} must be a power of 2 and greater than 0",
                size
            )));
        }

        // Check agent queue size limits
        let min_size = agent.get_queue_min_size().unwrap_or(1);
        let max_size = agent.get_queue_max_size().unwrap_or(u32::MAX);

        if size < min_size {
            return Err(HsaError::QueueCreationFailed(format!(
                "Queue size {} is below minimum {} for this agent",
                size, min_size
            )));
        }

        if size > max_size {
            return Err(HsaError::QueueCreationFailed(format!(
                "Queue size {} exceeds maximum {} for this agent",
                size, max_size
            )));
        }

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
                let error = HsaError::from_status_with_context(
                    status,
                    &format!(
                        "Failed to create queue with size {} for agent 0x{:x}",
                        size, agent.handle.handle
                    ),
                );
                log_error(&format!("Queue creation failed: {}", error));
                return Err(HsaError::QueueCreationFailed(error.to_string()));
            }
        }

        if queue_ptr.is_null() {
            return Err(HsaError::QueueCreationFailed(
                "Queue creation returned null pointer".to_string(),
            ));
        }

        let queue = Queue { ptr: queue_ptr };
        let actual_size = queue.get().size;

        log_info(&format!(
            "Queue created successfully: requested size {}, actual size {}",
            size, actual_size
        ));
        log_debug(&format!("Queue pointer: {:p}", queue_ptr));

        Ok(queue)
    }

    pub fn as_ptr(&self) -> *mut bindings::hsa_queue_t {
        self.ptr
    }

    pub fn get(&self) -> &bindings::hsa_queue_t {
        unsafe { &*self.ptr }
    }

    pub fn add_write_index(&self, value: u64) -> u64 {
        let old_index = unsafe { bindings::hsa_queue_add_write_index_relaxed(self.ptr, value) };
        log_debug(&format!(
            "Added {} to write index, old value: {}, new value: {}",
            value,
            old_index,
            old_index + value
        ));
        old_index
    }

    pub fn store_write_index(&self, value: u64) {
        log_debug(&format!("Storing write index: {}", value));
        unsafe {
            bindings::hsa_queue_store_write_index_relaxed(self.ptr, value);
        }
    }

    pub fn load_read_index(&self) -> u64 {
        let index = unsafe { bindings::hsa_queue_load_read_index_scacquire(self.ptr) };
        log_debug(&format!("Read index: {}", index));
        index
    }

    pub fn load_write_index(&self) -> u64 {
        let index = unsafe { bindings::hsa_queue_load_write_index_scacquire(self.ptr) };
        log_debug(&format!("Write index: {}", index));
        index
    }

    pub fn get_id(&self) -> u64 {
        self.get().id
    }

    pub fn get_size(&self) -> u32 {
        self.get().size
    }

    pub fn get_type(&self) -> u32 {
        self.get().type_
    }

    pub fn inactivate(&self) -> Result<()> {
        log_info("Inactivating queue");
        unsafe {
            let status = bindings::hsa_queue_inactivate(self.ptr);
            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error =
                    HsaError::from_status_with_context(status, "Failed to inactivate queue");
                log_error(&format!("Queue inactivation failed: {}", error));
                return Err(error);
            }
        }
        log_info("Queue inactivated successfully");
        Ok(())
    }

    pub fn print_info(&self) {
        let queue_ref = self.get();
        log_info(&format!("Queue Information:"));
        log_info(&format!("  ID: {}", queue_ref.id));
        log_info(&format!("  Size: {}", queue_ref.size));
        log_info(&format!("  Type: {}", queue_ref.type_));
        log_info(&format!("  Features: 0x{:x}", queue_ref.features));
        log_info(&format!("  Base Address: {:p}", queue_ref.base_address));
        log_info(&format!(
            "  Doorbell Signal: 0x{:x}",
            queue_ref.doorbell_signal.handle
        ));

        // Show current queue state
        let read_idx = self.load_read_index();
        let write_idx = self.load_write_index();
        let pending = write_idx.wrapping_sub(read_idx);

        log_info(&format!("  Current State:"));
        log_info(&format!("    Read Index: {}", read_idx));
        log_info(&format!("    Write Index: {}", write_idx));
        log_info(&format!("    Pending Packets: {}", pending));
        log_info(&format!(
            "    Utilization: {:.1}%",
            (pending as f64 / queue_ref.size as f64) * 100.0
        ));
    }
}

impl Drop for Queue {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            log_debug("Destroying queue");
            unsafe {
                let status = bindings::hsa_queue_destroy(self.ptr);
                if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                    log_error(&format!(
                        "Failed to destroy queue: {}",
                        HsaError::from_status(status)
                    ));
                }
            }
        }
    }
}

unsafe impl Send for Queue {}
unsafe impl Sync for Queue {}
