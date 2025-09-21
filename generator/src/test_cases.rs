use fs_err::File;
use std::{fmt, io::Write};

use inquire::{
    CustomType, Editor, InquireError, Select, Text, error::InquireResult, validator::Validation,
};
use owo_colors::OwoColorize;

use crate::{
    advanced::{prompt_advanced_options, update_by_advanced},
    configure::GeneratorConfig,
    escapable,
    structs::{
        CaseInputCompleter, LabelWithOptionIndex, OPEN_EDITOR_MAGIC, OptionalInput, TestCase,
        TestLimit, TestSuite, YamlPathCompleter,
    },
    utils::{ESCAPABLE, file_path_validator},
};

pub fn generate_test_case(config: &GeneratorConfig) -> InquireResult<String> {
    let mut test_cases: Vec<TestCase> = Vec::new();
    let mut test_limit = TestLimit::new();
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
            Action::LIST[0..Action::LIST.len() - (test_cases.is_empty()) as usize].to_vec(),
        )
        .prompt()?;

        match action {
            Action::Add => {
                let input = escapable!(
                    input_text_or_editor(&format!("測資 {} 輸入:", id)),
                    continue
                )?;
                let answer = escapable!(
                    input_text_or_editor(&format!("測資 {} 答案:", id)),
                    continue
                )?;

                let test_case = TestCase { input, answer, id };
                test_cases.push(test_case);
                id += 1;
            }
            Action::Delete => {
                let mut options: Vec<LabelWithOptionIndex> = test_cases
                    .iter()
                    .enumerate()
                    .map(|(index, case)| {
                        LabelWithOptionIndex::new(
                            Some(index),
                            format!(
                                "{} ({}字)",
                                if case.id == 0 {
                                    "外來測資".to_string()
                                } else {
                                    format!("測資 {}", case.id)
                                },
                                case.input.len() + case.answer.len()
                            ),
                        )
                    })
                    .collect();
                options.push(LabelWithOptionIndex::new(None, "取消".to_string()));
                let selection = escapable!(
                    Select::new(
                        &format!("選擇要刪除的測資 (共 {} 筆):", test_cases.len()),
                        options
                    )
                    .prompt(),
                    continue
                )?;
                if let Some(index) = selection.index {
                    test_cases.remove(index);
                };
            }
            Action::LimitTime => {
                let init_text: String;
                let mut dialogue = CustomType::<OptionalInput<u64>>::new("最長時間 (ms):")
                    .with_help_message("留白表示使用預設值");
                if let Some(max_time) = test_limit.time {
                    init_text = max_time.to_string();
                    dialogue = dialogue.with_starting_input(&init_text);
                }

                let max_time: Option<u64> = escapable!(dialogue.prompt(), continue)?.value;
                test_limit.time = max_time;
            }
            Action::LimitMemory => {
                let init_text: String;
                let mut dialogue = CustomType::<OptionalInput<u32>>::new("最大記憶體 (KiB):")
                    .with_help_message("留白表示使用預設值");
                if let Some(max_memory) = test_limit.memory {
                    init_text = max_memory.to_string();
                    dialogue = dialogue.with_starting_input(&init_text);
                }

                let max_memory = escapable!(dialogue.prompt(), continue)?.value;
                test_limit.memory = max_memory;
            }
            Action::Submit => break,
            Action::ListMore => {
                let suite = escapable!(prompt_advanced_options(config), continue)?;
                update_by_advanced(suite, &mut test_cases, &mut test_limit);
            }
        }
    }

    let mut file = File::create(&file_path)?;
    let yaml = serde_yml::to_string(&TestSuite {
        limit: test_limit.into_option(),
        cases: test_cases,
    })
    .unwrap();

    file.write_all(yaml.as_bytes())?;

    println!("{}", format!("成功創建 '{}'", &file_path).green());

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
    const LIST: &'static [Action] = &[
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
        "".to_string()
    } else if input.ends_with(".yaml") || input.ends_with(".yml") {
        input.to_string()
    } else {
        format!("{}.yaml", input)
    }
}

fn input_text_or_editor(message: &str) -> Result<String, InquireError> {
    let input = Text::new(message)
        .with_autocomplete(CaseInputCompleter)
        .with_help_message(ESCAPABLE)
        .with_formatter(&|i| {
            if i == OPEN_EDITOR_MAGIC {
                format!("<{}>", i)
            } else {
                i.to_string()
            }
        })
        .prompt()?;
    if input == OPEN_EDITOR_MAGIC {
        Editor::new(message).prompt()
    } else {
        Ok(input)
    }
}
