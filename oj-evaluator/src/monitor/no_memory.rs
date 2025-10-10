use crate::{
    judge::verdict::Limitation,
    monitor::common::{JudgeMonitor, MonitorOutput, TimingStatus, timeout_with_limit},
};
use shared::RawCommand;
use std::{process::Stdio, time::Instant};
use tokio::io::AsyncWriteExt;

pub struct NoMemoryMonitor<'a> {
    runner: &'a RawCommand,
    input: &'a str,
    limit: &'a Limitation,
}

impl<'a> JudgeMonitor<'a> for NoMemoryMonitor<'a> {
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
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let start_time = Instant::now();

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(self.input.as_bytes()).await?;
            stdin.flush().await?;
        }

        let output_status = timeout_with_limit(self.limit, child.wait_with_output()).await;

        let elapsed_time = start_time.elapsed();

        output_status.map_result(move |result| {
            let output = result?;
            Ok(MonitorOutput {
                duration: elapsed_time,
                memory: None,
                status: output.status,
                stderr: output.stderr,
                stdout: output.stdout,
            })
        })
    }
}
