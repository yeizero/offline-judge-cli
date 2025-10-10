use serde::Deserialize;
use shared::get_config_path;
use std::fs;

use crate::reader::error::ReaderError;

#[derive(Debug, Deserialize)]
struct ConfigRoot {
    pub evaluator: EvaluatorConfig,
}

#[derive(Debug, Deserialize)]
pub struct EvaluatorConfig {
    pub languages: Vec<LanguageProfile>,
    pub warmup: Option<u32>,
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

pub fn read_config() -> Result<EvaluatorConfig, ReaderError> {
    let config_path = get_config_path().map_err(|e| ReaderError::General(e.to_string()))?;
    let config_contents = fs::read_to_string(&config_path)
        .map_err(|_| ReaderError::FileNotFound(config_path.to_string_lossy().to_string()))?;
    let root: ConfigRoot = serde_yml::from_str(&config_contents)
        .map_err(|e| ReaderError::General(format!("Failed to read {}: {}", config_path.display(), e)))?;
    log::debug!("{:?}", root.evaluator);
    Ok(root.evaluator)
}
