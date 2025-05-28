use crate::config::PYTHON_RUNNER;

use super::super::structs::CompileError;
use super::super::utils::is_compiler_available;
use std::process::Command;

pub fn resolve_python(py_file_path: &str) -> Result<Command, CompileError> {
    if !is_compiler_available(PYTHON_RUNNER) {
        return Err(CompileError::SE(format!(
            "Python Runner '{}' not found",
            PYTHON_RUNNER
        )));
    }

    let mut runner = Command::new(PYTHON_RUNNER);
    runner.arg(py_file_path);
    Ok(runner)
}
