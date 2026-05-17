use std::fmt;

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
            Self::NoConfigFile(msg) => {
                write!(f, "找不到配置檔：{msg}，考慮用'-n'參數直接執行程式")
            }
            Self::FileNotFound(msg) => write!(f, "檔案不存在：{msg}"),
            Self::FolderNotFound(msg) => write!(f, "資料夾不存在：{msg}"),
            Self::General(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ReaderError {}
