use crate::config::{resolve_java_args, JAVA_COMPILER, JAVA_RUNNER, TEMP_DIR};

use super::super::structs::CompileError;
use super::super::utils::is_compiler_available;
use std::path::Path;
use std::process::Command;

pub fn resolve_java(java_file_path: &str) -> Result<Command, CompileError> {
    let file_name = Path::new(java_file_path).file_stem().unwrap();
    let output_dir = &TEMP_DIR;

    if !is_compiler_available(JAVA_COMPILER) {
        return Err(CompileError::SE(format!(
            "Java Compiler '{}' not found",
            JAVA_COMPILER
        )));
    }

    let compilation_output = Command::new(JAVA_COMPILER)
        .args(["-d", &output_dir.to_string_lossy()])
        .arg(java_file_path)
        .output()
        .map_err(|e| CompileError::CE(e.to_string()))?;

    if !compilation_output.status.success() {
        return Err(CompileError::CE(
            String::from_utf8_lossy(&compilation_output.stderr).into(),
        ));
    }

    let mut runner = Command::new(JAVA_RUNNER);
    resolve_java_args(&mut runner);
    runner
        .args(["-cp", &output_dir.to_string_lossy()])
        .arg(file_name);
    Ok(runner)
}
