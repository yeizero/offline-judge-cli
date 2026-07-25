use crate::{
    judge::verdict::Limitation,
    monitor::common::{
        JudgeMonitor, MonitorOutput, TimingStatus, collect_child_output, terminate_and_reap_child,
        timeout_with_limit,
    },
};
use async_trait::async_trait;
use shared::ShellCommand;
use std::{process::Stdio, time::Instant};

pub struct NoMemoryMonitor<'a> {
    runner: &'a ShellCommand,
    input: &'a str,
    limit: &'a Limitation,
}

#[async_trait]
impl<'a> JudgeMonitor<'a> for NoMemoryMonitor<'a> {
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
        let mut command = self.runner.build_tokio();
        command
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;

        let start_time = Instant::now();

        let output_status = timeout_with_limit(
            self.limit,
            collect_child_output(&mut child, self.input, self.limit),
        )
        .await;
        terminate_fallback_tree_and_reap(&mut child).await?;

        let elapsed_time = start_time.elapsed();

        output_status.map_result(move |result| {
            let output = result?;
            Ok(MonitorOutput {
                duration: elapsed_time,
                memory: None,
                status: output.status,
                stderr: output.stderr,
                stdout: output.stdout,
                stdout_exceeded: output.stdout_exceeded,
                stderr_exceeded: output.stderr_exceeded,
            })
        })
    }
}

async fn terminate_fallback_tree_and_reap(
    child: &mut tokio::process::Child,
) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        let pid = child.id().ok_or_else(|| {
            std::io::Error::other("child process ID is unavailable during cleanup")
        })?;
        let pid = i32::try_from(pid).map_err(|_| {
            std::io::Error::other(format!(
                "child PID {pid} cannot be represented as a process group"
            ))
        })?;

        if unsafe { libc::kill(-pid, libc::SIGKILL) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                if let Err(child_error) = terminate_and_reap_child(child).await {
                    log::warn!(
                        "Direct child cleanup also failed after process-group termination error: {child_error}"
                    );
                }
                return Err(error);
            }
        }

        child.wait().await.map(|_| ())
    }

    #[cfg(not(unix))]
    {
        terminate_and_reap_child(child).await
    }
}
