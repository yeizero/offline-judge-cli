use super::error::ReaderError;
use super::test_cases::{TestCase, TestCasePath, read_test_cases};
use super::utils::{change_extension, file_exists};
use crate::logger::init_logger;
use crate::reader::EvaluatorConfig;
use clap::Parser;
use std::{path::Path, time::Duration};

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
    /// Recompile code ragardless of caching.
    pub recompile: bool,
}

pub fn resolve_args() -> Result<TestInfo, ReaderError> {
    let args = Args::parse();

    init_logger(if args.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    });

    log::debug!("{:?}", &args);

    if !file_exists(&args.file) {
        return Err(ReaderError::FileNotFound(args.file));
    }

    let file_type = match args.lang {
        Some(i) => i,
        None => match Path::new(&args.file).extension() {
            Some(extension) => extension.to_string_lossy().into_owned(),
            None => "".to_string(),
        },
    };

    if args.no_judge {
        Ok(TestInfo {
            file_type,
            file: args.file,
            cases: vec![],
            max_memory: None,
            max_time: None,
            do_judge: false,
            warmup_times: None,
            force_compile: args.recompile,
        })
    } else {
        let config = read_test_cases(if let Some(config) = args.config {
            TestCasePath::specified(config)
        } else {
            TestCasePath::no_extension(change_extension(&args.file, ""))
        })?;

        log::debug!("{:?}", &config);

        let config_limit = config.limit.unwrap_or_default();

        Ok(TestInfo {
            file_type,
            file: args.file,
            cases: config.cases,
            max_memory: args.memory.or(config_limit.memory),
            max_time: args.time.or(config_limit.time).map(Duration::from_millis),
            do_judge: true,
            warmup_times: args.warmup,
            force_compile: args.recompile,
        })
    }
}

pub struct TestInfo {
    pub file: String,
    pub file_type: String,
    pub cases: Vec<TestCase>,
    pub max_memory: Option<usize>,
    pub max_time: Option<Duration>,
    pub do_judge: bool,
    pub warmup_times: Option<u32>,
    pub force_compile: bool,
}

impl TestInfo {
    pub fn merge_config(&mut self, config: &EvaluatorConfig) {
        if self.warmup_times.is_none() {
            self.warmup_times = config.warmup;
        }
    }
}
