use std::thread;
use std::time::Duration;
use win32job::{ExtendedLimitInfo, Job};
use windows::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
use windows::Win32::System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SET_QUOTA, PROCESS_TERMINATE, PROCESS_VM_READ};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};

const CHECK_MEMORY_INTERVAL: Duration = Duration::from_millis(5);

pub fn create_memory_monitor(pid: u32) -> Box<dyn FnOnce() -> Option<usize>> {
    let job = match apply_job_for_process(pid) {
        Ok(job) => job,
        Err(e) => {
            log::warn!("無法取得記憶體使用量: {}", e);
            return Box::new(|| None);
        }
    };

    let monitor_thread = std::thread::spawn(move || {
        monitor_job_memory_usage(job)
      });
    Box::new(|| { monitor_thread.join().unwrap() })
}

fn apply_job_for_process(pid: u32) -> Result<Job, Box<dyn std::error::Error>> {
    let handle = pid_to_handle(pid)?;
    let job = Job::create_with_limit_info(
        ExtendedLimitInfo::new()
            .limit_kill_on_job_close()
    )?;
    
    job.assign_process(handle.0)?;
    Ok(job)
}

fn monitor_job_memory_usage(job: Job) -> Option<usize> {
    let mut max_memory_usage = 0;
    loop {
        let pids = match job.query_process_id_list() {
            Ok(list) => list,
            Err(e) => {
                log::warn!("Failed to query job info: {}", e);
                return None;
            }
        };

        if pids.is_empty() {
            break;
        }

        let memory_usage: usize = pids.iter().map(|&pid| {
            let handle_result = ProcessHandle::open(pid.try_into().unwrap());
            let handle = match handle_result {
                Ok(handle) => handle,
                Err(e) => {
                    log::warn!("Failed to open process handle: {}", e);
                    return 0;
                }
            };
            get_memory_usage(&handle).unwrap_or(0)
        }).sum();

        if memory_usage > max_memory_usage {
            max_memory_usage = memory_usage;
        }
        
        thread::sleep(CHECK_MEMORY_INTERVAL);
    }
    Some(max_memory_usage)
}

fn pid_to_handle(pid: u32) -> Result<HANDLE, windows::core::Error> {
    unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) }
}

fn get_memory_usage(handle: &ProcessHandle) -> Option<usize> {
  if !handle.is_alive() {
      return None;
  }

  let process_handle = handle.raw();

  let mut pmc = PROCESS_MEMORY_COUNTERS::default();
  let cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;

  if unsafe { GetProcessMemoryInfo(process_handle, &mut pmc, cb) }.is_ok() {
      let memory_usage_bytes = pmc.PeakWorkingSetSize;
      let memory_usage_kb = memory_usage_bytes / 1024;
      Some(memory_usage_kb)
  } else {
      log::warn!("呼叫 GetProcessMemoryInfo 失敗");
      None
  }
}

struct ProcessHandle {
    handle: HANDLE,
}

impl ProcessHandle {
    pub fn open(pid: u32) -> Result<Self, windows::core::Error> {
        unsafe {
            Ok(Self {
                handle: OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                    false,
                    pid,
                )?,
            })
        }
    }

    pub fn raw(&self) -> HANDLE {
        self.handle
    }

    pub fn is_alive(&self) -> bool {
        unsafe {
            let mut exit_code: u32 = 0;
            if GetExitCodeProcess(self.handle, &mut exit_code).is_ok() {
                exit_code == STILL_ACTIVE.0 as u32
            } else {
                false
            }
        }
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        unsafe {
            if self.handle.0 != 0 {
                let _ = CloseHandle(self.handle);
            }
        }
    }
}