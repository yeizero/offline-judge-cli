use std::io;
use std::{env, path::PathBuf};
use std::process::Command;

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

pub fn build_native_shell_command(command_string: &str) -> Result<Command, io::Error> {
    if cfg!(unix) {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command_string);
        Ok(cmd)
    } else if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.arg("-Command").arg(command_string);
        Ok(cmd)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "This platform is not supported for native shell commands."
        ))
    }
}