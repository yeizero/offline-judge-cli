use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::{fmt::Display, process::Command};

use inquire::ui::{Color, RenderConfig, StyleSheet};
use inquire::{Confirm, InquireError, Select, Text};
use owo_colors::OwoColorize;
use crate::setting_reader::{Setting, Plugin};
use crate::structs::{parse_easy_config, ConfigFile, TestCase, TestLimit};
use crate::utils::{get_exe_dir, with_ellipsis};
use crate::{error, escapable, info, warn};

pub fn prompt_advance_options(setting: &Setting) -> Result<Option<ConfigFile>, InquireError> {
    let mut options = Vec::new();

    setting.plugins.as_ref().map(|plugins| {
        plugins.iter().for_each(|ext| options.push(Action::External(ext)));
    });
    options.insert(0, Action::Cancel);

    let action = Select::new("選擇進階選項:", options).prompt()?;

    let Action::External(ext) = action else {
        return Ok(None);
    };
    let status = Confirm::new("你即將執行外部指令，是否信任?")
        .with_help_message(&with_ellipsis(&ext.command.join(" "), 60))
        .with_default(true)
        .with_render_config(
            RenderConfig::default()
            .with_help_message(
                StyleSheet::default()
                .with_fg(Color::DarkGrey)
            )
        )
        .prompt()?;
    if !status {
        return Ok(None); 
    }

    let program = &ext.command[0];
    // SAFE `unwrap`: `plugins` are retrieved from config, which is loaded via exe_dir.
    let exe_path = get_exe_dir().unwrap();
    let mut command = if (program.contains('/') || program.contains('\\')) && PathBuf::from(program).is_relative() {
        let mut abs_path = exe_path.clone();
        abs_path.push(program);
        Command::new(abs_path)
    } else {
        Command::new(program)
    };

    let mut child = command
        .args(&ext.command[1..ext.command.len()])
        .current_dir(exe_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("PYTHONIOENCODING", "UTF8")
        .spawn()
        .map_err(InquireError::IO)?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let stderr_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut collected = String::new();
        for line in reader.lines() {
            let line = line.unwrap_or_default();
            collected.push_str(&line);
            collected.push('\n');
        }
        collected
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
                    Text::new(content.unwrap_or_default())
                        .prompt(),
                    return Ok(None)
                )?;
                text.push('\n');
                stdin.write_all(text.as_bytes())?                                
            }
            "confirm" => {
                let status = escapable!(
                    Confirm::new(content.unwrap_or_default())
                        .with_default(true)
                        .prompt(),
                    return Ok(None)
                )?;
                stdin.write_all(&[status as u8 + b'0', b'\n'])?                                
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
                let config = parse_easy_config(&result_output);
                Ok(Some(config))
            } else {
                let stderr_output = stderr_handle.join().unwrap();
                error!(stderr_output);
                Ok(None)
            }
        },
        Err(e) => {
            error!(e);
            Ok(None)
        }
    }
}

pub fn update_by_advance(config: Option<ConfigFile>, test_cases: &mut Vec<TestCase>, test_limit: &mut TestLimit) {
    if let Some(config) = config {
        let mut no_change = config.cases.is_empty();

        if !no_change {
            info!("新增 {} 筆測資", config.cases.len());
        }
        test_cases.extend(config.cases);

        if let Some(limit) = config.limit {
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
            Self::External(ext) => write!(f, "{}", ext.option),
        }
    }
}