use crate::judge::verdict::Limitation;
use shared::ShellCommand;
mod common;
pub use common::{JudgeMonitor, TimingStatus};

#[cfg(target_os = "windows")]
mod wins;
#[cfg(target_os = "windows")]
pub use wins::WindowsMonitor as Monitor;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxMonitor as Monitor;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod no_memory;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use no_memory::NoMemoryMonitor as Monitor;

pub async fn load_monitor<'a>(
    runner: &'a ShellCommand,
    input: &'a str,
    limit: &'a Limitation,
) -> anyhow::Result<impl JudgeMonitor<'a>> {
    Monitor::load(runner, input, limit).await
}
