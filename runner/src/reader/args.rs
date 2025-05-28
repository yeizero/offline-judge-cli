use clap::{Parser, ValueEnum};

use super::FileType;

/// Code Judge Tool
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// File path
    #[arg(index(1))]
    pub file: String,

    /// Config path for testing program. Default to [file path].yaml
    #[arg(short, long)]
    pub config: Option<String>,

    /// Programming language for compiling or running.
    #[arg(short, long)]
    pub lang: Option<ArgFileLang>,

    /// No Judgement Mode. No config file needed if enabled
    #[arg(short, long("no-judge"))]
    pub no_judge: bool,

    /// Maximum time (ms) for a test case.
    #[arg(short('T'), long)]
    pub time: Option<u64>,

    /// Maximum memory usage (KiB) for a test case.
    #[arg(short('M'), long)]
    pub memory: Option<usize>,

    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ArgFileLang {
    C,    
    Cpp,
    Java,    
    Python,
    Rust,
    Go,
}

impl ArgFileLang {
    pub fn to_file_type(&self) -> FileType {
        match self {
            ArgFileLang::C => FileType::C,
            ArgFileLang::Cpp => FileType::Cpp,
            ArgFileLang::Java => FileType::Java,
            ArgFileLang::Python => FileType::Python,
            ArgFileLang::Rust => FileType::Rust,
            ArgFileLang::Go => FileType::Go,
        }
    }
}
