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
    /// 設定檔的路徑 (可選)。
    /// Path to the configuration file (optional).
    /// 若未提供，程式預設會尋找與輸入檔案同名的 .yaml 檔。
    /// If not provided, it default to a .yaml file with the same name as the input file.
    #[arg(short, long)]
    pub config: Option<String>,

    /// 要執行或測試的檔案路徑。
    /// The file path to execute or test.
    #[arg(index(1))]
    pub file: String,

    /// 指定檔案的程式語言 (可選)。
    /// The programming language for compiling or running (optional).
    #[arg(short, long)]
    pub lang: Option<String>,

    /// 設定單一測試案例的最大記憶體用量限制 (單位: KiB)。
    /// Maximum memory usage (in KiB) for a single test case.
    #[arg(short('M'), long)]
    pub memory: Option<usize>,

    /// 啟用「無評判模式」，此模式下不需要設定檔。
    /// Enable "No Judgement Mode", which does not require a config file.
    /// CLI: -n, --no-judge
    #[arg(short, long("no-judge"))]
    pub no_judge: bool,

    /// 設定單一測試案例的最大執行時間限制 (單位: 毫秒 ms)。
    /// Maximum time (in milliseconds) for a single test case.
    #[arg(short('T'), long)]
    pub time: Option<u64>,

    /// 啟用詳細輸出模式，顯示更多過程資訊。
    /// Enable verbose mode to print more process information.
    #[arg(short, long)]
    pub verbose: bool,

    /// 在正式測試前執行的預熱次數 (可選)。
    /// Number of warmup runs to perform before the actual test (optional).
    /// 用於穩定效能測試結果，例如讓 JIT 編譯器有時間最佳化。
    /// Used to stabilize performance results, e.g., by allowing a JIT compiler to warm up.
    #[arg(short, long)]
    pub warmup: Option<u32>,
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
}

impl TestInfo {
    pub fn with_config(&mut self, config: &EvaluatorConfig) {
        if self.warmup_times.is_none() {
            self.warmup_times = config.warmup;
        }
    }
}
