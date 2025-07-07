use std::{fmt, fs::File, io::Write};

use inquire::{error::InquireResult, validator::Validation, CustomType, Editor, InquireError, Select, Text};
use owo_colors::OwoColorize;

use crate::{
    advance::{prompt_advance_options, update_by_advance}, escapable, configure::GeneratorConfig, structs::{CaseInputCompleter, ConfigFile, LabelWithOptionIndex, OptionalInput, TestCase, TestLimit, YamlPathCompleter, OPEN_EDITOR_MAGIC}, utils::{file_path_validator, ESCAPABLE}
};

pub fn generate_test_case(setting: &GeneratorConfig) -> InquireResult<String> {
    let mut test_cases: Vec<TestCase> = Vec::new();
    let mut test_limit = TestLimit::new();
    let mut id: u32 = 1;

    let file = Text::new("配置檔案名稱:")
        .with_validator(with_yaml_path_validator)
        .with_formatter(&|i| with_yaml(i))
        .with_help_message("副檔名為yaml，若沒有會自動補上")
        .with_autocomplete(YamlPathCompleter::default())
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

                let test_case = TestCase {
                    input, answer, id,
                };
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
                                if case.id == 0 { "外來測資".to_owned() } else { format!("測資 {}", case.id) },
                                case.input.len() + case.answer.len()
                            ),
                        )
                    })
                    .collect();
                options.push(LabelWithOptionIndex::new(None, "取消".to_owned()));
                let selection = escapable!(Select::new(
                    &format!("選擇要刪除的測資 (共 {} 筆):", test_cases.len()), 
                    options
                ).prompt(), continue)?;
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

                let max_time: Option<u64> = escapable!(dialogue.prompt(), continue) ?.value;
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
                let config = escapable!(
                    prompt_advance_options(setting),
                    continue
                )?;
                update_by_advance(config, &mut test_cases, &mut test_limit);
            }
        }
    }

    let mut file = File::create(&file_path).expect("創建檔案失敗");
    let yaml = serde_yml::to_string(&ConfigFile {
        limit: test_limit.to_option(),
        cases: test_cases,
    }).unwrap();

    file.write_all(yaml.as_bytes()).expect("寫入檔案失敗");

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
        "".to_owned()
    } else if input.ends_with(".yaml") || input.ends_with(".yml") {
        input.to_owned()
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
                i.to_owned()
            }
        })
        .prompt()?;
    if input == OPEN_EDITOR_MAGIC {
        Editor::new(message)
            .prompt()        
    } else {
        Ok(input)
    }
}