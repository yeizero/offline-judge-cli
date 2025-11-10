use crate::judge::verdict::Limitation;
use async_trait::async_trait;
use shared::ShellCommand;
use std::{process::ExitStatus, time::Duration};
use tokio::time::timeout;

#[async_trait]
pub trait JudgeMonitor<'a>: Sized {
    /// Err as system error
    async fn load(
        runner: &'a ShellCommand,
        input: &'a str,
        limit: &'a Limitation,
    ) -> anyhow::Result<Self>;
    /// Err as runtime error
    async fn execute(&mut self) -> anyhow::Result<TimingStatus<MonitorOutput>>;
}

pub struct MonitorOutput {
    pub duration: Duration,
    pub memory: Option<usize>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub status: ExitStatus,
}

pub enum TimingStatus<T> {
    /// InTime means the program didn't abort, but it might still TLE.
    InTime(T),
    Aborted(Duration),
}

impl<T> TimingStatus<T> {
    pub fn map_result<R, E, F>(self, f: F) -> Result<TimingStatus<R>, E>
    where
        F: FnOnce(T) -> Result<R, E>,
    {
        match self {
            Self::InTime(value) => match f(value) {
                Ok(new_value) => Ok(TimingStatus::InTime(new_value)),
                Err(e) => Err(e)
            },
            Self::Aborted(d) => Ok(TimingStatus::Aborted(d)),
        }
    }
}

pub async fn timeout_with_limit<F>(limit: &Limitation, future: F) -> TimingStatus<F::Output>
where
    F: IntoFuture,
{
    match limit.max_time {
        Some(duration) => {
            let abort_duartion = Duration::from_millis(duration.as_millis() as u64 * 3 / 2);
            match timeout(abort_duartion, future.into_future()).await {
                Ok(result) => TimingStatus::InTime(result),
                Err(_) => TimingStatus::Aborted(abort_duartion),
            }
        }
        None => {
            let result = future.into_future().await;
            TimingStatus::InTime(result)
        }
    }
}