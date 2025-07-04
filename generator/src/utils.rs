use std::env::current_dir;
use std::path::{Path, PathBuf};
use std::fs::File;

use inquire::validator::{Validation, ErrorMessage};

pub const ESCAPABLE: &str = "Esc to cancel";

pub const SUPPORT_CODE_TYPES: [&str; 6] = ["c", "cpp", "java", "py", "go", "rs"];

#[macro_export]
macro_rules! escapable {
    // 匹配 `match` 的結構
    ($expr:expr, $stmt:expr) => {
        match $expr {
            Err($crate::InquireError::OperationCanceled) => $stmt, // 如果錯誤是 `OperationCanceled`，就執行這裡的語句
            result => result, // 否則返回結果
        }
    };
}

pub fn get_exe_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        // debug
        let current_dir = env::current_dir()?;
        Ok(current_dir)
    } else {
        // release
        let exe_path = env::current_exe()?;
        let exe_dir = exe_path.parent().ok_or("Failed to get exe directory")?;
        Ok(exe_dir.to_path_buf())
    }
}

pub fn test_create_file<P: AsRef<Path>>(file_path: P) -> FileStatus {
  let path = match current_dir() {
      Ok(current) => current.join(file_path.as_ref()),
      Err(_) => PathBuf::from(file_path.as_ref())
  };
  
  // 檢查父目錄是否存在
  if let Some(parent) = path.parent() {
      if !parent.exists() {
          return FileStatus::ParentNotExists;
      }
  }

  if path.is_file() {
    return FileStatus::Exists;
  } 

  if path.is_dir() {
    return FileStatus::IsDir;
  }
  
  // 嘗試實際創建檔案
  match File::create(&path) {
      Ok(file) => {
          drop(file);
          let _ = std::fs::remove_file(&path);
          FileStatus::NotFound
      },
      Err(_) => {
          FileStatus::Failed
      }
  }
}

#[derive(Debug)]
pub enum FileStatus {
    NotFound,
    ParentNotExists,
    Failed,
    Exists,
    IsDir
}

impl FileStatus {
    pub fn to_str(&self) -> &str {
        match self {
            FileStatus::NotFound => "檔案不存在",
            FileStatus::Exists => "檔案已存在",
            FileStatus::ParentNotExists => "目標檔案的資料夾不存在",
            FileStatus::IsDir => "目標位置為資料夾",
            FileStatus::Failed => "檔案不合法",
        }
    }
}

pub fn file_path_validator(input: String) -> Result<Validation, Box<dyn std::error::Error + Send + Sync>> {
    if (&input).len() == 0 {
      return  Ok(Validation::Invalid(ErrorMessage::Custom("請輸入檔案名稱".to_owned())));
    }
    match test_create_file(&input) {
        FileStatus::NotFound => Ok(Validation::Valid),
        status @ FileStatus::Exists => Ok(Validation::Invalid(ErrorMessage::Custom(format!("{} ({})", &status.to_str(), &input)))),
        status => Ok(Validation::Invalid(ErrorMessage::Custom(status.to_str().to_owned()))),
    }
}

/// Truncate a string to a maximum length and append ellipsis if necessary.
pub fn with_ellipsis(input: &str, n: usize) -> String {
    let mut chars = input.chars();
    let mut result: String = chars.by_ref().take(n).collect();

    if chars.next().is_some() {
        result.push_str("...");
    }

    result
}

#[macro_export]
macro_rules! error {
    // 名稱 + 格式化字串 + 參數
    ($name:expr, $fmt:literal, $($arg:expr),+) => {
        println!(
            "{}",
            format!(concat!("! {}: ", $fmt), $name, $($arg),*).red()
        )
    };

    // 名稱 + 單一訊息
    ($name:expr, $msg:expr) => {
        println!(
            "{}",
            format!("! {}: {}", $name, $msg).red()
        )
    };

    // 單一格式化字串
    ($fmt:literal, $($arg:expr),+) => {
        println!(
            "{}",
            format!(concat!("! 錯誤: ", $fmt), $($arg),*).red()
        )
    };

    // 單一訊息
    ($msg:expr) => {
        println!(
            "{}",
            format!("! 錯誤: {}", $msg).red()
        )
    };
}

#[macro_export]
macro_rules! warn {
    // 1. 名稱 + 格式化字串 + 多個參數
    ($name:expr, $fmt:literal, $($arg:expr),+) => {
        println!(
            "{}",
            format!(concat!("! {}: ", $fmt), $name, $($arg),*).yellow()
        );
    };

    // 2. 名稱 + 單一訊息
    ($name:expr, $msg:expr) => {
        println!(
            "{}",
            format!("! {}: {}", $name, $msg).yellow()
        );
    };

    // 3. 單一格式化字串 + 多個參數
    ($fmt:literal, $($arg:expr),+) => {
        println!(
            "{}",
            format!(concat!("! 警告: ", $fmt), $($arg),*).yellow()
        );
    };

    // 4. 單一訊息
    ($msg:expr) => {
        println!(
            "{}",
            format!("! 警告: {}", $msg).yellow()
        );
    };
}

#[macro_export]
macro_rules! info {
    // 單一格式化字串
    ($fmt:literal, $($arg:expr),+) => {
        println!(
            "{}",
            format!(concat!("! ", $fmt), $($arg),*).bright_blue()
        );
    };

    // 單一訊息
    ($msg:expr) => {
        println!(
            "{}",
            format!("! {}", $msg).bright_blue()
        );
    };
}

#[cfg(test)]
use owo_colors::OwoColorize;

#[cfg(test)]
mod tests {
    // 引入你實現的 error 宏（這裡假設你有引入 colored 庫來支持 .red()）
    use super::*;

    // 測試 1: 簡單的錯誤訊息
    #[test]
    fn test_error_single_message() {
        // 這裡只會輸出錯誤訊息，並且測試永遠通過
        error!("無法開啟檔案");

        // 確保測試成功
        assert!(true);
    }

    // 測試 2: 錯誤名稱與錯誤訊息
    #[test]
    fn test_error_with_name() {
        // 錯誤名稱與錯誤訊息
        error!("檔案錯誤", "無法開啟檔案");

        // 確保測試成功
        assert!(true);
    }

    // 測試 3: 格式化錯誤訊息
    #[test]
    fn test_error_with_format_args() {
        // 帶有格式化參數的錯誤
        error!("讀取錯誤", "無法解析檔案: {}", "檔案不存在");

        // 確保測試成功
        assert!(true);
    }

    // 測試 4: 錯誤名稱與多個格式化參數
    #[test]
    fn test_error_with_name_and_format_args() {
        // 錯誤名稱與格式化參數
        error!("網路錯誤", "無法連線到 {}: {}", "example.com", 8080);

        // 確保測試成功
        assert!(true);
    }
}