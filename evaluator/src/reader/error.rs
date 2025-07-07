use std::fmt;

#[derive(Debug)]
pub enum ReaderError {
  NoConfigFile(String),
  FileNotFound(String),
  General(String),
}

impl fmt::Display for ReaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
      match self {
        ReaderError::NoConfigFile(msg) => write!(f, "找不到配置檔：{}，考慮用'-n'參數直接執行程式", msg),
        ReaderError::FileNotFound(msg) => write!(f, "檔案不存在：{}", msg),
        ReaderError::General(msg) => write!(f, "{}", msg)
      }
    }
}

impl std::error::Error for ReaderError {}