use std::{fmt, fs::File, path::PathBuf};

use inquire::{error::InquireResult, Select};
use owo_colors::OwoColorize;

use crate::utils::{test_create_file, FileStatus, SUPPORT_CODE_TYPES};

pub fn generate_code_file(judge_config_path: String) -> InquireResult<()> {
  let mut options = get_code_paths(&judge_config_path);
  options.insert(0, CodeOption::Cancel);
  let code_path= Select::new("生成同名程式檔", options)
    .prompt()?;
  if let CodeOption::File { path, .. } = code_path {
    File::create(&path)?;
    println!("{}", 
      format!( "{} {}", "檔案已創建:", path.display())
      .green()
    );
  } else {
    return Ok(());
  }
  Ok(())
}

fn get_code_paths(judge_config_path: &str) -> Vec<CodeOption> {
  let path = PathBuf::from(judge_config_path);
  let mut code_file_paths = vec![];
  SUPPORT_CODE_TYPES.iter().for_each(|code_type| {
    let code_path = path.with_extension(code_type);

    let status = test_create_file(&code_path);

    if !matches!(status, FileStatus::NotFound) {
      return;
    }

    let file_name = code_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    code_file_paths.push(CodeOption::File { 
      path: code_path, 
      file_name
    });
  });
  code_file_paths
}

pub enum CodeOption {
  Cancel,
  File {
    path: PathBuf,
    file_name: String,
  },
}

impl fmt::Display for CodeOption {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CodeOption::Cancel => write!(f, "取消"),
      CodeOption::File { file_name, .. } => {
        write!(f, "{}", file_name)
      }
    }
  }
}