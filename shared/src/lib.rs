use std::io;
use std::process::Command;
use std::{env, path::PathBuf};

pub mod bridge;

pub fn get_exe_dir() -> io::Result<PathBuf> {
    if cfg!(debug_assertions) {
        // debug
        let current_dir = env::current_dir()?;
        Ok(current_dir)
    } else {
        // release
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path
            .parent()
            .ok_or(io::Error::other("Failed to get exe directory"))?;
        Ok(exe_dir.to_path_buf())
    }
}

pub fn get_config_path() -> io::Result<PathBuf> {
    Ok(get_exe_dir()?.join("config.yaml"))
}

#[cfg(windows)]
fn build_native_shell_command(command_string: &str) -> io::Result<Command> {
    use std::os::windows::process::CommandExt;

    fn split_command_name(command_str: &str) -> Option<(&str, &str)> {
        let trimmed = command_str.trim_start();
        let original_len = command_str.len();

        if trimmed.is_empty() {
            return None;
        }

        let program_name_end_index: usize;
        let arguments_start_index: usize;

        if let Some(inner_str) = trimmed.strip_prefix('"') {
            if let Some(idx) = inner_str.find('"') {
                program_name_end_index = 1 + idx + 1;

                let after_quote_index = program_name_end_index;

                if let Some(start_arg_idx) =
                    trimmed[after_quote_index..].find(|c: char| !c.is_whitespace())
                {
                    arguments_start_index = after_quote_index + start_arg_idx;
                } else {
                    arguments_start_index = trimmed.len();
                }
            } else {
                program_name_end_index = trimmed.len();
                arguments_start_index = trimmed.len();
            }
        } else if let Some(idx) = trimmed.find(|c: char| c.is_whitespace()) {
            program_name_end_index = idx;

            if let Some(start_arg_idx) = trimmed[idx..].find(|c: char| !c.is_whitespace()) {
                arguments_start_index = idx + start_arg_idx;
            } else {
                arguments_start_index = trimmed.len();
            }
        } else {
            program_name_end_index = trimmed.len();
            arguments_start_index = trimmed.len();
        }

        let offset = original_len - trimmed.len();

        let program_name = &command_str[offset..offset + program_name_end_index];
        let arguments = &command_str[offset + arguments_start_index..];

        Some((program_name, arguments))
    }

    let (program, raw_args) = split_command_name(command_string).unwrap_or_default();

    let mut cmd = Command::new(program);
    if !raw_args.is_empty() {
        cmd.raw_arg(raw_args);
    }
    Ok(cmd)
}

#[cfg(unix)]
fn build_native_shell_command(command_string: &str) -> io::Result<Command> {
    use ::shlex;

    let args = shlex::split(command_string).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Failed to parse command string",
        )
    })?;
    if args.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Received an empty command string.",
        ));
    }
    let mut cmd = Command::new(&args[0]);
    if args.len() > 1 {
        cmd.args(&args[1..]);
    }
    Ok(cmd)
}

#[cfg(not(any(unix, windows)))]
pub fn build_native_shell_command(_command_string: &str) -> io::Result<Command> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "This platform is not supported for native shell commands.",
    ))
}

#[derive(Debug)]
pub struct RawCommand {
    raw: String,
}

impl RawCommand {
    pub fn new(s: impl Into<String>) -> Self {
        Self { raw: s.into() }
    }
    pub fn build(&self) -> io::Result<std::process::Command> {
        build_native_shell_command(&self.raw)
    }
    pub fn build_tokio(&self) -> io::Result<tokio::process::Command> {
        self.build().map(Into::into)
    }
    pub fn raw_str(&self) -> &str {
        &self.raw
    }
}
