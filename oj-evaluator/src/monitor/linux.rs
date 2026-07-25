use anyhow::anyhow;
use async_trait::async_trait;
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
    monitor::common::{
        JudgeMonitor, MonitorOutput, TimingStatus, collect_child_output, terminate_and_reap_child,
        timeout_with_limit,
    },
};
use shared::ShellCommand;
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::time::{sleep, timeout_at};

const CGROUP_CLEANUP_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CGROUP_CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);

pub struct LinuxMonitor<'a> {
    runner: &'a ShellCommand,
    input: &'a str,
    limit: &'a Limitation,
}

#[async_trait]
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
        let child_pid = child
            .id()
            .ok_or_else(|| anyhow!("spawned child process ID is unavailable"))?;

        let output_status = timeout_with_limit(
            self.limit,
            collect_child_output(&mut child, self.input, self.limit),
        )
        .await;
        let elapsed_time = start_time.elapsed();
        let memory = cgroup_job.get_max_memory_usage();
        let duration: std::time::Duration = cgroup_job.get_cpu_time().unwrap_or_else(|| {
            log::debug!("Fallback to wall clock");
            elapsed_time
        });

        cgroup_job.terminate_and_reap(child_pid, &mut child).await?;

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

        let cgroup_result = CgroupBuilder::new(&cgroup_name)
            .memory()
            .done()
            .cpu()
            .done()
            .build(hier);

        match cgroup_result {
            Ok(cgroup) => {
                log::debug!("cgroup ver {}", if cgroup.v2() { "2" } else { "1" });
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

    pub fn get_cpu_time(&self) -> Option<std::time::Duration> {
        let cgroup = self.cgroup.as_ref()?;
        let cpu_controller: &cgroups_rs::cpu::CpuController = cgroup.controller_of()?;
        let cpu_stats = cpu_controller.cpu();

        let usage_usec = cpu_stats
            .stat
            .lines()
            .find(|line| line.starts_with("usage_usec"))
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|value| value.parse::<u64>().ok())?;

        Some(std::time::Duration::from_micros(usage_usec))
    }

    async fn terminate_and_reap(&self, child_pid: u32, child: &mut Child) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + CGROUP_CLEANUP_TIMEOUT;
        if let Err(tree_error) = self.terminate_process_tree(child_pid) {
            if child.id().is_some()
                && let Err(child_error) = terminate_and_reap_child(child).await
            {
                log::warn!(
                    "Direct child cleanup also failed after process-tree termination error: {child_error}"
                );
            }
            return Err(tree_error);
        }

        if child.id().is_some() {
            wait_for_child_exit_until(child, deadline).await?;
        }
        self.wait_until_cgroup_empty(deadline).await
    }

    fn terminate_process_tree(&self, child_pid: u32) -> anyhow::Result<()> {
        let cgroup_result = self.cgroup.as_ref().map_or_else(
            || Err(anyhow!("cgroup is unavailable")),
            kill_cgroup_processes,
        );
        let process_group_result = kill_process_group(child_pid);

        match (cgroup_result, process_group_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Ok(()), Err(error)) => {
                log::debug!("Process-group cleanup fallback failed after cgroup kill: {error}");
                Ok(())
            }
            (Err(error), Ok(())) => {
                log::debug!("Cgroup cleanup unavailable; process group was terminated: {error}");
                Ok(())
            }
            (Err(cgroup_error), Err(group_error)) => Err(anyhow!(
                "failed to terminate cgroup ({cgroup_error}) and process group ({group_error})"
            )),
        }
    }

    async fn wait_until_cgroup_empty(&self, deadline: tokio::time::Instant) -> anyhow::Result<()> {
        let Some(cgroup) = self.cgroup.as_ref() else {
            return Ok(());
        };

        loop {
            if cgroup.tasks().is_empty() {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!(
                    "cgroup still contains processes after {} ms",
                    CGROUP_CLEANUP_TIMEOUT.as_millis()
                ));
            }

            kill_cgroup_processes(cgroup)?;
            sleep(CGROUP_CLEANUP_POLL_INTERVAL).await;
        }
    }

    pub fn spawn_in_cgroup(&self, runner: &ShellCommand) -> anyhow::Result<Child> {
        let mut cmd = runner.build_tokio();

        cmd.kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd.process_group(0);

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
                    if result < 0 {
                        return Err(std::io::Error::last_os_error());
                    }

                    // if setgid(65534) != 0 {
                    //     return Err(std::io::Error::last_os_error());
                    // }
                    // if setuid(65534) != 0 {
                    //     return Err(std::io::Error::last_os_error());
                    // }

                    Ok(())
                });
            };
        }

        let child = cmd.spawn().map_err(anyhow::Error::from)?;
        Ok(child)
    }
}

async fn wait_for_child_exit_until(
    child: &mut Child,
    deadline: tokio::time::Instant,
) -> anyhow::Result<()> {
    timeout_at(deadline, child.wait())
        .await
        .map_err(|_| anyhow!("direct child did not exit before the cleanup deadline"))??;
    Ok(())
}

fn kill_cgroup_processes(cgroup: &Cgroup) -> anyhow::Result<()> {
    if cgroup.v2() {
        match cgroup.kill() {
            Ok(()) => return Ok(()),
            Err(error) => {
                log::debug!(
                    "Atomic cgroup.kill is unavailable; falling back to task enumeration: {error}"
                );
            }
        }
    }

    for task in cgroup.tasks() {
        kill_pid(task.pid)?;
    }
    Ok(())
}

fn kill_process_group(child_pid: u32) -> anyhow::Result<()> {
    let pid = i32::try_from(child_pid)
        .map_err(|_| anyhow!("child PID {child_pid} cannot be represented as a process group"))?;
    kill_raw_pid(-pid)
}

fn kill_pid(pid: u64) -> anyhow::Result<()> {
    let pid = i32::try_from(pid)
        .map_err(|_| anyhow!("cgroup PID {pid} cannot be represented by libc::pid_t"))?;
    kill_raw_pid(pid)
}

fn kill_raw_pid(pid: libc::pid_t) -> anyhow::Result<()> {
    if unsafe { libc::kill(pid, libc::SIGKILL) } == 0 {
        return Ok(());
    }

    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error.into())
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

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{CgroupJob, kill_process_group, wait_for_child_exit_until};
    use std::{path::PathBuf, process::Stdio, time::Duration};
    use tokio::process::Command;
    use tokio::time::{Instant, sleep};

    struct ProcessGroupGuard(u32);

    impl Drop for ProcessGroupGuard {
        fn drop(&mut self) {
            let _ = kill_process_group(self.0);
        }
    }

    #[tokio::test]
    async fn child_wait_respects_cleanup_deadline() {
        let mut child = Command::new("sleep")
            .arg("5")
            .kill_on_drop(true)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("test child must spawn");

        let error =
            wait_for_child_exit_until(&mut child, Instant::now() + Duration::from_millis(10))
                .await
                .expect_err("a running child must exceed the cleanup deadline");
        assert!(
            error
                .to_string()
                .contains("direct child did not exit before the cleanup deadline")
        );

        child
            .start_kill()
            .expect("test child must accept termination");
        child.wait().await.expect("test child must reap after kill");
    }

    #[tokio::test]
    async fn cleanup_reclaims_survivor_after_direct_child_is_reaped() {
        let mut child = Command::new("sh")
            .args(["-c", "sleep 5 &"])
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("test shell must spawn");
        let child_pid = child.id().expect("spawned child must have a PID");
        let _process_group_guard = ProcessGroupGuard(child_pid);
        child.wait().await.expect("test shell must exit");
        assert!(child.id().is_none());

        CgroupJob {
            cgroup: None,
            absolute_path: PathBuf::new(),
        }
        .terminate_and_reap(child_pid, &mut child)
        .await
        .expect("process group cleanup must succeed after direct child reaping");

        let group_pid = i32::try_from(child_pid).expect("test PID must fit in i32");
        let deadline = Instant::now() + Duration::from_millis(500);
        loop {
            if unsafe { libc::kill(-group_pid, 0) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "process group {child_pid} still exists 500 ms after cleanup"
            );
            sleep(Duration::from_millis(10)).await;
        }
    }
}
