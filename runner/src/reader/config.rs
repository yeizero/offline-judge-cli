use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use serde_yml;

use super::error::ReaderError;

pub fn read_config(path: ConfigPath) -> Result<Config, ReaderError> {
    let path  = match path {
        ConfigPath::Specified(p) => p,
        ConfigPath::NoExtension(p) => {
            resolve_yaml_path(p)?
        },
    };
    let raw_str = fs::read_to_string(&path)
        .map_err(|_| ReaderError::FileNotFound(path.to_string_lossy().to_string()))?;
    
    let config: Config = serde_yml::from_str(&raw_str)
        .map_err(|e| ReaderError::General(e.to_string()))?;

    Ok(config)
}

fn resolve_yaml_path<P: AsRef<Path>>(base_path: P) -> Result<PathBuf, ReaderError> {
    let base = base_path.as_ref();

    let yml_path = base.with_extension("yml");
    let yaml_path = base.with_extension("yaml");

    let yml_exists = yml_path.exists();
    let yaml_exists = yaml_path.exists();

    match (yml_exists, yaml_exists) {
        (true, false) => Ok(yml_path),
        (false, true) => Ok(yaml_path),
        (true, true) => Err(ReaderError::FileNotFound(format!(
            "配置檔衝突：同時存在 {} 和 {}",
            yml_path.display(),
            yaml_path.display()
        ))),
        (false, false) => Err(ReaderError::NoConfigFile(
            yaml_path.to_string_lossy().to_string()
        )),
    }
}

pub enum ConfigPath {
    Specified(PathBuf),
    NoExtension(PathBuf),
}

impl ConfigPath {
    pub fn specified<P: AsRef<Path>>(path: P) -> Self {
        ConfigPath::Specified(path.as_ref().to_path_buf())
    }

    pub fn no_extension<P: AsRef<Path>>(path: P) -> Self {
        ConfigPath::NoExtension(path.as_ref().to_path_buf())
    }
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub cases: Vec<TestCase>,
    pub limit: Option<LimitInfo>,
}

#[derive(Deserialize, Debug)]
pub struct TestCase {
    pub input: String,
    pub answer: String,
}

#[derive(Deserialize, Debug)]
pub struct LimitInfo {
    pub memory: Option<usize>,
    pub time: Option<u64>,
}

pub fn flatten_limit_info(limit: Option<LimitInfo>) -> LimitInfo {
    if let Some(l) = limit {
        l
    } else {
        LimitInfo {
            memory: None,
            time: None,
        }
    }
}
