use std::fmt::{Display, Formatter};

use crate::util::Pid;

#[derive(Debug, Clone, Copy)]
pub enum ProcessControlOperation {
    Terminate,
    ForceKill,
}

#[derive(Debug)]
pub enum ProcessControlError {
    InvalidPid(Pid),
    ProcessNotFound(Pid),
    PermissionDenied(Pid),
    UnsupportedOperation,
    SignalFailed {
        pid: Pid,
        operation: ProcessControlOperation,
        errno: i32,
        details: String,
    },
}

impl ProcessControlError {
    pub fn from_errno(pid: Pid, operation: ProcessControlOperation, errno: i32) -> Self {
        match errno {
            libc::ESRCH => ProcessControlError::ProcessNotFound(pid),
            libc::EPERM => ProcessControlError::PermissionDenied(pid),
            _ => ProcessControlError::SignalFailed {
                pid,
                operation,
                errno,
                details: std::io::Error::from_raw_os_error(errno).to_string(),
            },
        }
    }
}

impl Display for ProcessControlError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessControlError::InvalidPid(pid) => write!(f, "invalid pid: {}", pid),
            ProcessControlError::ProcessNotFound(pid) => {
                write!(f, "process not found: {}", pid)
            }
            ProcessControlError::PermissionDenied(pid) => {
                write!(f, "permission denied for pid: {}", pid)
            }
            ProcessControlError::UnsupportedOperation => write!(f, "unsupported operation"),
            ProcessControlError::SignalFailed {
                pid,
                operation,
                errno,
                details,
            } => write!(
                f,
                "signal failed for pid {} ({:?}), errno {}: {}",
                pid, operation, errno, details
            ),
        }
    }
}

impl std::error::Error for ProcessControlError {}
