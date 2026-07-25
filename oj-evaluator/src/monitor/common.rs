use crate::judge::verdict::Limitation;
use async_trait::async_trait;
use shared::ShellCommand;
use std::{ops::Div, process::ExitStatus, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Child;
use tokio::time::timeout;

const READ_BUFFER_SIZE: usize = 64 * 1024;
pub struct CappedOutput {
    pub bytes: Vec<u8>,
    pub exceeded: bool,
}

async fn read_capped_with_policy<R>(
    mut reader: R,
    limit: usize,
    stop_on_overflow: bool,
) -> std::io::Result<CappedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut buffer = vec![0u8; READ_BUFFER_SIZE];
    let mut exceeded = false;

    loop {
        let count = reader.read(&mut buffer).await?;
        if count == 0 {
            break;
        }

        let retained = limit.saturating_sub(bytes.len()).min(count);
        bytes.extend_from_slice(&buffer[..retained]);

        if retained < count && !exceeded {
            exceeded = true;
            if stop_on_overflow {
                break;
            }
        }
    }

    Ok(CappedOutput { bytes, exceeded })
}

struct CollectedStreams {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stdout_exceeded: bool,
    stderr_exceeded: bool,
}

pub struct CollectedOutput {
    pub status: Option<ExitStatus>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_exceeded: bool,
    pub stderr_exceeded: bool,
}

trait ChildControl {
    #[cfg(any(not(windows), test))]
    fn start_kill(&mut self) -> std::io::Result<()>;
    async fn wait(&mut self) -> std::io::Result<ExitStatus>;
}

impl ChildControl for Child {
    #[cfg(any(not(windows), test))]
    fn start_kill(&mut self) -> std::io::Result<()> {
        self.start_kill()
    }

    async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.wait().await
    }
}

#[cfg(not(windows))]
pub async fn terminate_and_reap_child(child: &mut Child) -> std::io::Result<()> {
    terminate_and_reap(child).await
}

#[cfg(any(not(windows), test))]
async fn terminate_and_reap<C>(child: &mut C) -> std::io::Result<()>
where
    C: ChildControl,
{
    match child.start_kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(error),
    }

    child.wait().await.map(|_| ())
}

pub async fn collect_child_output(
    child: &mut Child,
    input: &str,
    limit: &Limitation,
) -> std::io::Result<CollectedOutput> {
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| std::io::Error::other("child stdin was not piped"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| std::io::Error::other("child stdout was not piped"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| std::io::Error::other("child stderr was not piped"))?;

    let output = collect_piped_output(
        stdin,
        stdout,
        stderr,
        input,
        limit.max_stdout,
        limit.max_stderr,
    )
    .await;

    finish_child_collection(child, output).await
}

async fn finish_child_collection<C>(
    child: &mut C,
    output: std::io::Result<CollectedStreams>,
) -> std::io::Result<CollectedOutput>
where
    C: ChildControl,
{
    let output = match output {
        Ok(output) if output.stdout_exceeded => {
            return Ok(CollectedOutput {
                status: None,
                stdout: output.stdout,
                stderr: output.stderr,
                stdout_exceeded: true,
                stderr_exceeded: output.stderr_exceeded,
            });
        }
        output => output,
    };

    let status = Some(child.wait().await?);
    let output = output?;
    Ok(CollectedOutput {
        status,
        stdout: output.stdout,
        stderr: output.stderr,
        stdout_exceeded: false,
        stderr_exceeded: output.stderr_exceeded,
    })
}

async fn collect_piped_output<W, O, E>(
    mut stdin: W,
    stdout: O,
    stderr: E,
    input: &str,
    max_stdout: usize,
    max_stderr: usize,
) -> std::io::Result<CollectedStreams>
where
    W: AsyncWrite + Unpin,
    O: AsyncRead + Unpin,
    E: AsyncRead + Unpin,
{
    let write = async move {
        let result = stdin.write_all(input.as_bytes()).await;
        if result.is_ok() {
            let _ = stdin.flush().await;
        }
        drop(stdin);
    };
    let stdout = read_capped_with_policy(stdout, max_stdout, true);
    let stderr = read_capped_with_policy(stderr, max_stderr, false);
    tokio::pin!(write, stdout, stderr);

    let mut write_finished = false;
    let mut stderr_result = None;
    let stdout = loop {
        tokio::select! {
            result = &mut stdout => break result,
            () = &mut write, if !write_finished => write_finished = true,
            result = &mut stderr, if stderr_result.is_none() => stderr_result = Some(result),
        }
    };

    if stdout.as_ref().is_ok_and(|stdout| stdout.exceeded) {
        let stdout = stdout?;
        return Ok(CollectedStreams {
            stdout: stdout.bytes,
            stderr: Vec::new(),
            stdout_exceeded: true,
            stderr_exceeded: false,
        });
    }

    let finish_write = async {
        if !write_finished {
            write.await;
        }
    };
    let finish_stderr = async {
        match stderr_result {
            Some(result) => result,
            None => stderr.await,
        }
    };
    let ((), stderr) = tokio::join!(finish_write, finish_stderr);
    let stdout = stdout?;
    let stderr = stderr?;

    Ok(CollectedStreams {
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_exceeded: false,
        stderr_exceeded: stderr.exceeded,
    })
}

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
    pub status: Option<ExitStatus>,
    pub stdout_exceeded: bool,
    pub stderr_exceeded: bool,
}

pub enum TimingStatus<T> {
    /// `InTime` means the program didn't abort, but it might still TLE.
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
                Err(e) => Err(e),
            },
            Self::Aborted(d) => Ok(TimingStatus::Aborted(d)),
        }
    }
}

pub async fn timeout_with_limit<F>(limit: &Limitation, future: F) -> TimingStatus<F::Output>
where
    F: IntoFuture,
{
    if let Some(duration) = limit.max_time {
        let abort_duartion = duration.div(2).saturating_mul(3);
        match timeout(abort_duartion, future.into_future()).await {
            Ok(result) => TimingStatus::InTime(result),
            Err(_) => TimingStatus::Aborted(abort_duartion),
        }
    } else {
        let result = future.into_future().await;
        TimingStatus::InTime(result)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use crate::monitor::common::read_capped_with_policy;

    use super::{
        ChildControl, CollectedStreams, collect_piped_output, finish_child_collection,
        terminate_and_reap,
    };
    use std::{
        pin::Pin,
        process::ExitStatus,
        task::{Context, Poll},
        time::Duration,
    };
    use tokio::io::{AsyncRead, AsyncWriteExt, ReadBuf, duplex};

    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;
    #[cfg(windows)]
    use std::os::windows::process::ExitStatusExt;

    fn successful_exit_status() -> ExitStatus {
        #[cfg(unix)]
        {
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            ExitStatus::from_raw(0)
        }
        #[cfg(not(any(unix, windows)))]
        {
            unimplemented!("test exit status is only defined on supported process platforms")
        }
    }

    struct RecordingChild {
        actions: Vec<&'static str>,
        kill_error: Option<std::io::ErrorKind>,
        wait_error: Option<std::io::ErrorKind>,
    }

    struct FailingReader;

    impl AsyncRead for FailingReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut ReadBuf<'_>,
        ) -> Poll<std::io::Result<()>> {
            Poll::Ready(Err(std::io::Error::other("stderr read failed")))
        }
    }

    impl ChildControl for RecordingChild {
        fn start_kill(&mut self) -> std::io::Result<()> {
            self.actions.push("kill");
            match self.kill_error {
                Some(kind) => Err(std::io::Error::from(kind)),
                None => Ok(()),
            }
        }

        async fn wait(&mut self) -> std::io::Result<ExitStatus> {
            self.actions.push("wait");
            match self.wait_error {
                Some(kind) => Err(std::io::Error::from(kind)),
                None => Ok(successful_exit_status()),
            }
        }
    }

    async fn collect(payload: Vec<u8>, limit: usize) -> super::CappedOutput {
        let capacity = payload.len().max(1);
        let (mut writer, reader) = duplex(capacity);
        let write = async move {
            writer.write_all(&payload).await.unwrap();
            writer.shutdown().await.unwrap();
        };
        let read = read_capped_with_policy(reader, limit, false);
        let ((), output) = tokio::join!(write, read);
        output.unwrap()
    }

    #[tokio::test]
    async fn accepts_exact_limit() {
        let output = collect(vec![b'x'; 65_536], 65_536).await;
        assert_eq!(output.bytes.len(), 65_536);
        assert!(!output.exceeded);
    }

    #[tokio::test]
    async fn retains_only_limit_when_exceeded() {
        let output = collect(vec![b'x'; 65_537], 65_536).await;
        assert_eq!(output.bytes.len(), 65_536);
        assert!(output.exceeded);
    }

    #[tokio::test]
    async fn handles_no_newlines_and_split_utf8_bytes() {
        let payload = "界".repeat(30_000).into_bytes();
        let output = collect(payload.clone(), payload.len()).await;
        assert_eq!(output.bytes, payload);
        assert!(!output.exceeded);
    }

    #[tokio::test]
    async fn inherited_open_pipes_cannot_delay_latched_stdout_overflow() {
        let (stdin, _stdin_reader) = duplex(8);
        let (mut stdout_writer, stdout) = duplex(8);
        let (_stderr_writer, stderr) = duplex(8);
        stdout_writer.write_all(b"12345").await.unwrap();

        let output = tokio::time::timeout(
            Duration::from_millis(100),
            collect_piped_output(stdin, stdout, stderr, "", 4, 1024),
        )
        .await
        .expect("collection must not wait for inherited pipe EOF")
        .unwrap();

        assert!(output.stdout_exceeded);
        assert_eq!(output.stdout, b"1234");
    }

    #[tokio::test]
    async fn completed_io_latches_stdout_overflow_without_waiting() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: None,
            wait_error: None,
        };
        let streams = Ok(CollectedStreams {
            stdout: b"1234".to_vec(),
            stderr: Vec::new(),
            stdout_exceeded: true,
            stderr_exceeded: false,
        });

        let output = finish_child_collection(&mut child, streams).await.unwrap();

        assert!(output.stdout_exceeded);
        assert!(output.status.is_none());
        assert!(child.actions.is_empty());
    }

    #[tokio::test]
    async fn completed_io_latches_stdout_overflow_over_stderr_error() {
        let (stdin, _stdin_reader) = duplex(8);
        let (mut stdout_writer, stdout) = duplex(8);
        stdout_writer.write_all(b"12345").await.unwrap();

        let output = collect_piped_output(stdin, stdout, FailingReader, "", 4, 1024)
            .await
            .unwrap();

        assert!(output.stdout_exceeded);
        assert_eq!(output.stdout, b"1234");
    }

    #[tokio::test]
    async fn completed_io_waits_before_non_overflow_reader_error() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: None,
            wait_error: None,
        };
        let streams = Err(std::io::Error::other("stdout read failed"));

        let result = finish_child_collection(&mut child, streams).await;

        let Err(error) = result else {
            panic!("stdout reader error was not returned");
        };
        assert_eq!(error.to_string(), "stdout read failed");
        assert_eq!(child.actions, ["wait"]);
    }

    #[tokio::test]
    async fn terminate_and_reap_kills_then_waits() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: None,
            wait_error: None,
        };

        terminate_and_reap(&mut child).await.unwrap();

        assert_eq!(child.actions, ["kill", "wait"]);
    }

    #[tokio::test]
    async fn terminate_and_reap_waits_when_kill_races_an_exited_child() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: Some(std::io::ErrorKind::InvalidInput),
            wait_error: None,
        };

        terminate_and_reap(&mut child).await.unwrap();

        assert_eq!(child.actions, ["kill", "wait"]);
    }

    #[tokio::test]
    async fn terminate_and_reap_returns_kill_error_without_waiting() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: Some(std::io::ErrorKind::PermissionDenied),
            wait_error: None,
        };

        let error = terminate_and_reap(&mut child).await.unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(child.actions, ["kill"]);
    }

    #[tokio::test]
    async fn terminate_and_reap_propagates_wait_error_after_successful_kill() {
        let mut child = RecordingChild {
            actions: Vec::new(),
            kill_error: None,
            wait_error: Some(std::io::ErrorKind::BrokenPipe),
        };

        let error = terminate_and_reap(&mut child).await.unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(child.actions, ["kill", "wait"]);
    }
}
