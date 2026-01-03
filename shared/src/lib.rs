use std::io;
use std::path::Path;
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

#[derive(Debug, Clone)]
pub struct ShellCommand {
    program: String,
    args: Vec<String>,
}

impl ShellCommand {
    /// 從完整的命令列字串解析並創建一個 `ParsedCommand`。
    ///
    /// 這個方法會在創建時就進行平台特定的解析。
    /// 如果命令字串為空或無效，將會回傳錯誤。
    ///
    /// # Errors
    ///
    /// 如果輸入字串無法解析為有效的命令，將回傳 `io::Error`。
    pub fn parse_str(command_string: &str) -> io::Result<Self> {
        let mut parts = Self::split_string(command_string)?;

        if parts.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Received an empty command string.",
            ));
        }

        let program = parts.remove(0);
        let args = parts;

        Ok(Self { program, args })
    }

    pub fn build(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        cmd
    }

    pub fn build_tokio(&self) -> tokio::process::Command {
        self.build().into()
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn absolute_program(&mut self, base_dir: impl AsRef<Path>) -> &mut Self {
        let program_path = Path::new(&self.program);

        if program_path.is_relative() && (self.program.contains('/') || self.program.contains('\\'))
        {
            let new_program_path = base_dir.as_ref().join(program_path);

            if let Ok(canon_path) = dunce::canonicalize(new_program_path)
                && let Some(s) = canon_path.to_str()
            {
                self.program = s.to_string();
            }
        }
        self
    }

    // -- Private --

    #[cfg(unix)]
    fn split_string(command_string: &str) -> io::Result<Vec<String>> {
        shlex::split(command_string).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Failed to parse command string with shlex",
            )
        })
    }

    #[cfg(windows)]
    fn split_string(command_string: &str) -> io::Result<Vec<String>> {
        use std::ffi::OsStr;
        use std::io;
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Foundation::{HLOCAL, LocalFree};
        use windows_sys::Win32::UI::Shell::CommandLineToArgvW;

        if command_string.trim().is_empty() {
            return Ok(Vec::new());
        }

        let wide_chars: Vec<u16> = OsStr::new(command_string)
            .encode_wide()
            .chain(Some(0))
            .collect();

        let mut argc = 0;
        let argv_ptr = unsafe { CommandLineToArgvW(wide_chars.as_ptr(), &mut argc) };
        if argv_ptr.is_null() {
            return Err(io::Error::last_os_error());
        }

        struct ArgvGuard(HLOCAL);
        impl Drop for ArgvGuard {
            fn drop(&mut self) {
                unsafe { LocalFree(self.0) };
            }
        }
        let _guard = ArgvGuard(argv_ptr as HLOCAL);

        let argv_slice = unsafe { std::slice::from_raw_parts(argv_ptr, argc as usize) };

        argv_slice
            .iter()
            .map(|&arg_ptr| unsafe {
                let len = (0..).take_while(|&i| *arg_ptr.add(i) != 0).count();
                String::from_utf16(std::slice::from_raw_parts(arg_ptr, len))
            })
            .collect::<Result<Vec<String>, _>>()
            .map_err(|os_string| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid UTF-16 in command line argument: {}", os_string),
                )
            })
    }

    #[cfg(not(any(unix, windows)))]
    fn split_string(_command_string: &str) -> io::Result<Vec<String>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "This platform is not supported.",
        ))
    }
}
