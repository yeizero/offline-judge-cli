use std::fmt;

use shared::tr;

#[derive(Debug)]
pub enum ReaderError {
    NoConfigFile(String),
    FileNotFound(String),
    FolderNotFound(String),
    General(String),
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NoConfigFile(path) => {
                write!(f, "{}", tr!(NoConfigFile { path }))
            }
            Self::FileNotFound(path) => write!(f, "{}", tr!(FileNotFound { path })),
            Self::FolderNotFound(path) => write!(f, "{}", tr!(FolderNotFound { path })),
            Self::General(path) => write!(f, "{path}"),
        }
    }
}

impl std::error::Error for ReaderError {}
