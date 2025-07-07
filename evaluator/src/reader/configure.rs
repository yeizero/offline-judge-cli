use std::fs;
use serde::Deserialize;
use shared::get_config_path;

#[derive(Debug, Deserialize)]
struct ConfigRoot {
  pub evaluator: EvaluatorConfig,
}

#[derive(Debug, Deserialize)]
pub struct EvaluatorConfig {
    pub languages: Vec<LanguageProfile>,
}

#[derive(Debug, Deserialize)]
pub struct LanguageProfile {
    pub extension: String,
    pub compile: Option<CommandInstruction>,
    pub run: Option<CommandInstruction>,
}

#[derive(Debug, Deserialize)]
pub struct CommandInstruction {
    pub command: String,
}

pub fn read_config() -> Result<EvaluatorConfig, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    let config_contents = fs::read_to_string(&config_path).map_err(|_| format!("Failed to read {}: file not found", config_path.display()))?;
    let root: ConfigRoot = serde_yml::from_str(&config_contents).map_err(|e| format!("Failed to read {}: {}", config_path.display(), e))?;
    Ok(root.evaluator)
}