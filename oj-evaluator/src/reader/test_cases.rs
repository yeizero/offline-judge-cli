use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use serde::de::{self, Deserializer, Visitor};
use std::{fmt, fs};

use super::error::ReaderError;

pub fn read_test_cases(path: TestCasePath) -> Result<TestCaseSet, ReaderError> {
    let path = match path {
        TestCasePath::Specified(p) => p,
        TestCasePath::NoExtension(p) => resolve_yaml_path(p)?,
    };
    let Ok(raw_str) = fs::read_to_string(&path) else {
        return Err(ReaderError::FileNotFound(path.into_string()));
    };

    let raw: RawTestCases =
        serde_yaml_ng::from_str(&raw_str).map_err(|e| ReaderError::General(e.to_string()))?;

    let cases = match raw.cases {
        CasesSource::List(cases) => cases,
        CasesSource::Path(dir) => {
            let mut config_folder = path;
            if !config_folder.pop() {
                return Err(ReaderError::General(
                    "Cannot get parent directory of config file".to_string(),
                ));
            }
            config_folder.push(dir);
            load_cases_from_folder(config_folder)?
        }
    };

    Ok(TestCaseSet {
        cases,
        limit: raw.limit,
    })
}

fn load_cases_from_folder(folder: Utf8PathBuf) -> Result<Vec<TestCase>, ReaderError> {
    if !folder.is_dir() {
        return Err(ReaderError::FolderNotFound(folder.into_string()));
    }

    let mut named_cases = Vec::<(String, TestCase)>::new();

    for entry in fs::read_dir(folder).map_err(|e| ReaderError::General(e.to_string()))? {
        let entry = entry.map_err(|e| ReaderError::General(e.to_string()))?;
        let path = entry.path();

        if !path.is_file() {
            log::debug!("Skip non-file entry: {}", path.display());
            continue;
        }
        if let Some(ext) = path.extension()
            && ext != "in"
        {
            if ext != "out" {
                log::debug!("Unknown file ignored: {}", path.display());
            }
            continue;
        }
        let Some(stem) = path.file_stem() else {
            log::debug!("Invalid filename (no stem): {}", path.display());
            continue;
        };

        let stem = stem.to_string_lossy();
        let output_path = path.with_file_name(format!("{stem}.out"));

        if !output_path.exists() {
            log::debug!(
                "Input file without matching .out ignored: {}",
                path.display()
            );
            continue;
        }

        let input = fs::read_to_string(&path).map_err(|e| {
            ReaderError::General(format!("Failed to read {}: {}", path.display(), e))
        })?;
        let answer = fs::read_to_string(&output_path).map_err(|e| {
            ReaderError::General(format!("Failed to read {}: {}", output_path.display(), e))
        })?;

        log::info!("Loaded test case: {stem}");

        named_cases.push((stem.into_owned(), TestCase { input, answer }));
    }

    named_cases.sort_by(|(n1, _), (n2, _)| natord::compare(n1, n2));

    Ok(named_cases.into_iter().map(|(_, case)| case).collect())
}

fn resolve_yaml_path<T: AsRef<Utf8Path>>(base_path: T) -> Result<Utf8PathBuf, ReaderError> {
    let base = base_path.as_ref();
    let yml_path = base.with_extension("yml");
    let yaml_path = base.with_extension("yaml");

    match (yml_path.exists(), yaml_path.exists()) {
        (true, false) => Ok(yml_path),
        (false, true) => Ok(yaml_path),
        (true, true) => Err(ReaderError::FileNotFound(format!(
            "配置檔衝突：同時存在 {yml_path} 和 {yaml_path}"
        ))),
        (false, false) => Err(ReaderError::NoConfigFile(yaml_path.into_string())),
    }
}

pub enum TestCasePath {
    Specified(Utf8PathBuf),
    NoExtension(Utf8PathBuf),
}

#[derive(Deserialize, Debug)]
pub struct TestCaseSet {
    pub cases: Vec<TestCase>,
    pub limit: Option<LimitInfo>,
}

#[derive(Deserialize, Debug)]
struct RawTestCases {
    pub cases: CasesSource,
    pub limit: Option<LimitInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum CasesSource {
    List(Vec<TestCase>),
    Path(String),
}

#[derive(Deserialize, Debug)]
pub struct TestCase {
    #[serde(deserialize_with = "string_or_number")]
    pub input: String,
    #[serde(deserialize_with = "string_or_number")]
    pub answer: String,
}

#[derive(Deserialize, Debug, Default)]
pub struct LimitInfo {
    pub memory: Option<usize>,
    pub time: Option<u64>,
}

fn string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrNumberVisitor;

    impl Visitor<'_> for StringOrNumberVisitor {
        type Value = String;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "string or number")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v)
        }

        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }

        fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.to_string())
        }
    }

    deserializer.deserialize_any(StringOrNumberVisitor)
}
