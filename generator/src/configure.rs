use serde::Deserialize;
use shared::get_config_path;
use std::env;
use fs_err as fs;

pub fn read_config() -> anyhow::Result<GeneratorConfig> {
    let config_path = get_config_path()?;
    let config_contents = fs::read_to_string(&config_path)?;
    let root: ConfigRoot = serde_yml::from_str(&config_contents)?;
    Ok(root.into_config())
}

/// MUST be called in the single thread
pub fn apply_config(config: &GeneratorConfig) {
    if let Some(editor) = &config.editor
        && !editor.is_empty()
    {
        // SAFETY: Called before any threads are spawned, in the single-threaded init phase.
        unsafe {
            env::set_var("EDITOR", editor);
        }
    }
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
    pub editor: Option<String>,
    pub plugins: Option<Vec<Plugin>>,
    #[serde(skip_deserializing, default)]
    pub supported_code_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Plugin {
    pub name: String,
    pub command: String,
}

#[derive(Debug, Deserialize)]
pub struct PartialEvaluatorConfig {
    pub languages: Vec<PartialLanguageProfile>,
}

#[derive(Debug, Deserialize)]
pub struct PartialLanguageProfile {
    pub extension: String,
}