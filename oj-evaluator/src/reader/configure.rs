use serde::Deserialize;
use shared::get_config_path;
use std::fs;

use crate::reader::error::ReaderError;

pub const MIB: usize = 1024 * 1024;
const DEFAULT_OUTPUT_LIMIT_MIB: usize = 16;

const fn default_output_limit_mib() -> usize {
    DEFAULT_OUTPUT_LIMIT_MIB
}

#[derive(Debug, Deserialize)]
struct ConfigRoot {
    pub evaluator: EvaluatorConfig,
}

#[derive(Debug, Deserialize)]
pub struct EvaluatorConfig {
    pub languages: Vec<LanguageProfile>,
    pub warmup: Option<u32>,
    #[serde(default = "default_output_limit_mib")]
    pub stdout_limit: usize,
    #[serde(default = "default_output_limit_mib")]
    pub stderr_limit: usize,
}

impl EvaluatorConfig {
    fn output_limit_bytes(name: &str, value: usize) -> Result<usize, ReaderError> {
        if value == 0 {
            return Err(ReaderError::General(format!(
                "evaluator.{name} must be greater than zero"
            )));
        }
        value
            .checked_mul(MIB)
            .ok_or_else(|| ReaderError::General(format!("evaluator.{name} is too large")))
    }

    pub fn stdout_limit_bytes(&self) -> Result<usize, ReaderError> {
        Self::output_limit_bytes("stdout_limit", self.stdout_limit)
    }

    pub fn stderr_limit_bytes(&self) -> Result<usize, ReaderError> {
        Self::output_limit_bytes("stderr_limit", self.stderr_limit)
    }
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
    let root: ConfigRoot = serde_yaml_ng::from_str(&config_contents).map_err(|e| {
        ReaderError::General(format!("Failed to read {}: {}", config_path.display(), e))
    })?;
    log::debug!("{:?}", root.evaluator);
    Ok(root.evaluator)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{ConfigRoot, MIB};

    fn parse(yaml: &str) -> ConfigRoot {
        serde_yaml_ng::from_str(yaml).expect("test YAML must parse")
    }

    #[test]
    fn output_limits_default_to_16_mib() {
        let root = parse("evaluator:\n  languages: []\n");
        assert_eq!(root.evaluator.stdout_limit_bytes().unwrap(), 16 * MIB);
        assert_eq!(root.evaluator.stderr_limit_bytes().unwrap(), 16 * MIB);
    }

    #[test]
    fn custom_output_limits_are_converted_to_bytes() {
        let root = parse("evaluator:\n  languages: []\n  stdout_limit: 2\n  stderr_limit: 3\n");
        assert_eq!(root.evaluator.stdout_limit_bytes().unwrap(), 2 * MIB);
        assert_eq!(root.evaluator.stderr_limit_bytes().unwrap(), 3 * MIB);
    }

    #[test]
    fn zero_output_limit_is_rejected() {
        let root = parse("evaluator:\n  languages: []\n  stdout_limit: 0\n  stderr_limit: 16\n");
        assert!(root.evaluator.stdout_limit_bytes().is_err());
    }

    #[test]
    fn invalid_output_limit_type_is_rejected_by_yaml() {
        let result = serde_yaml_ng::from_str::<ConfigRoot>(
            "evaluator:\n  languages: []\n  stdout_limit: many\n",
        );
        assert!(result.is_err());
    }

    #[test]
    fn overflowing_output_limit_is_rejected() {
        let root = parse(&format!(
            "evaluator:\n  languages: []\n  stdout_limit: {}\n  stderr_limit: 16\n",
            usize::MAX
        ));
        assert!(root.evaluator.stdout_limit_bytes().is_err());
    }
}
