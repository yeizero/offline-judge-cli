use super::error::ReaderError;
use super::test_cases::{TestCase, TestCasePath, read_test_cases};
use crate::reader::{EvaluatorConfig, LanguageProfile};
use camino::Utf8PathBuf;
use clap::Parser;
use std::time::Duration;

/// Evaluator - Code Judge Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the configuration file (optional).
    /// If not provided, it default to a .yaml file with the same name as the input file.
    #[arg(short, long)]
    pub config: Option<String>,

    /// The file path to execute or test.
    #[arg(index(1))]
    pub file: String,

    /// The programming language for compiling or running (optional).
    #[arg(short, long)]
    pub lang: Option<String>,

    /// Maximum memory usage (in KiB) for a single test case.
    #[arg(short('M'), long)]
    pub memory: Option<usize>,

    /// Enable "No Judgement Mode", which does not require a config file.
    #[arg(short, long("no-judge"))]
    pub no_judge: bool,

    /// Maximum time (in milliseconds) for a single test case.
    #[arg(short('T'), long)]
    pub time: Option<u64>,

    /// Enable verbose mode.
    #[arg(short, long)]
    pub verbose: bool,

    /// Number of warmup runs to perform before the actual test (optional).
    /// Used to stabilize performance results, e.g., by allowing a JIT compiler to warm up.
    #[arg(short, long)]
    pub warmup: Option<u32>,

    #[arg(short, long)]
    /// Force to recompile code ragardless of caching.
    pub force: bool,
}

pub fn resolve_args(args: Args, config: EvaluatorConfig) -> Result<TestInfo, ReaderError> {
    let max_stdout = config.stdout_limit_bytes()?;
    let max_stderr = config.stderr_limit_bytes()?;
    let mut path = Utf8PathBuf::from(args.file);

    let final_ext: &str = match &args.lang {
        Some(lang) => {
            if !path.is_file() {
                return Err(ReaderError::FileNotFound(lang.clone()));
            }
            lang
        }
        None => {
            if path.is_file() {
                path.extension().unwrap_or("")
            } else if path.extension().is_none() {
                let mut found_ext = None;
                for lang_profile in &config.languages {
                    path.set_extension(&lang_profile.extension);
                    if path.is_file() {
                        if let Some(old_ext) = &found_ext {
                            return Err(ReaderError::General(format!(
                                "發現多個可能的副檔名 (.{} vs .{})",
                                old_ext, lang_profile.extension
                            )));
                        }
                        found_ext = Some(&lang_profile.extension);
                    }
                }

                let Some(ext) = found_ext else {
                    path.set_extension("");
                    return Err(ReaderError::FileNotFound(path.into_string()));
                };

                path.set_extension(ext);
                path.extension().unwrap_or("")
            } else {
                return Err(ReaderError::FileNotFound(path.into_string()));
            }
        }
    };

    let file_profile = config
        .languages
        .into_iter()
        .find(|l| l.extension == final_ext)
        .ok_or_else(|| {
            ReaderError::General(format!(
                "未知原始碼副檔名 {final_ext} ，請選擇 config.yaml 中含有的類型"
            ))
        })?;

    if args.no_judge {
        Ok(TestInfo {
            file: path.into_string(),
            file_profile,
            cases: Vec::new(),
            max_memory: None,
            max_time: None,
            max_stdout,
            max_stderr,
            do_judge: false,
            warmup_times: None,
            force_compile: args.force,
        })
    } else {
        let case_set = read_test_cases(if let Some(config) = args.config {
            TestCasePath::Specified(Utf8PathBuf::from(config))
        } else {
            let mut test_case_path = path.clone();
            test_case_path.set_extension("");
            TestCasePath::NoExtension(test_case_path)
        })?;

        let case_limit = case_set.limit.unwrap_or_default();

        Ok(TestInfo {
            file: path.into_string(),
            file_profile,
            cases: case_set.cases,
            max_memory: args.memory.or(case_limit.memory),
            max_time: args.time.or(case_limit.time).map(Duration::from_millis),
            max_stdout,
            max_stderr,
            do_judge: true,
            warmup_times: args.warmup.or(config.warmup),
            force_compile: args.force,
        })
    }
}

#[derive(Debug)]
pub struct TestInfo {
    pub file: String,
    pub file_profile: LanguageProfile,
    pub cases: Vec<TestCase>,
    pub max_memory: Option<usize>,
    pub max_time: Option<Duration>,
    pub max_stdout: usize,
    pub max_stderr: usize,
    pub do_judge: bool,
    pub warmup_times: Option<u32>,
    pub force_compile: bool,
}
