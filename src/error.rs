use crate::bindings;
use std::ffi::CStr;
use std::os::raw::c_char;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, HsaError>;

#[derive(Debug, Error)]
pub enum HsaError {
    #[error("HSA initialization failed")]
    InitializationFailed,

    #[error("HSA shutdown failed")]
    ShutdownFailed,

    #[error("No GPU agent found")]
    AgentNotFound,

    #[error("Queue creation failed: {0}")]
    QueueCreationFailed(String),

    #[error("Memory allocation failed: {0}")]
    MemoryAllocationFailed(String),

    #[error("Code object reader creation failed: {0}")]
    CodeObjectReaderFailed(String),

    #[error("Code object load failed: {0}")]
    CodeObjectLoadFailed(String),

    #[error("Executable creation failed: {0}")]
    ExecutableCreationFailed(String),

    #[error("Executable freeze failed: {0}")]
    ExecutableFreezeFailed(String),

    #[error("Kernel not found: {0}")]
    KernelNotFound(String),

    #[error("Kernel execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Required memory region not found")]
    MemoryRegionNotFound,

    #[error("Signal operation failed: {0}")]
    SignalOperationFailed(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Invalid agent: {0}")]
    InvalidAgent(String),

    #[error("Invalid region: {0}")]
    InvalidRegion(String),

    #[error("Invalid allocation: {0}")]
    InvalidAllocation(String),

    #[error("Invalid code object: {0}")]
    InvalidCodeObject(String),

    #[error("Invalid executable: {0}")]
    InvalidExecutable(String),

    #[error("Invalid ISA: {0}")]
    InvalidIsa(String),

    #[error("Invalid symbol name: {0}")]
    InvalidSymbolName(String),

    #[error("Frozen executable: {0}")]
    FrozenExecutable(String),

    #[error("Variable already defined: {0}")]
    VariableAlreadyDefined(String),

    #[error("Variable undefined: {0}")]
    VariableUndefined(String),

    #[error("Incompatible arguments: {0}")]
    IncompatibleArguments(String),

    #[error("Out of resources: {0}")]
    OutOfResources(String),

    #[error("Runtime not initialized: {0}")]
    NotInitialized(String),

    #[error("Fatal HSA error: {0}")]
    Fatal(String),

    #[error("HSA error {status}: {description}")]
    HsaStatus { status: u32, description: String },

    #[error("String conversion error")]
    StringConversionError,
}

impl HsaError {
    pub fn from_status(status: bindings::hsa_status_t) -> Self {
        let description = get_status_string(status);

        match status {
            bindings::hsa_status_t_HSA_STATUS_SUCCESS => {
                // This shouldn't happen, but handle gracefully
                Self::HsaStatus {
                    status,
                    description,
                }
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ARGUMENT => {
                Self::InvalidArgument(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_QUEUE_CREATION => {
                Self::QueueCreationFailed(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ALLOCATION => {
                Self::InvalidAllocation(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_AGENT => {
                Self::InvalidAgent(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_REGION => {
                Self::InvalidRegion(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_OUT_OF_RESOURCES => {
                Self::OutOfResources(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_NOT_INITIALIZED => {
                Self::NotInitialized(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_CODE_OBJECT => {
                Self::InvalidCodeObject(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_EXECUTABLE => {
                Self::InvalidExecutable(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_FROZEN_EXECUTABLE => {
                Self::FrozenExecutable(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_SYMBOL_NAME => {
                Self::InvalidSymbolName(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_VARIABLE_ALREADY_DEFINED => {
                Self::VariableAlreadyDefined(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_VARIABLE_UNDEFINED => {
                Self::VariableUndefined(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INCOMPATIBLE_ARGUMENTS => {
                Self::IncompatibleArguments(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ISA => Self::InvalidIsa(description),
            bindings::hsa_status_t_HSA_STATUS_ERROR_INVALID_ISA_NAME => {
                Self::InvalidIsa(description)
            }
            bindings::hsa_status_t_HSA_STATUS_ERROR_FATAL => Self::Fatal(description),
            _ => Self::HsaStatus {
                status,
                description,
            },
        }
    }

    pub fn from_status_with_context(status: bindings::hsa_status_t, context: &str) -> Self {
        let mut error = Self::from_status(status);

        // Add context to the error message
        match &mut error {
            Self::InvalidArgument(msg)
            | Self::QueueCreationFailed(msg)
            | Self::InvalidAllocation(msg)
            | Self::InvalidAgent(msg)
            | Self::InvalidRegion(msg)
            | Self::OutOfResources(msg)
            | Self::NotInitialized(msg)
            | Self::InvalidCodeObject(msg)
            | Self::InvalidExecutable(msg)
            | Self::FrozenExecutable(msg)
            | Self::InvalidSymbolName(msg)
            | Self::VariableAlreadyDefined(msg)
            | Self::VariableUndefined(msg)
            | Self::IncompatibleArguments(msg)
            | Self::InvalidIsa(msg)
            | Self::Fatal(msg) => {
                *msg = format!("{}: {}", context, msg);
            }
            Self::HsaStatus { description, .. } => {
                *description = format!("{}: {}", context, description);
            }
            _ => {}
        }

        error
    }
}

fn get_status_string(status: bindings::hsa_status_t) -> String {
    unsafe {
        let mut status_string_ptr: *const c_char = std::ptr::null();
        let result = bindings::hsa_status_string(status, &mut status_string_ptr);

        if result == bindings::hsa_status_t_HSA_STATUS_SUCCESS && !status_string_ptr.is_null() {
            match CStr::from_ptr(status_string_ptr).to_str() {
                Ok(s) => s.to_string(),
                Err(_) => format!("HSA status code: 0x{:x}", status),
            }
        } else {
            format!("HSA status code: 0x{:x}", status)
        }
    }
}

// Logging utilities
pub fn log_info(message: &str) {
    eprintln!("[HSA INFO] {}", message);
}

pub fn log_warning(message: &str) {
    eprintln!("[HSA WARN] {}", message);
}

pub fn log_error(message: &str) {
    eprintln!("[HSA ERROR] {}", message);
}

pub fn log_debug(message: &str) {
    if std::env::var("HSA_DEBUG").is_ok() {
        eprintln!("[HSA DEBUG] {}", message);
    }
}
