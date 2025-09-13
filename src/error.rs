use crate::bindings;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, HsaError>;

#[derive(Debug, Error)]
pub enum HsaError {
    #[error("HSA initialization failed")]
    InitializationFailed,

    #[error("No GPU agent found")]
    AgentNotFound,

    #[error("Queue creation failed")]
    QueueCreationFailed,

    #[error("Memory allocation failed")]
    MemoryAllocationFailed,

    #[error("Code object load failed")]
    CodeObjectLoadFailed,

    #[error("Kernel not found")]
    KernelNotFound,

    #[error("Kernel execution failed")]
    ExecutionFailed,

    #[error("Required memory region not found")]
    MemoryRegionNotFound,

    #[error("Signal operation failed")]
    SignalOperationFailed,

    #[error("Invalid argument")]
    InvalidArgument,

    #[error("HSA error: {0}")]
    HsaStatus(u32),
}

impl HsaError {
    pub fn from_status(status: bindings::hsa_status_t) -> Self {
        match status {
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ARGUMENT => Self::InvalidArgument,
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_QUEUE_CREATION => {
                Self::QueueCreationFailed
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ALLOCATION => {
                Self::MemoryAllocationFailed
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_AGENT => Self::AgentNotFound,
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_REGION => Self::MemoryRegionNotFound,
            _ => Self::HsaStatus(status),
        }
    }
}
