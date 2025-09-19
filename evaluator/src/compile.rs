use shared::build_native_shell_command;
use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::config::TEMP_DIR;
use crate::judge::verdict::CompileError;
use crate::reader::LanguageProfile;
use crate::utils::TEMP_FILE_EXE;

type Placeholders<'a> = HashMap<&'a str, &'a str>;

fn build_command_from_template(
    template: &str,
    placeholders: &Placeholders,
) -> Result<Command, io::Error> {
    let mut final_command_str = template.to_string();
    for (key, value) in placeholders {
        final_command_str = final_command_str.replace(&format!("{{{key}}}"), value);
    }

    build_native_shell_command(&final_command_str)
}

/// 根據原始碼檔案準備一個最終可執行的指令。
///
/// 對於編譯型語言，此函式會執行編譯，並在成功後回傳一個執行已編譯產物的指令。
/// 對於直譯型語言，此函式直接回傳執行原始碼的指令。
///
/// # Arguments
/// * `file_path` - 原始碼檔案的路徑。
/// * `file_type` - 程式語言的枚舉。
/// * `config` - 已載入的評測器設定。
///
/// # Returns
/// * `Ok(Command)` - 一個準備好執行的 `Command`。
/// * `Err(CompileError)` - 如果發生系統錯誤或編譯失敗。
pub fn prepare_command<'a>(
    file_path: &'a str,
    lang_profile: &'a LanguageProfile,
) -> Result<Command, CompileError<'a>> {
    let source_path_normalized = file_path.replace('\\', "/");

    if let Some(compile_instruction) = &lang_profile.compile {
        let source_path = Path::new(file_path);
        let source_filename_stem = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                CompileError::SE(format!("Invalid source file path: {file_path}").into())
            })?;

        let mut output_path = TEMP_DIR.clone();

        let output_folder_normalized = output_path
            .to_str()
            .ok_or_else(|| {
                CompileError::SE("Failed to construct a valid UTF-8 output path.".into())
            })?
            .replace('\\', "/");

        output_path.push(TEMP_FILE_EXE);

        let output_path_str = output_path.to_str().ok_or_else(|| {
            CompileError::SE("Failed to construct a valid UTF-8 output path.".into())
        })?;

        let output_path_normalized = output_path_str.replace('\\', "/");

        let mut placeholders = Placeholders::new();
        placeholders.insert("source", &source_path_normalized);
        placeholders.insert("output", &output_path_normalized);
        placeholders.insert("output_folder", &output_folder_normalized);
        placeholders.insert("source_stem", source_filename_stem);

        let mut compile_cmd =
            build_command_from_template(&compile_instruction.command, &placeholders)
                .map_err(|e| CompileError::SE(e.to_string().into()))?;

        let compile_status = compile_cmd.status().map_err(|e| {
            CompileError::SE(format!("Failed to execute compile command: {e}").into())
        })?;

        if !compile_status.success() {
            return Err(CompileError::CE("Failed to compile source code.".into()));
        }

        if let Some(run_instruction) = &lang_profile.run {
            build_command_from_template(&run_instruction.command, &placeholders)
                .map_err(|e| CompileError::SE(e.to_string().into()))
        } else {
            Ok(Command::new(&output_path_normalized))
        }
    } else if let Some(run_instruction) = &lang_profile.run {
        let mut placeholders = Placeholders::new();
        placeholders.insert("source", &source_path_normalized);

        build_command_from_template(&run_instruction.command, &placeholders)
            .map_err(|e| CompileError::SE(e.to_string().into()))
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
