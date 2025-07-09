use std::io;
use std::{env, path::PathBuf};
use std::process::Command;
#[cfg(unix)]
use::shlex;

pub fn get_exe_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        // debug
        let current_dir = env::current_dir()?;
        Ok(current_dir)
    } else {
        // release
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path.parent().ok_or("Failed to get exe directory")?;
        Ok(exe_dir.to_path_buf())
    }
}

pub fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(get_exe_dir()?.join("config.yaml"))
}

#[cfg(windows)]
pub fn build_native_shell_command(command_string: &str) -> Result<Command, io::Error> {
    let mut cmd = Command::new("powershell");
    cmd.arg("-Command").arg(command_string);
    Ok(cmd)
}

#[cfg(unix)]
pub fn build_native_shell_command(command_string: &str) -> Result<Command, io::Error> {
    let args = shlex::split(command_string).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "Failed to parse command string")
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
pub fn build_native_shell_command(_command_string: &str) -> Result<Command, io::Error> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "This platform is not supported for native shell commands."
    ))
}