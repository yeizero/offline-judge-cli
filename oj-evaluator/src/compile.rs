use shared::ShellCommand;
use std::collections::HashMap;
use std::io::{self};
use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::Mutex;

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

async fn run_command_with_onetime_callback<F>(
    cmd: &mut TokioCommand,
    on_first_output: F,
) -> io::Result<ExitStatus>
where
    F: FnMut() + Send + 'static,
{
    let has_called_back = Arc::new(AtomicBool::new(false));
    let callback = Arc::new(Mutex::new(on_first_output));

    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stdout_flag = Arc::clone(&has_called_back);
    let stdout_callback = Arc::clone(&callback);
    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut parent_stdout = tokio::io::stdout();
        let mut buf = vec![0; 1024];

        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if stdout_flag
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                        .is_ok()
                    {
                        let mut cb = stdout_callback.lock().await;
                        (*cb)();
                    }

                    if let Err(e) = parent_stdout.write_all(&buf[..bytes_read]).await {
                        log::debug!("Failed to write to parent stdout: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("Error reading from child stdout: {}", e);
                    break;
                }
            }
        }
    });

    let stderr_flag = Arc::clone(&has_called_back);
    let stderr_callback = Arc::clone(&callback);
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr);
        let mut parent_stderr = tokio::io::stderr();
        let mut buf = vec![0; 1024];

        loop {
            match reader.read(&mut buf).await {
                Ok(0) => break,
                Ok(bytes_read) => {
                    if stderr_flag
                        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
                        .is_ok()
                    {
                        let mut cb = stderr_callback.lock().await;
                        (*cb)();
                    }

                    if let Err(e) = parent_stderr.write_all(&buf[..bytes_read]).await {
                        log::debug!("Failed to write to parent stderr: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    log::warn!("Error reading from child stderr: {}", e);
                    break;
                }
            }
        }
    });

    let status = child.wait().await?;

    if let Err(e) = stdout_task.await {
        log::warn!("Stdout reading task failed: {:?}", e);
    }
    if let Err(e) = stderr_task.await {
        log::warn!("Stderr reading task failed: {:?}", e);
    }

    Ok(status)
}

/// 根據原始碼檔案準備一個最終可執行的指令。
///
/// 對於編譯型語言，此函式會執行編譯，並在成功後回傳一個執行已編譯產物的指令。
/// 對於直譯型語言，此函式直接回傳執行原始碼的指令。
pub async fn prepare_command<'a, F>(
    file_path: &'a str,
    lang_profile: &'a LanguageProfile,
    skip_compilation: bool,
    on_compile_output_start: F, // mut is needed here
) -> Result<ShellCommand, CompileError<'a>>
where
    F: FnMut() + Send + 'static,
{
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
                        format!("Error executing '{}' for compilation: {e}", program_name).into(),
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
