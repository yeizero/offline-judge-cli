use crate::config::{resolve_cpp_args, CPP_COMPILER, TEMP_DIR};
use crate::measure::utils::TEMP_FILE_EXE;

use super::super::structs::CompileError;
use super::super::utils::is_compiler_available;
use std::process::Command;

pub fn resolve_cpp(cpp_file_path: &str) -> Result<Command, CompileError> {
    let output_path = TEMP_DIR.join(TEMP_FILE_EXE);

    if !is_compiler_available(CPP_COMPILER) {
        return Err(CompileError::SE(format!(
            "C++ Compiler '{}' not found",
            CPP_COMPILER
        )));
    }

    let compilation_output = resolve_cpp_args(Command::new(CPP_COMPILER).arg(cpp_file_path))
        .args(["-o", &output_path.to_string_lossy()])
        .output()
        .map_err(|e| CompileError::CE(e.to_string()))?;

    if !compilation_output.status.success() {
        return Err(CompileError::CE(
            String::from_utf8_lossy(&compilation_output.stderr).into(),
        ));
    }

    Ok(Command::new(output_path))
}
