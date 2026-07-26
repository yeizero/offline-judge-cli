use std::borrow::Cow;
use std::fmt;
use std::time::Duration;

use owo_colors::OwoColorize;
use shared::tr;

use crate::judge::comparison::WrongAnswer;
use crate::utils::PrettyNumber;
use std::cmp::max;

#[expect(
    clippy::struct_field_names,
    reason = "the public Limitation API intentionally uses max_* names for all limits"
)]
#[derive(Debug, Clone, Copy)]
pub struct Limitation {
    pub max_memory: Option<usize>,
    pub max_time: Option<Duration>,
    pub max_stdout: usize,
    pub max_stderr: usize,
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
    pub const fn max_stdout(&mut self, max_stdout: usize) -> &mut Self {
        self.max_stdout = max_stdout;
        self
    }
    pub const fn max_stderr(&mut self, max_stderr: usize) -> &mut Self {
        self.max_stderr = max_stderr;
        self
    }
}

impl Default for Limitation {
    fn default() -> Self {
        Self {
            max_memory: Some(1024 * 1024),
            max_time: Some(Duration::from_secs(2)),
            max_stdout: 16 * 1024 * 1024,
            max_stderr: 16 * 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct JudgeVerdict<'a> {
    pub status: JudgeStatus<'a>,
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
    pub(super) fn status(&mut self, status: JudgeStatus<'a>) {
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
pub enum JudgeStatus<'case> {
    /// Accept
    AC,
    /// Runtime Error
    RE(String),
    /// System Error
    SE(anyhow::Error),
    /// Wrong Answer
    WA(WrongAnswer<'case>),
    /// Output Limit Exceeded
    Ole(usize),
    /// Time Limit Exceeded
    Tle(TleType),
    /// Memory Limit Exceeded
    Mle(usize),
}

impl JudgeStatus<'_> {
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::AC)
    }

    pub fn to_str_short(&self) -> &str {
        SummaryStatus::from_status(self).to_str_short()
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

pub struct SummaryInfo {
    pub success_rounds: usize,
    pub current_rounds: usize,
    pub max_time: Duration,
    pub max_memory: usize,
    worst_status: SummaryStatus,
}

#[derive(Debug, Clone, Copy)]
enum SummaryStatus {
    AC,
    RE,
    SE,
    WA,
    Ole(usize),
    Tle(TleType),
    Mle(usize),
}

impl SummaryStatus {
    const fn from_status(status: &JudgeStatus<'_>) -> Self {
        match status {
            JudgeStatus::AC => Self::AC,
            JudgeStatus::RE(_) => Self::RE,
            JudgeStatus::SE(_) => Self::SE,
            JudgeStatus::WA(_) => Self::WA,
            JudgeStatus::Ole(limit) => Self::Ole(*limit),
            JudgeStatus::Tle(time) => Self::Tle(*time),
            JudgeStatus::Mle(memory) => Self::Mle(*memory),
        }
    }

    fn to_str_short(self) -> &'static str {
        match self {
            Self::RE => tr!(VerdictRE),
            Self::SE => tr!(VerdictSE),
            Self::WA => tr!(VerdictWA),
            Self::Ole(_) => tr!(VerdictOLE),
            Self::Tle(_) => tr!(VerdictTLE),
            Self::Mle(_) => tr!(VerdictMLE),
            Self::AC => tr!(VerdictAC),
        }
    }

    const fn severity(self) -> u8 {
        match self {
            Self::SE => 6,
            Self::RE => 5,
            Self::WA => 4,
            Self::Ole(_) => 3,
            Self::Tle(_) => 2,
            Self::Mle(_) => 1,
            Self::AC => 0,
        }
    }

    fn is_severe_than(self, other: Self) -> bool {
        if self.severity() != other.severity() {
            return self.severity() > other.severity();
        }

        match (self, other) {
            (Self::Tle(self_time), Self::Tle(other_time)) => self_time > other_time,
            (Self::Mle(self_mem), Self::Mle(other_mem)) => self_mem > other_mem,
            _ => false,
        }
    }
}

impl Default for SummaryInfo {
    fn default() -> Self {
        Self {
            success_rounds: 0,
            current_rounds: 0,
            max_time: Duration::ZERO,
            max_memory: 0,
            worst_status: SummaryStatus::AC,
        }
    }
}

impl SummaryInfo {
    pub fn update(&mut self, verdict: &JudgeVerdict<'_>) {
        self.current_rounds += 1;
        self.max_time = max(self.max_time, verdict.duration.unwrap_or(Duration::ZERO));
        self.max_memory = max(self.max_memory, verdict.memory.unwrap_or(0));
        if verdict.is_accept() {
            self.success_rounds += 1;
        } else {
            let status = SummaryStatus::from_status(&verdict.status);
            if status.is_severe_than(self.worst_status) {
                self.worst_status = status;
            }
        }
    }
    pub const fn score(&self) -> usize {
        self.success_rounds * 100 / self.current_rounds
    }
}

impl fmt::Display for SummaryInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.worst_status {
            status @ SummaryStatus::WA => {
                write!(
                    f,
                    "{} (score: {}%)",
                    if self.current_rounds > 1 {
                        tr!(VerdictNA)
                    } else {
                        status.to_str_short()
                    },
                    self.score()
                )
            }
            status @ SummaryStatus::Tle(time) => write!(f, "{} ({})", status.to_str_short(), time),
            status @ SummaryStatus::Mle(memory) => {
                write!(f, "{} ({} KiB)", status.to_str_short(), memory.prettify())
            }
            status @ SummaryStatus::Ole(limit) => {
                write!(
                    f,
                    "{} (limit: {} bytes)",
                    status.to_str_short(),
                    limit.prettify()
                )
            }
            SummaryStatus::AC => write!(
                f,
                "{} ({} ms, {} KiB)",
                SummaryStatus::AC.to_str_short().bright_green(),
                self.max_time.as_millis(),
                self.max_memory
            ),
            status => write!(f, "{}", status.to_str_short()),
        }
    }
}
