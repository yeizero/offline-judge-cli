use owo_colors::OwoColorize;
use serde::Serialize;
use std::collections::VecDeque;

#[derive(Serialize)]
pub struct TestSuite {
    pub limit: Option<TestLimit>,
    pub cases: Vec<TestCase>,
}

#[derive(Serialize)]
pub struct TestCase {
    pub input: String,
    pub answer: String,

    #[serde(skip_serializing)]
    pub id: u32,
}

#[derive(Serialize)]
pub struct TestLimit {
    pub memory: Option<u32>,
    pub time: Option<u64>,
}

impl TestLimit {
    pub fn new() -> Self {
        Self {
            memory: None,
            time: None,
        }
    }
    pub fn into_option(self) -> Option<Self> {
        if self.memory.is_some() || self.time.is_some() {
            Some(self)
        } else {
            None
        }
    }
}

pub fn parse_easy_test_suite(input: &str) -> TestSuite {
    let mut lines = input.lines();
    let mut limit = TestLimit {
        memory: None,
        time: None,
    };
    let mut inputs = VecDeque::new();
    let mut answers = VecDeque::new();
    let mut cases = Vec::new();

    while let Some(header) = lines.next() {
        let parts: Vec<&str> = header.split_whitespace().collect();

        if parts.len() != 2 {
            eprintln!(
                "{}",
                format_args!("[Parse] 錯誤格式（應為 `key 行數`）: `{}`", header).red()
            );
            continue;
        }

        let key = parts[0];
        let count = match parts[1].parse::<usize>() {
            Ok(c) => c,
            Err(_) => {
                eprintln!(
                    "{}",
                    format_args!("[Parse] 行數無法解析於: `{}`", header).red()
                );
                continue;
            }
        };

        let mut content = Vec::new();
        for i in 0..count {
            match lines.next() {
                Some(line) => content.push(line.to_string()),
                None => {
                    eprintln!(
                        "{}",
                        format_args!(
                            "[Parse] 預期 {} 行，但只取得 {} 行，在 key `{}`",
                            count, i, key
                        )
                        .red()
                    );
                    break;
                }
            }
        }

        if content.len() != count {
            continue;
        }

        let joined = content.join("\n");

        match key {
            "limit" => {
                let tokens: Vec<&str> = joined.split_whitespace().collect();

                if tokens.len() % 2 != 0 {
                    eprintln!("{}", "[Parse] limit 欄位格式錯誤：參數需成對出現".red());
                    continue;
                }

                let iter = tokens.chunks(2);

                for chunk in iter {
                    match chunk {
                        ["time", val] => match val.parse::<u64>() {
                            Ok(ms) => limit.time = Some(ms),
                            Err(_) => {
                                eprintln!("{}", format_args!("[Parse] 時間格式錯誤: `{val}`").red())
                            }
                        },
                        ["memory", val] => match val.parse::<u32>() {
                            Ok(mem) => limit.memory = Some(mem),
                            Err(_) => {
                                eprintln!(
                                    "{}",
                                    format_args!("[Parse] 記憶體格式錯誤: `{val}`").red()
                                )
                            }
                        },
                        _ => {
                            eprintln!(
                                "{}",
                                format_args!("[Parse] limit 欄位未知格式: {chunk:?}").red()
                            );
                        }
                    }
                }
            }
            "input" => inputs.push_back(joined),
            "answer" => answers.push_back(joined),
            other => {
                eprintln!("{}", format_args!("[Parse] 忽略未知 key `{other}`").red());
            }
        }
    }

    while let (Some(input), Some(answer)) = (inputs.pop_front(), answers.pop_front()) {
        cases.push(TestCase {
            input,
            answer,
            id: 0,
        });
    }

    let limit = if limit.memory.is_some() || limit.time.is_some() {
        Some(limit)
    } else {
        None
    };

    TestSuite { limit, cases }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(input: &str) -> TestSuite {
        parse_easy_test_suite(input)
    }

    #[test]
    fn test_parse_valid_config() {
        let input = r#"
limit 2
time 1000
memory 512
input 1
1 2 3
answer 1
6
"#;
        let config = case(input.trim());

        assert_eq!(config.limit.as_ref().unwrap().time, Some(1000));
        assert_eq!(config.limit.as_ref().unwrap().memory, Some(512));
        assert_eq!(config.cases.len(), 1);
        assert_eq!(config.cases[0].input, "1 2 3");
        assert_eq!(config.cases[0].answer, "6");
    }

    #[test]
    fn test_parse_missing_lines() {
        let input = r#"
input 2
1 2
"#;
        let config = case(input.trim());
        // 缺了一行，無法生成 case
        assert!(config.cases.is_empty());
    }

    #[test]
    fn test_parse_invalid_limit() {
        let input = r#"
limit 2
time abc
memory 256
input 1
1 2
answer 1
3
"#;
        let config = case(input.trim());
        // memory 解析成功，但 time 是錯誤格式
        assert_eq!(config.limit.as_ref().unwrap().memory, Some(256));
        assert_eq!(config.limit.as_ref().unwrap().time, None);
    }

    #[test]
    fn test_parse_invalid_limit_odd_tokens() {
        let input = r#"
    limit 2
    time 6
    memory
    "#;
        let config = case(input.trim());

        assert!(config.limit.is_none());
    }
    #[test]
    fn test_parse_extra_header_parts() {
        let input = r#"
input 1 extra
1 2
answer 1
3
"#;
        let config = case(input.trim());
        // header 格式錯誤，整個 input 被略過
        assert!(config.cases.is_empty());
    }

    #[test]
    fn test_parse_unknown_key() {
        let input = r#"
banana 1
oops
input 1
hi
answer 1
yo
"#;
        let config = case(input.trim());
        // banana 是未知 key，會被略過
        assert_eq!(config.cases.len(), 1);
    }
}
