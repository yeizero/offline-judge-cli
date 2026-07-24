use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

use owo_colors::OwoColorize;

use crate::judge::comparison::StyledDiff;
use crate::utils::PrettyNumber;
use std::cmp::max;

#[derive(Debug, Clone, Copy)]
pub struct Limitation {
    pub max_memory: Option<usize>,
    pub max_time: Option<Duration>,
}

impl Limitation {
    pub const fn max_memory(&mut self, max_memory: Option<usize>) -> &mut Self {
        self.max_memory = max_memory;
        self
    }
    pub const fn max_time(&mut self, max_time: Option<Duration>) -> &mut Self {
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
            status: JudgeStatus::SE(anyhow::anyhow!("status is not handed")),
            input,
            duration: None,
            memory: None,
        }
    }
    pub const fn is_accept(&self) -> bool {
        self.status.is_accept()
    }
    pub(super) fn status(&mut self, status: JudgeStatus) {
        self.status = status;
    }
    pub(super) const fn duration(&mut self, duration: Option<Duration>) {
        self.duration = duration;
    }
    pub(super) const fn memory(&mut self, memory: Option<usize>) {
        self.memory = memory;
    }
}

#[derive(Debug)]
pub enum JudgeStatus {
    /// Accept
    AC,
    /// Runtime Error
    RE(String),
    /// System Error
    SE(anyhow::Error),
    /// Wrong Answer
    WA(StyledDiff),
    /// Time Limit Exceeded
    Tle(TleType),
    /// Memory Limit Exceeded
    Mle(usize),
}

impl JudgeStatus {
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::AC)
    }

    pub const fn to_str_short(&self) -> &str {
        match self {
            Self::RE(_) => "運行時錯誤 RE",
            Self::SE(_) => "系統錯誤 RE",
            Self::WA(_) => "答案錯誤 WA",
            Self::Tle(_) => "超時錯誤 TLE",
            Self::Mle(_) => "記憶體超限 MLE",
            Self::AC => "答案正確 AC",
        }
    }

    pub(crate) const fn severity(&self) -> u8 {
        match self {
            Self::SE(_) => 5,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum TleType {
    Abort(Duration),
    Normal(Duration),
}

impl fmt::Display for TleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Normal(time) => write!(f, "{} ms", time.as_millis().prettify()),
            Self::Abort(time) => write!(f, "{} ms aborted", time.as_millis().prettify()),
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

impl fmt::Display for CompileError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::SE(msg) => write!(f, "系統錯誤 (SE): {msg}"),
            Self::CE(msg) => write!(f, "編譯錯誤 (CE): {msg}"),
        }
    }
}

impl std::error::Error for CompileError<'_> {}

pub struct SummaryInfo {
    pub success_rounds: usize,
    pub current_rounds: usize,
    pub max_time: Duration,
    pub max_memory: usize,
    worse_status: JudgeStatus,
}

impl Default for SummaryInfo {
    fn default() -> Self {
        Self {
            success_rounds: 0,
            current_rounds: 0,
            max_time: Duration::ZERO,
            max_memory: 0,
            worse_status: JudgeStatus::AC,
        }
    }
}

impl SummaryInfo {
    pub fn update(&mut self, verdict: JudgeVerdict) {
        self.current_rounds += 1;
        self.max_time = max(self.max_time, verdict.duration.unwrap_or(Duration::ZERO));
        self.max_memory = max(self.max_memory, verdict.memory.unwrap_or(0));
        if verdict.is_accept() {
            self.success_rounds += 1;
        } else if verdict.status.is_severe_than(&self.worse_status) {
            self.worse_status = verdict.status;
        }
    }
    pub const fn score(&self) -> usize {
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
            status @ JudgeStatus::Tle(time) => write!(f, "{} ({})", status.to_str_short(), time),
            status @ JudgeStatus::Mle(memory) => {
                write!(f, "{} ({} KiB)", status.to_str_short(), memory.prettify())
            }
            JudgeStatus::AC => write!(
                f,
                "{} ({} ms, {} KiB)",
                JudgeStatus::AC.to_str_short().bright_green(),
                self.max_time.as_millis(),
                self.max_memory
            ),
            status => write!(f, "{}", status.to_str_short()),
        }
    }
}
