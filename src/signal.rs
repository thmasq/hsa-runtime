use crate::bindings;
use crate::error::{log_debug, log_error, log_info};
use crate::{HsaError, Result};
use std::ptr;

pub struct Signal {
    handle: bindings::hsa_signal_t,
}

impl Signal {
    pub fn create(initial_value: i64) -> Result<Self> {
        log_debug(&format!(
            "Creating signal with initial value: {}",
            initial_value
        ));

        let mut signal = bindings::hsa_signal_t { handle: 0 };

        unsafe {
            let status = bindings::hsa_signal_create(initial_value, 0, ptr::null(), &mut signal);

            if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                let error = HsaError::from_status_with_context(
                    status,
                    &format!(
                        "Failed to create signal with initial value {}",
                        initial_value
                    ),
                );
                log_error(&format!("Signal creation failed: {}", error));
                return Err(HsaError::SignalOperationFailed(error.to_string()));
            }
        }

        if signal.handle == 0 {
            return Err(HsaError::SignalOperationFailed(
                "Signal creation returned invalid handle (0)".to_string(),
            ));
        }

        log_debug(&format!(
            "Signal created successfully with handle: 0x{:x}",
            signal.handle
        ));

        Ok(Signal { handle: signal })
    }

    pub fn handle(&self) -> bindings::hsa_signal_t {
        self.handle
    }

    pub fn load(&self) -> i64 {
        let value = unsafe { bindings::hsa_signal_load_scacquire(self.handle) };
        log_debug(&format!(
            "Signal 0x{:x} loaded value: {}",
            self.handle.handle, value
        ));
        value
    }

    pub fn store(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} storing value: {}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_store_relaxed(self.handle, value);
        }
    }

    pub fn wait_eq(&self, value: i64, timeout_ns: u64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} waiting for value {} (timeout: {} ns)",
            self.handle.handle, value, timeout_ns
        ));

        let result = unsafe {
            bindings::hsa_signal_wait_scacquire(
                self.handle,
                bindings::hsa_signal_condition_t_HSA_SIGNAL_CONDITION_EQ,
                value,
                timeout_ns,
                bindings::hsa_wait_state_t_HSA_WAIT_STATE_BLOCKED,
            )
        };

        log_debug(&format!(
            "Signal 0x{:x} wait completed with value: {}",
            self.handle.handle, result
        ));
        result
    }

    pub fn wait_ne(&self, value: i64, timeout_ns: u64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} waiting for value != {} (timeout: {} ns)",
            self.handle.handle, value, timeout_ns
        ));

        let result = unsafe {
            bindings::hsa_signal_wait_scacquire(
                self.handle,
                bindings::hsa_signal_condition_t_HSA_SIGNAL_CONDITION_NE,
                value,
                timeout_ns,
                bindings::hsa_wait_state_t_HSA_WAIT_STATE_BLOCKED,
            )
        };

        log_debug(&format!(
            "Signal 0x{:x} wait_ne completed with value: {}",
            self.handle.handle, result
        ));
        result
    }

    pub fn wait_lt(&self, value: i64, timeout_ns: u64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} waiting for value < {} (timeout: {} ns)",
            self.handle.handle, value, timeout_ns
        ));

        let result = unsafe {
            bindings::hsa_signal_wait_scacquire(
                self.handle,
                bindings::hsa_signal_condition_t_HSA_SIGNAL_CONDITION_LT,
                value,
                timeout_ns,
                bindings::hsa_wait_state_t_HSA_WAIT_STATE_BLOCKED,
            )
        };

        log_debug(&format!(
            "Signal 0x{:x} wait_lt completed with value: {}",
            self.handle.handle, result
        ));
        result
    }

    pub fn wait_gte(&self, value: i64, timeout_ns: u64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} waiting for value >= {} (timeout: {} ns)",
            self.handle.handle, value, timeout_ns
        ));

        let result = unsafe {
            bindings::hsa_signal_wait_scacquire(
                self.handle,
                bindings::hsa_signal_condition_t_HSA_SIGNAL_CONDITION_GTE,
                value,
                timeout_ns,
                bindings::hsa_wait_state_t_HSA_WAIT_STATE_BLOCKED,
            )
        };

        log_debug(&format!(
            "Signal 0x{:x} wait_gte completed with value: {}",
            self.handle.handle, result
        ));
        result
    }

    pub fn add(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} adding value: {}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_add_relaxed(self.handle, value);
        }
    }

    pub fn subtract(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} subtracting value: {}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_subtract_relaxed(self.handle, value);
        }
    }

    pub fn exchange(&self, value: i64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} exchanging with value: {}",
            self.handle.handle, value
        ));
        let old_value = unsafe { bindings::hsa_signal_exchange_relaxed(self.handle, value) };
        log_debug(&format!(
            "Signal 0x{:x} exchange: old value {} -> new value {}",
            self.handle.handle, old_value, value
        ));
        old_value
    }

    pub fn compare_and_swap(&self, expected: i64, value: i64) -> i64 {
        log_debug(&format!(
            "Signal 0x{:x} CAS: expected {}, new value {}",
            self.handle.handle, expected, value
        ));
        let old_value = unsafe { bindings::hsa_signal_cas_relaxed(self.handle, expected, value) };
        log_debug(&format!(
            "Signal 0x{:x} CAS result: old value {}, succeeded: {}",
            self.handle.handle,
            old_value,
            old_value == expected
        ));
        old_value
    }

    pub fn and(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} AND with value: 0x{:x}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_and_relaxed(self.handle, value);
        }
    }

    pub fn or(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} OR with value: 0x{:x}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_or_relaxed(self.handle, value);
        }
    }

    pub fn xor(&self, value: i64) {
        log_debug(&format!(
            "Signal 0x{:x} XOR with value: 0x{:x}",
            self.handle.handle, value
        ));
        unsafe {
            bindings::hsa_signal_xor_relaxed(self.handle, value);
        }
    }

    pub fn print_info(&self) {
        let current_value = self.load();
        log_info(&format!("Signal Information:"));
        log_info(&format!("  Handle: 0x{:x}", self.handle.handle));
        log_info(&format!("  Current Value: {}", current_value));
    }
}

impl Drop for Signal {
    fn drop(&mut self) {
        if self.handle.handle != 0 {
            log_debug(&format!("Destroying signal 0x{:x}", self.handle.handle));
            unsafe {
                let status = bindings::hsa_signal_destroy(self.handle);
                if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
                    log_error(&format!(
                        "Failed to destroy signal: {}",
                        HsaError::from_status(status)
                    ));
                }
            }
        }
    }
}

unsafe impl Send for Signal {}
unsafe impl Sync for Signal {}
