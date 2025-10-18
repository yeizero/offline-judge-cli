use anyhow::anyhow;
use cgroups_rs::Cgroup;
use cgroups_rs::cgroup_builder::CgroupBuilder;
use cgroups_rs::hierarchies;
use cgroups_rs::memory::MemController;
use rand::Rng;
use std::fs::OpenOptions;
use std::os::fd::IntoRawFd;
use std::path::PathBuf;
use tokio::process::Child;

use crate::{
    judge::verdict::Limitation,
    monitor::common::{JudgeMonitor, MonitorOutput, TimingStatus, timeout_with_limit},
};
use shared::ShellCommand;
use std::{process::Stdio, time::Instant};
use tokio::io::AsyncWriteExt;

pub struct LinuxMonitor<'a> {
    runner: &'a ShellCommand,
    input: &'a str,
    limit: &'a Limitation,
}

impl<'a> JudgeMonitor<'a> for LinuxMonitor<'a> {
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
        let cgroup_job = CgroupJob::empty()?;

        let start_time = Instant::now();
        let mut child = cgroup_job.spawn_in_cgroup(self.runner)?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(self.input.as_bytes()).await?;
            stdin.flush().await?;
        }

        let output_status = timeout_with_limit(self.limit, child.wait_with_output()).await;
        let elapsed_time = start_time.elapsed();
        let memory = cgroup_job.get_max_memory_usage();

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

struct CgroupJob {
    cgroup: Option<Cgroup>,
    absolute_path: PathBuf,
}

impl CgroupJob {
    pub fn empty() -> anyhow::Result<Self> {
        let random_suffix: u32 = rand::rng().random();
        let cgroup_name = format!("offline-judge-{random_suffix}");
        let hier = hierarchies::auto();
        let mut full_path = hier.root();

        let cgroup_result = CgroupBuilder::new(&cgroup_name).memory().done().build(hier);

        match cgroup_result {
            Ok(cgroup) => {
                full_path.push(cgroup.path());
                Ok(Self {
                    cgroup: Some(cgroup),
                    absolute_path: full_path,
                })
            }
            Err(e) => {
                let mut current_err: Option<&(dyn std::error::Error + 'static)> = Some(&e);
                let mut is_permission_denied = false;

                while let Some(err) = current_err {
                    if let Some(io_err) = err.downcast_ref::<std::io::Error>()
                        && io_err.kind() == std::io::ErrorKind::PermissionDenied
                    {
                        is_permission_denied = true;
                        break;
                    }
                    current_err = err.source();
                }

                if is_permission_denied {
                    log::warn!(
                        "資源限制監控未啟用：權限不足。請使用 Root 權限或確保 Cgroup 檔案系統有足夠權限。"
                    );
                } else {
                    log::warn!("無法啟用資源限制監控，原因: {}", e);
                }

                Ok(Self {
                    cgroup: None,
                    absolute_path: PathBuf::new(),
                })
            }
        }
    }

    pub fn get_max_memory_usage(&self) -> Option<usize> {
        let cgroup = self.cgroup.as_ref()?;
        let mem_controller: &MemController = cgroup.controller_of().unwrap();
        let max_usage_in_bytes = mem_controller.memory_stat().max_usage_in_bytes;
        (max_usage_in_bytes / 1024).try_into().ok()
    }

    pub fn spawn_in_cgroup(&self, runner: &ShellCommand) -> anyhow::Result<Child> {
        let mut cmd = runner.build_tokio();

        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(ref cgroup) = self.cgroup {
            let path = &self.absolute_path;
            let tasks_file_name = if cgroup.v2() { "cgroup.procs" } else { "tasks" };
            let cgroup_tasks_path = path.join(tasks_file_name);

            if !cgroup_tasks_path.exists() {
                return Err(anyhow!(
                    "Cgroup tasks file does not exist: {:?}",
                    cgroup_tasks_path
                ));
            }

            let tasks_file = OpenOptions::new().write(true).open(&cgroup_tasks_path)?;
            let tasks_fd = tasks_file.into_raw_fd();

            unsafe {
                cmd.pre_exec(move || {
                    use libc::{close, getpid, write};

                    let current_pid = getpid();
                    let mut buf = itoa::Buffer::new();
                    let pid_bytes = buf.format(current_pid).as_bytes();

                    let mut buf2 = [0u8; 12];
                    buf2[..pid_bytes.len()].copy_from_slice(pid_bytes);
                    buf2[pid_bytes.len()] = b'\n';

                    let result = write(tasks_fd, buf2.as_ptr() as *const _, pid_bytes.len() + 1);

                    let _ = close(tasks_fd);

                    if result >= 0 {
                        Ok(())
                    } else {
                        Err(std::io::Error::last_os_error())
                    }
                });
            };
        }

        let child = cmd.spawn().map_err(anyhow::Error::from)?;
        Ok(child)
    }
}

impl Drop for CgroupJob {
    fn drop(&mut self) {
        if let Some(ref cgroup) = self.cgroup
            && let Err(e) = cgroup.delete()
        {
            log::warn!("Failed to delete cgroup '{}': {}", cgroup.path(), e);
        }
    }
}
