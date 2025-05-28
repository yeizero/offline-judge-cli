use std::fs;
use serde::Deserialize;
use env;

use crate::utils::get_exe_dir;

pub fn read_setting() -> Result<Setting, Box<dyn std::error::Error>> {
    // 獲取執行檔的路徑
    let exe_dir = get_exe_dir()?;

    let config_path = exe_dir.join("config.yaml");

    let config_contents = fs::read_to_string(&config_path).map_err(|_| format!("Failed to read {}: file not found", config_path.display()))?;

    let root: SettingRoot = serde_yml::from_str(&config_contents).map_err(|e| format!("Failed to read {}: {}", config_path.display(), e))?;

    Ok(root.as_config())
}

pub fn apply_setting(config: &Setting) {
  if let Some(editor) = &config.editor {
    if !editor.is_empty() {
      let status = env::set_var("EDITOR", editor);
      if status.is_none() {
        eprintln!("[Warn] Failed to set editor environment variable");
      }
    }
  }
}

#[derive(Debug, Deserialize)]
struct SettingRoot {
  pub generator: Option<Setting>,
}

impl SettingRoot {
  pub fn as_config(self) -> Setting {
    self.generator.unwrap_or_default()
  }
}

#[derive(Debug, Deserialize, Default)]
pub struct Setting {
    pub editor: Option<String>,
    pub plugins: Option<Vec<Plugin>>,
}

#[derive(Debug, Deserialize)]
pub struct Plugin {
    pub option: String,
    pub command: Vec<String>,
}

pub fn flatten_config(config: Option<Setting>) -> Setting {
    config.unwrap_or_default()
}