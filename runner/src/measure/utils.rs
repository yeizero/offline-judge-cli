use std::{process::Command};

use num_format::ToFormattedString;

use crate::config::NUMBER_FORMAT;

pub const TEMP_FILE_EXE: &str = "output.exe";

pub fn is_compiler_available(compiler: &str) -> bool {
    let result = Command::new(compiler).arg("--version").output();
    if log::log_enabled!(log::Level::Debug) {
        if let Ok(ref output) = result {
            log::debug!(
                "Program: {} / Stdout: {} / Stderr: {}",
                compiler,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            log::debug!(
                "Program: {} / Err: {}",
                compiler,
                result.as_ref().unwrap_err()
            );
        }
    }
    result.is_ok()
}

pub fn center_text(text: &str, total_length: usize, placeholder: &str) -> String {
    let text_length = text.len();
    if text_length >= total_length {
        return text.to_string();
    }

    let padding_length = (total_length - text_length) / 2;
    let left_padding = placeholder.repeat(padding_length);
    let right_padding = placeholder.repeat(total_length - text_length - padding_length);

    format!("{} {} {}", left_padding, text, right_padding)
}

pub fn compare_lines_ignoring_line_endings(a: &str, b: &str) -> bool {
    let lines_a = a.lines().map(str::trim_end);
    let lines_b = b.lines().map(str::trim_end);

    lines_a.eq(lines_b)
}

pub trait PrettyNumber {
    fn prettify(&self) -> String;
}

impl<T> PrettyNumber for T
where
    T: ToFormattedString,
{
    fn prettify(&self) -> String {
        self.to_formatted_string(&NUMBER_FORMAT)
    }
}
