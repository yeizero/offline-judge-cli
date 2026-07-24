use shared::ShellCommand;
use std::collections::HashMap;
use std::io::{self};
use std::path::Path;
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::oneshot;
use tokio::task::JoinSet;

use crate::config::TEMP_DIR;
use crate::judge::verdict::CompileError;
use crate::reader::LanguageProfile;
use crate::utils::TEMP_FILE_EXE;

type Placeholders<'a> = HashMap<&'a str, &'a str>;

fn build_command_from_template(
    template: &str,
    placeholders: &Placeholders,
) -> io::Result<ShellCommand> {
    let mut final_command_str = template.to_string();
    for (key, value) in placeholders {
        final_command_str = final_command_str.replace(&format!("{{{key}}}"), value);
    }
    ShellCommand::parse_str(&final_command_str)
}

async fn pipe_stream<R, W>(
    mut stream: R,
    mut parent_stream: W,
    mut tx: Option<oneshot::Sender<()>>,
) where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(&mut stream);
    let mut buf = vec![0; 1024];
    loop {
        match reader.read(&mut buf).await {
            Ok(0) => break,
            Ok(bytes_read) => {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
                if let Err(e) = parent_stream.write_all(&buf[..bytes_read]).await {
                    log::debug!("Failed to write to parent stream: {e}");
                    break;
                }
            }
            Err(e) => {
                log::warn!("Error reading from child stream: {e}");
                break;
            }
        }
    }
}

async fn run_command_with_onetime_callback(
    cmd: &mut TokioCommand,
    on_first_output: impl FnOnce(),
) -> io::Result<std::process::ExitStatus> {
    let (tx_out, rx_out) = oneshot::channel::<()>();
    let (tx_err, rx_err) = oneshot::channel::<()>();

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    #[expect(clippy::unwrap_used)]
    let stdout = child.stdout.take().unwrap();
    #[expect(clippy::unwrap_used)]
    let stderr = child.stderr.take().unwrap();

    let mut io_tasks = JoinSet::new();
    io_tasks.spawn(pipe_stream(stdout, tokio::io::stdout(), Some(tx_out)));
    io_tasks.spawn(pipe_stream(stderr, tokio::io::stderr(), Some(tx_err)));

    tokio::select! {
        biased;
        Ok(()) = rx_out => { on_first_output(); },
        Ok(()) = rx_err => { on_first_output(); },
        _ = child.wait() => {},
    }

    let status = child.wait().await?;
    while let Some(res) = io_tasks.join_next().await {
        if let Err(e) = res {
            log::warn!("I/O task panicked: {e:?}");
        }
    }
    Ok(status)
}

/// 根據原始碼檔案準備一個最終可執行的指令。
///
/// 對於編譯型語言，此函式會執行編譯，並在成功後回傳一個執行已編譯產物的指令。
/// 對於直譯型語言，此函式直接回傳執行原始碼的指令。
pub async fn prepare_command<'a>(
    file_path: &'a str,
    lang_profile: &'a LanguageProfile,
    skip_compilation: bool,
    on_compile_output_start: impl FnOnce(),
) -> Result<ShellCommand, CompileError<'a>> {
    let source_path = Path::new(file_path);
    let source_path_normalized = file_path.replace('\\', "/");

    let source_filename_stem = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| CompileError::SE(format!("Invalid source file path: {file_path}").into()))?;

    let mut output_path = TEMP_DIR.clone();
    let output_folder_normalized = output_path
        .to_str()
        .ok_or_else(|| CompileError::SE("Failed to construct a valid UTF-8 output path.".into()))?
        .replace('\\', "/");

    output_path.push(TEMP_FILE_EXE);
    let output_path_str = output_path
        .to_str()
        .ok_or_else(|| CompileError::SE("Failed to construct a valid UTF-8 output path.".into()))?;
    let output_path_normalized = output_path_str.replace('\\', "/");

    let mut placeholders = Placeholders::new();
    placeholders.insert("source", &source_path_normalized);
    placeholders.insert("output", &output_path_normalized);
    placeholders.insert("output_folder", &output_folder_normalized);
    placeholders.insert("source_stem", source_filename_stem);

    if !skip_compilation && let Some(compile_instruction) = &lang_profile.compile {
        let mut compile_cmd =
            build_command_from_template(&compile_instruction.command, &placeholders)
                .map_err(|e| {
                    CompileError::SE(format!("Failed to parse compile command: {e}").into())
                })?
                .build_tokio();
        compile_cmd.kill_on_drop(true);

        let compile_status =
            run_command_with_onetime_callback(&mut compile_cmd, on_compile_output_start)
                .await
                .map_err(|e| {
                    let program_name = compile_cmd
                        .as_std()
                        .get_program()
                        .to_string_lossy()
                        .into_owned();
                    CompileError::SE(
                        format!("Error executing '{program_name}' for compilation: {e}").into(),
                    )
                })?;

        if !compile_status.success() {
            return Err(CompileError::CE("Failed to compile source code.".into()));
        }
    }

    if let Some(run_instruction) = &lang_profile.run {
        build_command_from_template(&run_instruction.command, &placeholders)
            .map_err(|e| CompileError::SE(format!("Failed to parse run command: {e}").into()))
    } else if lang_profile.compile.is_some() {
        ShellCommand::parse_str(&output_path_normalized).map_err(|e| {
            CompileError::SE(format!("Failed to parse output path as command: {e}").into())
        })
    } else {
        Err(CompileError::SE(
            format!(
                "No 'compile' or 'run' instruction found for '{}' in config.",
                lang_profile.extension
            )
            .into(),
        ))
    }
}
