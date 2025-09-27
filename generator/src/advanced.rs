use crate::configure::{GeneratorConfig, Plugin};
use crate::structs::{TestCase, TestLimit, TestSuite, parse_easy_test_suite};
use crate::utils::with_ellipsis;
use crate::{error, escapable, info, warn};
use inquire::ui::{Color, RenderConfig, StyleSheet};
use inquire::{Confirm, InquireError, Select, Text};
use owo_colors::OwoColorize;
use shared::{build_native_shell_command, get_exe_dir};
use std::fmt::Display;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::Stdio;

pub fn prompt_advanced_options(
    config: &GeneratorConfig,
) -> Result<Option<TestSuite>, InquireError> {
    let mut options = Vec::with_capacity(1 + config.plugins.as_ref().map_or(0, |p| p.len()));
    options.push(Action::Cancel);

    if let Some(plugins) = config.plugins.as_ref() {
        options.extend(plugins.iter().map(Action::External));
    }

    let action = Select::new("選擇進階選項:", options).prompt()?;

    let Action::External(plugin) = action else {
        return Ok(None);
    };
    let status = Confirm::new("你即將執行外部指令，是否信任?")
        .with_help_message(&with_ellipsis(&plugin.command, 60))
        .with_default(true)
        .with_render_config(
            RenderConfig::default()
                .with_help_message(StyleSheet::default().with_fg(Color::DarkGrey)),
        )
        .prompt()?;
    if !status {
        return Ok(None);
    }

    // SAFE `unwrap`: `plugins` are retrieved from config, which is loaded via exe_dir.
    let exe_path = get_exe_dir().unwrap();

    let mut child = build_native_shell_command(&plugin.command)?
        .current_dir(exe_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "UTF8")
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stderr_handle = std::thread::spawn(move || -> String {
        let mut reader = std::io::BufReader::new(stderr);
        let mut total_buffer: Vec<u8> = Vec::new();
        let mut chunk_buffer = [0; 1024];
        loop {
            match reader.read(&mut chunk_buffer) {
                Ok(0) => break,
                Ok(n) => {
                    total_buffer.extend_from_slice(&chunk_buffer[..n]);
                }
                Err(e) => {
                    error!("讀取子程序 stderr 時發生錯誤", e);
                    break;
                }
            }
        }

        String::from_utf8_lossy(&total_buffer).to_string()
    });

    let reader = BufReader::new(stdout);
    let mut result_output = String::new();
    let mut after_result = false;

    for line in reader.lines() {
        let line = line?;

        if after_result {
            result_output.push_str(&line);
            result_output.push('\n');
            continue;
        }

        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("/") else {
            println!("{}", line);
            continue;
        };

        let mut parts = rest.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default().trim();
        let content = parts.next().map(str::trim);

        match command {
            "ask" => {
                let mut text = escapable!(
                    Text::new(content.unwrap_or_default()).prompt(),
                    return Ok(None)
                )?;
                text.push('\n');
                stdin.write_all(text.as_bytes())?;
            }
            "confirm" => {
                let status = escapable!(
                    Confirm::new(content.unwrap_or_default())
                        .with_default(true)
                        .prompt(),
                    return Ok(None)
                )?;
                stdin.write_all(&[status as u8 + b'0', b'\n'])?;
            }
            "info" => {
                info!(content.unwrap_or_default());
            }
            "warn" => {
                warn!(content.unwrap_or_default());
            }
            "error" => {
                error!(content.unwrap_or_default());
            }
            "config" => {
                let mut args = content.unwrap_or_default().splitn(2, char::is_whitespace);
                let method = args.next().unwrap_or_default().trim();
                let key = args.next().unwrap_or_default();

                if method == "read" {
                    let value: &str = plugin
                        .config
                        .get(key)
                        .map(|s| s.as_str())
                        .unwrap_or_default();
                    stdin.write_all(value.as_bytes())?;
                } else {
                    warn!("外部程式", "忽略未知操作 'config {}'", method);
                }
                stdin.write_all(b"\n")?
            }
            "result" => {
                after_result = true;
            }
            _ => {
                warn!("外部程式", "忽略未知操作 '{}'", command);
            }
        }
    }

    match child.wait() {
        Ok(status) => {
            if status.success() {
                let suite = parse_easy_test_suite(&result_output);
                Ok(Some(suite))
            } else {
                let stder_output = stderr_handle.join().unwrap();
                error!(stder_output);
                Ok(None)
            }
        }
        Err(e) => {
            error!(e);
            Ok(None)
        }
    }
}

pub fn update_by_advanced(
    suite: Option<TestSuite>,
    test_cases: &mut Vec<TestCase>,
    test_limit: &mut TestLimit,
) {
    if let Some(suite) = suite {
        let mut no_change = suite.cases.is_empty();

        if !no_change {
            info!("新增 {} 筆測資", suite.cases.len());
        }
        test_cases.extend(suite.cases);

        if let Some(limit) = suite.limit {
            if let Some(time) = limit.time {
                test_limit.time = limit.time;
                no_change = false;
                info!("時間限制更新為 {}", time);
            }
            if let Some(memory) = limit.memory {
                test_limit.memory = limit.memory;
                no_change = false;
                info!("記憶體限制更新為 {}", memory);
            }
        }

        if no_change {
            info!("未進行任何更新");
        }
    };
}

enum Action<'a> {
    Cancel,
    External(&'a Plugin),
}

impl<'a> Display for Action<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cancel => write!(f, "返回"),
            Self::External(ext) => write!(f, "{}", ext.name),
        }
    }
}
