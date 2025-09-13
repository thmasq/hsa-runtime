use crate::bindings;
use crate::{HsaError, Result};
use std::ptr;

pub struct Signal {
    handle: bindings::hsa_signal_t,
}

impl Signal {
    pub fn create(initial_value: i64) -> Result<Self> {
        let mut signal = bindings::hsa_signal_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_signal_create(initial_value, 0, ptr::null(), &mut signal);

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                return Err(HsaError::SignalOperationFailed);
            }
        }

        Ok(Signal { handle: signal })
    }

    pub fn handle(&self) -> bindings::hsa_signal_t {
        self.handle
    }

    pub fn wait_eq(&self, value: i64, timeout_ns: u64) -> i64 {
        unsafe {
            bindings::hsa_signal_wait_scacquire(
                self.handle,
                bindings::hsa_signal_condition_t_HSA_SIGNAL_CONDITION_EQ,
                value,
                timeout_ns,
                bindings::hsa_wait_state_t_HSA_WAIT_STATE_BLOCKED,
            )
        }
    }

    pub fn store(&self, value: i64) {
        unsafe {
            bindings::hsa_signal_store_relaxed(self.handle, value);
        }
    }
}

impl Drop for Signal {
    fn drop(&mut self) {
        unsafe {
            bindings::hsa_signal_destroy(self.handle);
        }
    }
}
