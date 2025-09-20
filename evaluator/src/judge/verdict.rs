use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

use owo_colors::OwoColorize;

use crate::judge::comparison::StyledDiff;
use crate::utils::PrettyNumber;

pub struct Limitation {
    pub(super) max_memory: Option<usize>,
    pub(super) max_time: Option<Duration>,
}

impl Limitation {
    pub fn max_memory(&mut self, max_memory: Option<usize>) -> &mut Self {
        self.max_memory = max_memory;
        self
    }
    pub fn max_time(&mut self, max_time: Option<Duration>) -> &mut Self {
        self.max_time = max_time;
        self
    }
}

impl Default for Limitation {
    fn default() -> Self {
        Self {
            max_memory: Some(1024 * 1024),
            max_time: Some(Duration::from_secs(2)),
        }
    }
}

#[derive(Debug)]
pub struct JudgeVerdict<'a> {
    pub status: JudgeStatus,
    pub input: &'a str,
    pub duration: Option<Duration>,
    pub memory: Option<usize>,
}

impl<'a> JudgeVerdict<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            status: JudgeStatus::RE("Failed".to_owned()),
            input,
            duration: None,
            memory: None,
        }
    }
    pub fn is_accept(&self) -> bool {
        self.status.is_accept()
    }
    pub(super) fn status(&mut self, status: JudgeStatus) {
        self.status = status;
    }
    pub(super) fn duration(&mut self, duration: Option<Duration>) {
        self.duration = duration;
    }
    pub(super) fn memory(&mut self, memory: Option<usize>) {
        self.memory = memory;
    }
}

#[derive(Debug)]
pub enum JudgeStatus {
    /// Accept
    AC,
    /// Runtime Error
    RE(String),
    /// Wrong Answer
    WA(StyledDiff),
    /// Time Limit Exceeded
    Tle(Duration),
    /// Memory Limit Exceeded
    Mle(usize),
}

impl JudgeStatus {
    pub fn is_accept(&self) -> bool {
        matches!(self, Self::AC)
    }

    pub fn to_str_short(&self) -> &str {
        match self {
            Self::RE(_) => "運行時錯誤 RE",
            Self::WA(_) => "答案錯誤 WA",
            Self::Tle(_) => "超時錯誤 TLE",
            Self::Mle(_) => "記憶體超限 MLE",
            Self::AC => "答案正確 AC",
        }
    }

    pub(crate) fn severity(&self) -> u8 {
        match self {
            Self::RE(_) => 4,
            Self::WA(_) => 3,
            Self::Tle(_) => 2,
            Self::Mle(_) => 1,
            Self::AC => 0,
        }
    }

    pub(crate) fn is_severe_than(&self, other: &Self) -> bool {
        let severity_self = self.severity();
        let severity_other = other.severity();

        if severity_self != severity_other {
            return severity_self > severity_other;
        }

        match (self, other) {
            (Self::Tle(self_time), Self::Tle(other_time)) => self_time > other_time,
            (Self::Mle(self_mem), Self::Mle(other_mem)) => self_mem > other_mem,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub enum CompileError<'a> {
    /// System Error
    SE(Cow<'a, str>),
    /// Compilation Error
    CE(Cow<'a, str>),
}

impl<'a> fmt::Display for CompileError<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SE(msg) => write!(f, "系統錯誤 (SE): {msg}"),
            Self::CE(msg) => write!(f, "編譯錯誤 (CE): {msg}"),
        }
    }
}

impl<'a> std::error::Error for CompileError<'a> {}

pub struct SummaryInfo {
    pub success_rounds: usize,
    pub current_rounds: usize,
    pub total_time: Duration,
    pub total_memory: usize,
    worse_status: JudgeStatus,
}

impl Default for SummaryInfo {
    fn default() -> Self {
        Self {
            success_rounds: 0,
            current_rounds: 0,
            total_time: Duration::ZERO,
            total_memory: 0,
            worse_status: JudgeStatus::AC,
        }
    }
}

impl SummaryInfo {
    pub fn update(&mut self, verdict: JudgeVerdict) {
        self.current_rounds += 1;
        if let Some(duration) = verdict.duration {
            self.total_time += duration;
        }
        if let Some(memory) = verdict.memory {
            self.total_memory += memory;
        }
        if verdict.is_accept() {
            self.success_rounds += 1;
        } else if verdict.status.is_severe_than(&self.worse_status) {
            self.worse_status = verdict.status;
        }
    }
    pub fn score(&self) -> usize {
        self.success_rounds * 100 / self.current_rounds
    }
}

impl fmt::Display for SummaryInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.worse_status {
            status @ JudgeStatus::WA(_) => {
                write!(
                    f,
                    "{} (score: {}%)",
                    if self.current_rounds > 1 {
                        "答案不正確 NA"
                    } else {
                        status.to_str_short()
                    },
                    self.score()
                )
            }
            status @ JudgeStatus::Tle(time) => write!(
                f,
                "{} ({} ms)",
                status.to_str_short(),
                time.as_millis().prettify()
            ),
            status @ JudgeStatus::Mle(memory) => {
                write!(f, "{} ({} KiB)", status.to_str_short(), memory.prettify())
            }
            JudgeStatus::AC => write!(
                f,
                "{} ({} ms, {} KiB)",
                JudgeStatus::AC.to_str_short().bright_green(),
                self.total_time.as_millis() / self.current_rounds as u128,
                self.total_memory / self.current_rounds
            ),
            status => write!(f, "{}", status.to_str_short()),
        }
    }
}
