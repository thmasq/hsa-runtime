//! HSA Runtime bindings for Rust
//!
//! This library provides safe Rust bindings for AMD's HSA Runtime,
//! focusing on kernel loading and execution functionality.

mod agent;
mod bindings;
mod context;
pub mod error;
mod executable;
mod memory;
mod queue;
mod signal;

pub use agent::{Agent, DeviceType};
pub use context::HsaContext;
pub use error::{HsaError, Result};
pub use executable::{Executable, KernelDispatch, KernelSymbol};
pub use memory::{Memory, MemoryRegion};
pub use queue::Queue;
pub use signal::Signal;

/// Initialize the HSA runtime
pub fn init() -> Result<()> {
    unsafe {
        let status = bindings::hsa_init();
        if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
            return Err(HsaError::from_status(status));
        }
    }
    Ok(())
}

/// Shutdown the HSA runtime
pub fn shutdown() -> Result<()> {
    unsafe {
        let status = bindings::hsa_shut_down();
        if status != bindings::hsa_status_t_HSA_STATUS_SUCCESS {
            return Err(HsaError::from_status(status));
        }
    }
    Ok(())
}
