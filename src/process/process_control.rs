use std::fs;

use crate::util::{Pid, ProcessControlError, ProcessControlOperation};

#[derive(Debug, Clone, Copy)]
pub struct ProcessControlResult {
    pub pid: Pid,
    pub operation: ProcessControlOperation,
    pub signal: i32,
}

pub trait TraitProcessControlService {
    fn terminate_process(&self, pid: Pid) -> Result<ProcessControlResult, ProcessControlError>;
    fn force_kill_process(&self, pid: Pid) -> Result<ProcessControlResult, ProcessControlError>;
}

pub struct ProcessControlService;

impl ProcessControlService {
    pub fn new() -> Self {
        ProcessControlService
    }

    fn validate_pid(&self, pid: Pid) -> Result<(), ProcessControlError> {
        if pid == 0 {
            return Err(ProcessControlError::InvalidPid(pid));
        }

        // check for existence before terminating process
        let proc_path = format!("/proc/{}", pid);
        if fs::metadata(proc_path).is_err() {
            return Err(ProcessControlError::ProcessNotFound(pid));
        }

        Ok(())
    }

    fn signal_for_operation(
        &self,
        operation: ProcessControlOperation,
    ) -> Result<i32, ProcessControlError> {
        match operation {
            ProcessControlOperation::Terminate => Ok(libc::SIGTERM),
            ProcessControlOperation::ForceKill => Ok(libc::SIGKILL),
        }
    }

    fn send_signal(
        &self,
        pid: Pid,
        operation: ProcessControlOperation,
    ) -> Result<ProcessControlResult, ProcessControlError> {
        self.validate_pid(pid)?;
        let signal = self.signal_for_operation(operation)?;

        let rc = unsafe { libc::kill(pid as i32, signal) };
        if rc != 0 {
            let errno = std::io::Error::last_os_error().raw_os_error().unwrap_or(-1);
            return Err(ProcessControlError::from_errno(pid, operation, errno));
        }

        Ok(ProcessControlResult {
            pid,
            operation,
            signal,
        })
    }
}

impl TraitProcessControlService for ProcessControlService {
    fn terminate_process(&self, pid: Pid) -> Result<ProcessControlResult, ProcessControlError> {
        self.send_signal(pid, ProcessControlOperation::Terminate)
    }

    fn force_kill_process(&self, pid: Pid) -> Result<ProcessControlResult, ProcessControlError> {
        self.send_signal(pid, ProcessControlOperation::ForceKill)
    }
}

#[cfg(test)]
mod tests {
    use super::{ProcessControlService, TraitProcessControlService};
    use crate::util::ProcessControlError;

    #[test]
    fn reject_pid_zero() {
        let service = ProcessControlService::new();
        let err = service
            .terminate_process(0)
            .expect_err("pid=0 should be rejected");
        assert!(matches!(err, ProcessControlError::InvalidPid(0)));
    }

    #[test]
    fn non_existing_pid_returns_not_found() {
        let service = ProcessControlService::new();

        // max pid would not reach u32::MAX under normal circumstances
        let err = service
            .force_kill_process(u32::MAX)
            .expect_err("non-existing pid should return not found");

        assert!(matches!(err, ProcessControlError::ProcessNotFound(_)));
    }
}
