use crate::configure::{GeneratorConfig, Plugin};
use crate::helper::truncate_with_ellipsis;
use crate::structs::{TestSuite, command_content::*};
use crate::{error, escapable, info, warn};
use inquire::ui::{Color, RenderConfig, StyleSheet};
use inquire::{Confirm, InquireError, Select, Text};
use owo_colors::OwoColorize;
use shared::{get_exe_dir, ShellCommand};
use std::fmt::Display;
use std::io::{BufRead, BufReader, Write};
use std::process::Stdio;

macro_rules! parse_json_or_continue {
    ($content:expr) => {
        match $content {
            Some(s) if !s.trim().is_empty() => match serde_json::from_str(s) {
                Ok(v) => v,
                Err(e) => {
                    warn!("外部程式", "Wrong JSON: {}", e);
                    continue;
                }
            },
            _ => {
                warn!("外部程式", "Missing JSON content");
                continue;
            }
        }
    };
}

pub fn prompt_advanced_options(
    config: &GeneratorConfig,
    old_suite: &TestSuite,
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
        .with_help_message(&truncate_with_ellipsis(&plugin.command, 60))
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

    let mut child = ShellCommand::parse_str(&plugin.command)?
        .absolute_program(&exe_path)
        .build()
        .current_dir(exe_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .env("PYTHONIOENCODING", "UTF8")
        .spawn()?;

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();

    let reader = BufReader::new(stdout);
    let mut suite = TestSuite::new();

    for line in reader.lines() {
        let line = line?;

        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("/") else {
            println!("{}", line);
            continue;
        };

        let mut parts = rest.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let content = parts.next().map(str::trim);

        match command.as_str() {
            "ask" => {
                let content: MessageContent = parse_json_or_continue!(content);
                let text = escapable!(Text::new(&content.message).prompt(), return Ok(None))?;
                stdin.write_all(text.as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            "confirm" => {
                let content: StringContent = parse_json_or_continue!(content);
                let status = escapable!(
                    Confirm::new(&content.0).with_default(true).prompt(),
                    return Ok(None)
                )?;
                stdin.write_all(&[status as u8 + b'0', b'\n'])?;
            }
            "info" => {
                let content: StringContent = parse_json_or_continue!(content);
                info!(&content.0);
            }
            "warn" => {
                let content: StringContent = parse_json_or_continue!(content);
                warn!(&content.0);
            }
            "error" => {
                let content: StringContent = parse_json_or_continue!(content);
                error!(&content.0);
            }
            "config" => {
                let config = match serde_json::to_value(&plugin.config) {
                    Ok(s) => ConfigResponse::Success { data: s },
                    Err(e) => ConfigResponse::Error {
                        error: e.to_string(),
                    },
                };

                stdin.write_all(serde_json::to_string(&config).unwrap().as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            "getdata" => {
                let mut merged_suite = old_suite.clone();
                merged_suite.merge(suite.clone());

                stdin.write_all(serde_json::to_string(&merged_suite).unwrap().as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            "data" => {
                let content: TestSuite = parse_json_or_continue!(content);
                suite.merge(content);
            }
            _ => {
                warn!(
                    "外部程式",
                    "忽略未知操作 '{}'",
                    truncate_with_ellipsis(&command, 12)
                );
            }
        }
    }

    match child.wait() {
        Ok(status) => {
            if status.success() {
                Ok(Some(suite))
            } else {
                match status.code() {
                    Some(code) => {
                        error!(format_args!("Subprocess Exited with status code: {code}"))
                    }
                    None => error!("Subprocess terminated by signal"),
                }
                Ok(None)
            }
        }
        Err(e) => {
            error!(e);
            Ok(None)
        }
    }
}

pub fn merge_with_tip(new_suite: Option<TestSuite>, old_suite: &mut TestSuite) {
    if let Some(suite) = new_suite {
        let mut no_change = true;

        if !suite.cases.is_empty() {
            info!("新增 {} 筆測資", suite.cases.len());
            no_change = false;
        }

        if let Some(limit) = suite.limit {
            if let Some(time) = limit.time {
                no_change = false;
                info!("時間限制更新為 {}", time);
            }
            if let Some(memory) = limit.memory {
                no_change = false;
                info!("記憶體限制更新為 {}", memory);
            }
        }

        if !suite.meta.is_empty() {
            info!("修改 {} 條附加資訊", suite.meta.len());
        }

        if no_change {
            info!("未進行任何更新");
        }

        old_suite.merge(suite);
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
