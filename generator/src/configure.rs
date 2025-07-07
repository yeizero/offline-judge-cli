use std::{env, fs};
use serde::Deserialize;
// use env;
use shared::get_config_path;

pub fn read_config() -> Result<GeneratorConfig, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    let config_contents = fs::read_to_string(&config_path).map_err(|_| format!("Failed to read {}: file not found", config_path.display()))?;
    let root: ConfigRoot = serde_yml::from_str(&config_contents).map_err(|e| format!("Failed to read {}: {}", config_path.display(), e))?;
    Ok(root.as_config())
}

pub fn apply_config(config: &GeneratorConfig) {
  if let Some(editor) = &config.editor {
    if !editor.is_empty() {
      // SAFETY: Called before any threads are spawned, in the single-threaded init phase.
      unsafe { env::set_var("EDITOR", editor); }
    }
  }
}

#[derive(Debug, Deserialize)]
struct ConfigRoot {
  pub generator: Option<GeneratorConfig>,
}

impl ConfigRoot {
  pub fn as_config(self) -> GeneratorConfig {
    self.generator.unwrap_or_default()
  }
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneratorConfig {
    pub editor: Option<String>,
    pub plugins: Option<Vec<Plugin>>,
}

#[derive(Debug, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub command: String,
}

pub fn flatten_config(config: Option<GeneratorConfig>) -> GeneratorConfig {
    config.unwrap_or_default()
}