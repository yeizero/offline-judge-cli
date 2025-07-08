use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use serde_yml;

use super::error::ReaderError;

pub fn read_test_cases(path: TestCasePath) -> Result<TestCases, ReaderError> {
    let path  = match path {
        TestCasePath::Specified(p) => p,
        TestCasePath::NoExtension(p) => {
            resolve_yaml_path(p)?
        },
    };
    let raw_str = fs::read_to_string(&path)
        .map_err(|_| ReaderError::FileNotFound(path.to_string_lossy().into_owned()))?;
    
    let cases: TestCases = serde_yml::from_str(&raw_str)
        .map_err(|e| ReaderError::General(e.to_string()))?;

    Ok(cases)
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
            yaml_path.to_string_lossy().into_owned()
        )),
    }
}

pub enum TestCasePath {
    Specified(PathBuf),
    NoExtension(PathBuf),
}

impl TestCasePath {
    pub fn specified<P: AsRef<Path>>(path: P) -> Self {
        TestCasePath::Specified(path.as_ref().to_path_buf())
    }

    pub fn no_extension<P: AsRef<Path>>(path: P) -> Self {
        TestCasePath::NoExtension(path.as_ref().to_path_buf())
    }
}

#[derive(Deserialize, Debug)]
pub struct TestCases {
    pub cases: Vec<TestCase>,
    pub limit: Option<LimitInfo>,
}

#[derive(Deserialize, Debug)]
pub struct TestCase {
    pub input: String,
    pub answer: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct LimitInfo {
    pub memory: Option<usize>,
    pub time: Option<u64>,
}