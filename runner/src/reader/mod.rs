use std::{path::Path, time::Duration};

use clap::Parser;

mod args;
mod config;
mod error;
mod utils;
pub use args::Args;
use config::{flatten_limit_info, read_config, ConfigPath, TestCase};
use error::ReaderError;
pub use utils::ensure_dir_exists;
use utils::{change_extension, file_exists};

use crate::logger::init_logger;

pub fn resolve_args() -> Result<TestInfo, ReaderError> {
    let args = Args::parse();

    init_logger(if args.verbose {log::LevelFilter::Debug} else {log::LevelFilter::Warn});
    
    log::debug!("{:?}", &args);

    if !file_exists(&args.file) {
        return Err(ReaderError::FileNotFound(args.file));
    }

    let file_type = match args.lang {
        Some(t) => t.to_file_type(),
        None => match Path::new(&args.file).extension() {
            Some(extension) => match extension.to_str() {
                Some("c") => FileType::C,
                Some("cpp") => FileType::Cpp,
                Some("java") => FileType::Java,
                Some("py") => FileType::Python,
                Some("rs") => FileType::Rust,
                Some("go") => FileType::Go,
                _ => FileType::Unknown((*extension.to_string_lossy()).to_owned()),
            },
            None => FileType::Unknown("".to_owned()),
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
        })
    } else {
        let config = read_config(if let Some(config) = args.config {
            ConfigPath::specified(config)
        } else {
            ConfigPath::no_extension(change_extension(&args.file, ""))
        })?;

        log::debug!("{:?}", &config);

        let config_limit = flatten_limit_info(config.limit);

        Ok(TestInfo {
            file_type,
            file: args.file,
            cases: config.cases,
            max_memory: args
                .memory
                .or_else(|| config_limit.memory),
            max_time: args
                .time
                .or_else(|| config_limit.time)
                .map(|t| Duration::from_millis(t)),
            do_judge: true,
        })        
    }
}

pub struct TestInfo {
    pub file: String,
    pub file_type: FileType,
    pub cases: Vec<TestCase>,
    pub max_memory: Option<usize>,
    pub max_time: Option<Duration>,
    pub do_judge: bool,
}

pub enum FileType {
    C,
    Cpp,
    Java,
    Python,
    Rust,
    Go,
    Unknown(String),
}