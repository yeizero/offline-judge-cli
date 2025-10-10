use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::time::Instant;
use std::{mem, thread, time::Duration};

use tokio::io::AsyncWriteExt;
use windows::Win32::Foundation::{CloseHandle, E_FAIL, HANDLE, STILL_ACTIVE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, GetExitCodeProcess, OpenProcess, OpenThread,
    PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE, PROCESS_VM_READ, ResumeThread,
    THREAD_SUSPEND_RESUME,
};
use windows::core::{Error as WinError, Result as WinResult};

use win32job::{ExtendedLimitInfo, Job};

use crate::{
    judge::verdict::Limitation,
    monitor::common::{JudgeMonitor, MonitorOutput, TimingStatus, timeout_with_limit},
};
use shared::RawCommand;

pub struct WindowsMonitor<'a> {
    runner: &'a RawCommand,
    input: &'a str,
    limit: &'a Limitation,
}

impl<'a> JudgeMonitor<'a> for WindowsMonitor<'a> {
    async fn load(
        runner: &'a RawCommand,
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
            .build_tokio()?
            .kill_on_drop(true)
            .creation_flags((CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT).0)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let pid = child.id().unwrap();
        assert_ne!(pid, 0);

        let monitor_task = create_memory_monitor(pid)?;

        resume_suspended_child_by_pid(pid)?;

        let start_time = Instant::now();

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(self.input.as_bytes()).await?;
            stdin.flush().await?;
        }

        let output_status = timeout_with_limit(self.limit, child.wait_with_output()).await;
        let elapsed_time = start_time.elapsed();
        let memory = monitor_task.await?;

        output_status.map_result(move |result| {
            let output = result?;
            Ok(MonitorOutput {
                duration: elapsed_time,
                memory,
                status: output.status,
                stderr: output.stderr,
                stdout: output.stdout,
            })
        })
    }
}

pub fn resume_suspended_child_by_pid(pid: u32) -> WinResult<()> {
    let tid = find_main_thread_id(pid)?;
    let thread_handle = unsafe { OpenThread(THREAD_SUSPEND_RESUME, false, tid) }?;

    if thread_handle.is_invalid() {
        return Err(WinError::from_win32());
    }

    let suspend_count = unsafe { ResumeThread(thread_handle) };
    let _ = unsafe { CloseHandle(thread_handle) };

    if suspend_count == u32::MAX {
        return Err(WinError::from_win32());
    }

    Ok(())
}

fn find_main_thread_id(pid: u32) -> WinResult<u32> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0)? };
    if snapshot.is_invalid() {
        return Err(WinError::from_win32());
    }

    let mut te32 = THREADENTRY32 {
        dwSize: mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut result = unsafe { Thread32First(snapshot, &mut te32) };
    let mut thread_id = None;

    while result.is_ok() {
        if te32.th32OwnerProcessID == pid {
            thread_id = Some(te32.th32ThreadID);
            break;
        }
        result = unsafe { Thread32Next(snapshot, &mut te32) };
    }

    let _ = unsafe { CloseHandle(snapshot) };
    thread_id.ok_or_else(|| WinError::new(E_FAIL, "Main thread not found for PID."))
}

const CHECK_MEMORY_INTERVAL: Duration = Duration::from_millis(5);

pub fn create_memory_monitor(pid: u32) -> anyhow::Result<tokio::task::JoinHandle<Option<usize>>> {
    let job = apply_job_for_process(pid)?;

    Ok(tokio::task::spawn_blocking(
        move || monitor_job_memory_usage(job),
    ))
}

fn apply_job_for_process(pid: u32) -> anyhow::Result<Job> {
    let handle = pid_to_process_handle(pid)?;
    let job = Job::create_with_limit_info(ExtendedLimitInfo::new().limit_kill_on_job_close())?;

    job.assign_process(handle.0 as isize)?;
    Ok(job)
}

fn monitor_job_memory_usage(job: Job) -> Option<usize> {
    let mut max_memory = 0;
    let mut handles: HashMap<u32, ProcessHandle> = HashMap::new();

    loop {
        let pids = job.query_process_id_list().ok()?;
        if pids.is_empty() {
            break;
        }

        let current_pids: HashSet<u32> = pids.into_iter().map(|p| p.try_into().unwrap()).collect();

        handles.retain(|&pid, _| current_pids.contains(&pid));

        let mut memory_usage = 0;

        for &pid in &current_pids {
            let handle = handles.entry(pid).or_insert_with(|| {
                ProcessHandle::open(pid).expect("Failed to open process handle")
            });

            memory_usage += get_memory_usage(handle).unwrap_or(0);
        }

        if memory_usage > max_memory {
            max_memory = memory_usage;
        }
        thread::sleep(CHECK_MEMORY_INTERVAL);
    }
    Some(max_memory)
}

fn pid_to_process_handle(pid: u32) -> WinResult<HANDLE> {
    unsafe {
        OpenProcess(
            PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        )
    }
}

fn get_memory_usage(handle: &ProcessHandle) -> Option<usize> {
    if !handle.is_alive() {
        return None;
    }

    let mut pmc = PROCESS_MEMORY_COUNTERS::default();
    let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

    unsafe {
        GetProcessMemoryInfo(handle.raw(), &mut pmc, cb)
            .is_ok()
            .then_some(pmc.PeakWorkingSetSize / 1024)
    }
}

struct ProcessHandle {
    handle: HANDLE,
}

impl ProcessHandle {
    pub fn open(pid: u32) -> anyhow::Result<Self, windows::core::Error> {
        Ok(Self {
            handle: pid_to_process_handle(pid)?,
        })
    }

    pub fn raw(&self) -> HANDLE {
        self.handle
    }

    pub fn is_alive(&self) -> bool {
        unsafe {
            let mut exit_code: u32 = 0;
            GetExitCodeProcess(self.handle, &mut exit_code).is_ok()
                && exit_code == STILL_ACTIVE.0 as u32
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            if !self.handle.is_invalid() {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}
