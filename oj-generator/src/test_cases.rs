use fs_err::File;
use shared::bridge::write_keymap_to_file;
use std::{
    env,
    ffi::OsStr,
    fmt,
    io::{self, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use inquire::{
    CustomType, Editor, InquireError, Select, Text, error::InquireResult, validator::Validation,
};
use owo_colors::OwoColorize;

use crate::{
    advanced::{merge_with_tip, prompt_advanced_options},
    configure::{EditorChoice, GeneratorConfig},
    escapable,
    helper::{ESCAPABLE, file_path_validator},
    structs::{
        CaseInputCompleter, LabelWithOptionIndex, OPEN_EDITOR_MAGIC, OptionalInput, TestCase,
        TestSuite, YamlPathCompleter,
    },
};

pub fn generate_test_case(config: &GeneratorConfig) -> InquireResult<String> {
    let mut suite = TestSuite::new();
    let mut id: u32 = 1;

    let file = Text::new("配置檔案名稱:")
        .with_validator(with_yaml_path_validator)
        .with_formatter(&|i| with_yaml(i))
        .with_help_message("副檔名為yaml，若沒有會自動補上")
        .with_autocomplete(
            YamlPathCompleter::default().supported_code_types(config.supported_code_types.clone()),
        )
        .prompt()?;
    let file_path = with_yaml(&file);

    loop {
        let action = Select::new(
            "動作:",
            Action::LIST[0..Action::LIST.len() - usize::from(suite.cases.is_empty())].to_vec(),
        )
        .prompt()?;

        match action {
            Action::Add => {
                let input = escapable!(
                    input_text_or_editor(config, &format!("測資 {id} 輸入:")),
                    continue
                )?;
                let answer = escapable!(
                    input_text_or_editor(config, &format!("測資 {id} 答案:")),
                    continue
                )?;

                suite.cases.push(TestCase { input, answer, id });
                id += 1;
            }
            Action::Delete => {
                let mut options: Vec<LabelWithOptionIndex> = suite
                    .cases
                    .iter()
                    .enumerate()
                    .map(|(index, case)| {
                        let count = case.input.len() + case.answer.len();
                        LabelWithOptionIndex::new(
                            Some(index),
                            if case.id == 0 {
                                format!("外來測資 ({count}字)")
                            } else {
                                format!("測資 {} ({}字)", case.id, count)
                            },
                        )
                    })
                    .collect();
                options.push(LabelWithOptionIndex::new(None, "取消".to_string()));
                let selection = escapable!(
                    Select::new(
                        &format!("選擇要刪除的測資 (共 {} 筆):", suite.cases.len()),
                        options
                    )
                    .prompt(),
                    continue
                )?;
                if let Some(index) = selection.index {
                    suite.cases.remove(index);
                }
            }
            Action::LimitTime => {
                let mut limit = suite.limit.unwrap_or_default();

                let mut dialogue = CustomType::<OptionalInput<u64>>::new("最長時間 (ms):")
                    .with_help_message("留白表示使用預設值");

                let message: String;
                if let Some(max_time) = limit.time {
                    message = max_time.to_string();
                    dialogue = dialogue.with_starting_input(&message);
                }

                let max_time: Option<u64> = escapable!(dialogue.prompt(), continue)?.value;
                limit.time = max_time;
                suite.merge_limit(limit);
            }
            Action::LimitMemory => {
                let mut limit = suite.limit.unwrap_or_default();

                let mut dialogue = CustomType::<OptionalInput<u32>>::new("最大記憶體 (KiB):")
                    .with_help_message("留白表示使用預設值");

                let message: String;
                if let Some(max_memory) = limit.memory {
                    message = max_memory.to_string();
                    dialogue = dialogue.with_starting_input(&message);
                }

                let max_memory = escapable!(dialogue.prompt(), continue)?.value;
                limit.memory = max_memory;
                suite.merge_limit(limit);
            }
            Action::Submit => break,
            Action::ListMore => {
                let new_suite = escapable!(prompt_advanced_options(config, &suite), continue)?;
                merge_with_tip(new_suite, &mut suite);
            }
        }
    }

    suite.meta.retain(|_, value| !value.is_null());

    let mut file = File::create(&file_path)?;
    #[expect(clippy::unwrap_used)]
    let yaml = serde_yaml_ng::to_string(&suite).unwrap();

    file.write_all(yaml.as_bytes())?;

    println!("{}", format_args!("成功創建 '{file_path}'").green());

    Ok(file_path)
}

#[derive(Debug, Copy, Clone)]
enum Action {
    Add,
    Delete,
    Submit,
    LimitTime,
    LimitMemory,
    ListMore,
}

impl Action {
    const LIST: &'static [Self] = &[
        Self::Add,
        Self::Delete,
        Self::LimitTime,
        Self::LimitMemory,
        Self::ListMore,
        Self::Submit,
    ];
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Add => write!(f, "新增測資"),
            Self::Delete => write!(f, "刪除測資"),
            Self::LimitTime => write!(f, "限制時間"),
            Self::LimitMemory => write!(f, "限制記憶體"),
            Self::ListMore => write!(f, "進階操作"),
            Self::Submit => write!(f, "完成"),
        }
    }
}

fn with_yaml_path_validator(
    input: &str,
) -> Result<Validation, Box<dyn std::error::Error + Send + Sync>> {
    file_path_validator(with_yaml(input))
}

fn with_yaml(input: &str) -> String {
    if input.trim().is_empty() {
        String::new()
    } else if std::path::Path::new(input)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
    {
        input.to_string()
    } else {
        format!("{input}.yaml")
    }
}

fn input_text_or_editor(config: &GeneratorConfig, message: &str) -> Result<String, InquireError> {
    let input = Text::new(message)
        .with_autocomplete(CaseInputCompleter)
        .with_help_message(ESCAPABLE)
        .with_formatter(&|i| {
            if i == OPEN_EDITOR_MAGIC {
                format!("<{i}>")
            } else {
                i.to_string()
            }
        })
        .prompt()?;
    if input == OPEN_EDITOR_MAGIC {
        let mut editor = Editor::new(message);
        let mut config_path: Option<PathBuf> = None;
        let editor_path;

        match &config.editor {
            EditorChoice::Local(editor_config) => {
                editor_path = env::current_exe()?
                    .parent()
                    .ok_or_else(|| io::Error::other("executable path has no parent directory"))?
                    .join("editor");
                editor = editor.with_editor_command(editor_path.as_os_str());

                if let Some(keymap) = &editor_config.keymap {
                    let mut path: PathBuf = env::temp_dir();
                    let filename = format!(
                        "OJC-E-{}",
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos()
                    );
                    path.push(filename);

                    unsafe {
                        write_keymap_to_file(&path, keymap)
                            .map_err(|e| InquireError::Custom(e.into_boxed_dyn_error()))?;
                    };

                    config_path = Some(path);
                }
            }
            EditorChoice::Other(command) => {
                editor = editor.with_editor_command(OsStr::new(command));
            }
        }

        #[allow(clippy::manual_map)]
        // ownership problem (cannot return reference to temporary value)
        let args = if let Some(path) = &config_path {
            Some(&[OsStr::new("--input-fast"), path.as_os_str()])
        } else {
            None
        };

        if let Some(args) = args {
            editor = editor.with_args(args);
        }

        editor.prompt()
    } else {
        Ok(input)
    }
}

#[cfg(test)]
mod tests {
    use super::with_yaml;

    #[test]
    fn with_yaml_preserves_yaml_extensions_case_insensitively() {
        assert_eq!(with_yaml("problem.yaml"), "problem.yaml");
        assert_eq!(with_yaml("problem.yml"), "problem.yml");
        assert_eq!(with_yaml("problem.YAML"), "problem.YAML");
        assert_eq!(with_yaml("problem.YML"), "problem.YML");
    }
}
