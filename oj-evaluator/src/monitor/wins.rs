use std::mem;
use std::ops::Deref;
use std::process::Stdio;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_BASIC_LIMIT_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
};
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, OpenProcess, OpenThread,
    PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE, PROCESS_VM_READ, ResumeThread,
    THREAD_SUSPEND_RESUME,
};
use windows::core::{Error as WinError, PCWSTR, Result as WinResult};

use crate::{
    judge::verdict::Limitation,
    monitor::common::{
        JudgeMonitor, MonitorOutput, TimingStatus, collect_child_output, timeout_with_limit,
    },
};
use shared::ShellCommand;

pub struct WindowsMonitor<'a> {
    runner: &'a ShellCommand,
    input: &'a str,
    limit: &'a Limitation,
}

#[async_trait]
impl<'a> JudgeMonitor<'a> for WindowsMonitor<'a> {
    async fn load(
        runner: &'a ShellCommand,
        input: &'a str,
        limit: &'a Limitation,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            runner,
            input,
            limit,
        })
    }

    async fn execute(&mut self) -> anyhow::Result<TimingStatus<MonitorOutput>> {
        let mut child = self
            .runner
            .build_tokio()
            .kill_on_drop(true)
            .creation_flags((CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT).0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        #[expect(
            clippy::unwrap_used,
            reason = "a successfully spawned, unpolled child must have a process ID"
        )]
        let pid = child.id().unwrap();
        assert_ne!(pid, 0);

        let job_object = JobObject::create()?;
        job_object.set_extended_limit_info(&JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                ..Default::default()
            },
            ..Default::default()
        })?;
        job_object.assign_process(&pid_to_process_handle(pid)?)?;

        resume_suspended_child_by_pid(pid)?;

        let start_time = Instant::now();

        let output_status = timeout_with_limit(
            self.limit,
            collect_child_output(&mut child, self.input, self.limit),
        )
        .await;
        let duration = job_object.get_cpu_time().unwrap_or_else(|e| {
            log::debug!("Fallback to wall clock: {e}");
            start_time.elapsed()
        });
        let memory = job_object
            .get_max_memory_usage_kb()
            .inspect_err(|e| log::debug!("Failed to fetch memory: {e}"))
            .ok();

        output_status.map_result(move |result| {
            let output = result?;
            Ok(MonitorOutput {
                duration,
                memory,
                status: output.status,
                stderr: output.stderr,
                stdout: output.stdout,
                stdout_exceeded: output.stdout_exceeded,
                stderr_exceeded: output.stderr_exceeded,
            })
        })
    }
}

pub fn resume_suspended_child_by_pid(pid: u32) -> WinResult<()> {
    let tid = find_main_thread_id(pid)?;
    let thread_handle = OwnedHandle(unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, tid) }?);

    let suspend_count = unsafe { ResumeThread(*thread_handle) };
    if suspend_count == u32::MAX {
        return Err(WinError::from_win32());
    }

    Ok(())
}

fn find_main_thread_id(pid: u32) -> WinResult<u32> {
    let snapshot = OwnedHandle(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)? });
    if snapshot.is_invalid() {
        return Err(WinError::from_win32());
    }

    let mut te32 = THREADENTRY32 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "THREADENTRY32 is a fixed Windows structure smaller than u32::MAX"
        )]
        dwSize: mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut result = unsafe { Thread32First(*snapshot, &raw mut te32) };
    let mut thread_id = None;

    while result.is_ok() {
        if te32.th32OwnerProcessID == pid {
            thread_id = Some(te32.th32ThreadID);
            break;
        }
        result = unsafe { Thread32Next(*snapshot, &raw mut te32) };
    }

    thread_id.ok_or_else(|| WinError::new(E_FAIL, "Main thread not found for PID."))
}

struct JobObject {
    handle: OwnedHandle,
}

unsafe impl Send for JobObject {}

impl JobObject {
    pub fn create() -> WinResult<Self> {
        unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map(|handle| Self {
            handle: OwnedHandle(handle),
        })
    }

    pub fn set_extended_limit_info(
        &self,
        info: &JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    ) -> WinResult<()> {
        unsafe {
            #[expect(clippy::cast_possible_truncation)]
            SetInformationJobObject(
                *self.handle,
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(info).cast(),
                mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
    }

    pub fn assign_process(&self, proc_handle: &OwnedHandle) -> WinResult<()> {
        unsafe { AssignProcessToJobObject(*self.handle, **proc_handle) }
    }

    pub fn get_cpu_time(&self) -> WinResult<Duration> {
        let mut accounting_info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            #[expect(clippy::cast_possible_truncation)]
            QueryInformationJobObject(
                Some(*self.handle),
                JobObjectBasicAccountingInformation,
                std::ptr::from_mut(&mut accounting_info).cast(),
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )?;
        };
        let total_100ns = accounting_info.TotalUserTime + accounting_info.TotalKernelTime;
        #[expect(
            clippy::cast_sign_loss,
            reason = "Windows job CPU accounting times are non-negative"
        )]
        Ok(Duration::from_nanos(total_100ns as u64 * 100))
    }

    pub fn get_max_memory_usage_kb(&self) -> WinResult<usize> {
        let mut accounting_info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        unsafe {
            #[expect(clippy::cast_possible_truncation)]
            QueryInformationJobObject(
                Some(*self.handle),
                JobObjectExtendedLimitInformation,
                std::ptr::from_mut(&mut accounting_info).cast(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                None,
            )?;
        };
        Ok(accounting_info.PeakJobMemoryUsed / 1024)
    }
}

fn pid_to_process_handle(pid: u32) -> WinResult<OwnedHandle> {
    let handle = unsafe {
        OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )?
    };
    if handle.is_invalid() {
        return Err(WinError::from_win32());
    }
    Ok(OwnedHandle(handle))
}

#[repr(transparent)]
struct OwnedHandle(HANDLE);

unsafe impl Send for OwnedHandle {}

impl Deref for OwnedHandle {
    type Target = HANDLE;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::WindowsMonitor;
    use crate::{
        judge::verdict::Limitation,
        monitor::common::{JudgeMonitor, TimingStatus},
    };
    use shared::ShellCommand;
    use std::time::Duration;

    #[tokio::test]
    async fn inherited_open_pipe_cannot_turn_stdout_overflow_into_timeout() {
        let runner = ShellCommand::parse_str(
            r#"powershell.exe -NoProfile -Command "Start-Process powershell.exe -ArgumentList '-NoProfile','-Command','Start-Sleep -Seconds 30' -NoNewWindow; [Console]::Out.Write('12345'); [Console]::Out.Flush(); Start-Sleep -Milliseconds 100""#,
        )
        .unwrap();
        let limit = Limitation {
            max_memory: None,
            max_time: Some(Duration::from_secs(1)),
            max_stdout: 4,
            max_stderr: 1024,
        };
        let mut monitor = WindowsMonitor::load(&runner, "", &limit).await.unwrap();

        let result = tokio::time::timeout(Duration::from_secs(4), monitor.execute())
            .await
            .expect("monitor must not wait for the inherited pipe handle")
            .unwrap();

        let TimingStatus::InTime(output) = result else {
            panic!("stdout overflow must stay OLE rather than becoming TLE");
        };
        assert!(output.stdout_exceeded);
        assert!(output.status.is_none());
    }
}
