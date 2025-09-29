use fs_err as fs;
use serde::Deserialize;
use shared::{bridge::KeyMapListProtocal, get_config_path};
use std::collections::HashMap;

pub fn read_config() -> anyhow::Result<GeneratorConfig> {
    let config_path = get_config_path()?;
    let config_contents = fs::read_to_string(&config_path)?;
    let root: ConfigRoot = serde_yml::from_str(&config_contents)?;
    Ok(root.into_config())
}

#[derive(Debug, Deserialize)]
struct ConfigRoot {
    pub evaluator: PartialEvaluatorConfig,
    pub generator: Option<GeneratorConfig>,
}

impl ConfigRoot {
    pub fn into_config(self) -> GeneratorConfig {
        let mut config = self.generator.unwrap_or_default();

        config.supported_code_types = self
            .evaluator
            .languages
            .into_iter()
            .map(|lang| lang.extension)
            .collect();

        config
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct GeneratorConfig {
    #[serde(default)]
    pub editor: EditorChoice,
    pub plugins: Option<Vec<Plugin>>,
    #[serde(skip_deserializing, default)]
    pub supported_code_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EditorChoice {
    Local(EditorConfig),
    Other(String),
}

impl Default for EditorChoice {
    fn default() -> Self {
        Self::Local(EditorConfig::default())
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct EditorConfig {
    pub keymap: Option<KeyMapListProtocal>,
}

#[derive(Debug, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub config: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct PartialEvaluatorConfig {
    pub languages: Vec<PartialLanguageProfile>,
}

#[derive(Debug, Deserialize)]
pub struct PartialLanguageProfile {
    pub extension: String,
}
