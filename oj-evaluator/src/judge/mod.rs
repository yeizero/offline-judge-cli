use shared::ShellCommand;

use crate::judge::comparison::{Comparison, compare};
use crate::judge::verdict::{JudgeStatus, JudgeVerdict, Limitation, TleType};
use crate::monitor::{JudgeMonitor, TimingStatus, load_monitor};

#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::{borrow::Cow, process::ExitStatus};

mod comparison;
pub mod display;
pub mod verdict;

pub async fn evaluate<'case>(
    runner: &ShellCommand,
    input: &'case str,
    ans: &'case str,
    limit: &Limitation,
) -> JudgeVerdict<'case> {
    match evaluate_with_system_error(runner, input, ans, limit).await {
        Ok(verdict) => verdict,
        Err(e) => {
            let mut verdict = JudgeVerdict::new(input);
            verdict.status(JudgeStatus::SE(e));
            verdict
        }
    }
}

async fn evaluate_with_system_error<'case>(
    runner: &ShellCommand,
    input: &'case str,
    ans: &'case str,
    limit: &Limitation,
) -> anyhow::Result<JudgeVerdict<'case>> {
    let mut verdict = JudgeVerdict::new(input);

    let mut monitor = load_monitor(runner, input, limit).await?;

    match monitor.execute().await {
        Ok(TimingStatus::InTime(output)) => {
            verdict.duration(Some(output.duration));
            verdict.memory(output.memory);

            if output.stdout_exceeded {
                verdict.status(JudgeStatus::Ole(limit.max_stdout));
                return Ok(verdict);
            }

            let status = output
                .status
                .ok_or_else(|| anyhow::anyhow!("missing child status after normal completion"))?;

            if let Some(code) = status.code() {
                log::debug!("Exit code: {code}");
            } else {
                #[cfg(unix)]
                if let Some(signal) = status.signal() {
                    log::debug!("Signal: {}", signal);
                }
            }

            let actual_output = match String::from_utf8(output.stdout) {
                Ok(output) => output,
                Err(error) => String::from_utf8_lossy(&error.into_bytes()).into_owned(),
            };
            match compare(actual_output, ans) {
                Comparison::Same => {
                    verdict.status(JudgeStatus::AC);
                }
                Comparison::Diff(diff) => {
                    if let Some(error_msg) = get_error_exit_status_description(status) {
                        let mut error_msg = error_msg.into_owned();
                        let stderr_msg = String::from_utf8_lossy(&output.stderr);
                        let stderr_msg = stderr_msg.trim();

                        if !stderr_msg.is_empty() {
                            error_msg.push('\n');
                            error_msg.push_str(stderr_msg);
                            if output.stderr_exceeded {
                                error_msg.push_str("\n... (stderr truncated)");
                            }
                        }
                        verdict.status(JudgeStatus::RE(error_msg));
                    } else {
                        verdict.status(JudgeStatus::WA(diff));
                    }
                }
            }

            if verdict.is_accept() {
                if let Some(max_time) = limit.max_time
                    && output.duration.as_millis() > max_time.as_millis()
                {
                    verdict.status(JudgeStatus::Tle(TleType::Normal(output.duration)));
                }
                if let Some(max_memory) = limit.max_memory
                    && let Some(memory_usage) = output.memory
                    && memory_usage > max_memory
                {
                    verdict.status(JudgeStatus::Mle(memory_usage));
                }
            }
        }
        Ok(TimingStatus::Aborted(duration)) => {
            verdict.duration(Some(duration));
            verdict.status(JudgeStatus::Tle(TleType::Abort(duration)));
        }
        Err(e) => verdict.status(JudgeStatus::RE(e.to_string())),
    }

    Ok(verdict)
}

fn get_error_exit_status_description(status: ExitStatus) -> Option<Cow<'static, str>> {
    #[cfg(unix)]
    #[cfg(unix)]
    if let Some(signal) = status.signal() {
        use libc;
        let description = match signal {
            libc::SIGSEGV => "Segmentation Fault (SIGSEGV)",
            libc::SIGABRT => "Abort (SIGABRT)",
            libc::SIGFPE => "Floating Point Exception (SIGFPE)",
            libc::SIGILL => "Illegal Instruction (SIGILL)",
            libc::SIGBUS => "Bus Error (SIGBUS)",
            libc::SIGKILL => "Killed (SIGKILL)",
            libc::SIGTERM => "Terminated (SIGTERM)",
            libc::SIGINT => "Interrupted (SIGINT)",
            libc::SIGQUIT => "Quit (SIGQUIT)",
            libc::SIGPIPE => "Broken Pipe (SIGPIPE)",
            _ => return Some(Cow::Owned(format!("Terminated by signal {signal}"))),
        };
        return Some(Cow::Borrowed(description));
    }

    if let Some(code) = status.code() {
        #[cfg(windows)]
        {
            let description = match code.cast_unsigned() {
                0xC000_0005 => "Access Violation",
                0xC000_0094 => "Divide by Zero",
                0xC000_00FD => "Stack Overflow",
                0xC000_001D => "Illegal Instruction",
                0 => return None,
                _ => return Some(Cow::Owned(format!("Application Exit Code: {code}"))),
            };
            Some(Cow::Borrowed(description))
        }

        #[cfg(not(windows))]
        match code {
            0 => None,
            126 => Some(Cow::Borrowed("Command found but not executable")),
            127 => Some(Cow::Borrowed("Command not found")),
            _ => Some(Cow::Owned(format!("Application Exit Code: {}", code))),
        }
    } else {
        #[cfg(unix)]
        {
            Some(Cow::Borrowed(
                "Terminated by unknown cause (No code, no signal)",
            ))
        }
        #[cfg(windows)]
        {
            unreachable!();
        }
        #[cfg(not(any(windows, unix)))]
        {
            Some(Cow::Borrowed("Terminated by unknown cause (No code)"))
        }
    }
}
