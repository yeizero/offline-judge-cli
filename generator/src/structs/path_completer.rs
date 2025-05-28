use std::io::ErrorKind;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use inquire::{
    autocompletion::{Autocomplete, Replacement},
    CustomUserError,
};

use crate::utils::{test_create_file, FileStatus, SUPPORT_CODE_TYPES};

#[derive(Clone, Default)]
pub struct FilePathCompleter {
    input: String,
    paths: Vec<String>,
}

impl FilePathCompleter {
    fn update_input(&mut self, input: &str) -> Result<(), CustomUserError> {
        if input == self.input && !self.paths.is_empty() {
            return Ok(());
        }

        self.input = input.to_owned();
        self.paths.clear();

        let input_path = std::path::PathBuf::from(input);

        let fallback_parent = input_path
            .parent()
            .map(|p| {
                if p.to_string_lossy() == "" {
                    std::path::PathBuf::from(".")
                } else {
                    p.to_owned()
                }
            })
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let scan_dir = if input.ends_with('/') {
            input_path
        } else {
            fallback_parent.clone()
        };

        let entries = match std::fs::read_dir(scan_dir) {
            Ok(read_dir) => Ok(read_dir),
            Err(err) if err.kind() == ErrorKind::NotFound => std::fs::read_dir(fallback_parent),
            Err(err) => Err(err),
        }?
        .collect::<Result<Vec<_>, _>>()?;

        for entry in entries {
            let path = entry.path();
            if path.is_file() {
              self.paths.push(path.to_string_lossy().replace("\\", "/").trim_start_matches("./").to_string());
            }
        }

        Ok(())
    }

    fn fuzzy_sort(&self, input: &str) -> Vec<(String, i64)> {
        fuzzy_sort(input, &self.paths)
    }
}

impl Autocomplete for FilePathCompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        self.update_input(input)?;

        let matches = self.fuzzy_sort(input);
        Ok(matches.into_iter().take(15).map(|(path, _)| path).collect())
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        self.update_input(input)?;

        Ok(if let Some(suggestion) = highlighted_suggestion {
            Replacement::Some(suggestion)
        } else {
            let matches = self.fuzzy_sort(input);
            matches
                .first()
                .map(|(path, _)| Replacement::Some(path.clone()))
                .unwrap_or(Replacement::None)
        })
    }
}

#[derive(Clone, Default)]
pub struct YamlPathCompleter {
    input: String,
    paths: Vec<String>,
}

impl YamlPathCompleter {
    fn update_input(&mut self, input: &str) -> Result<(), CustomUserError> {
        if input == self.input && !self.paths.is_empty() {
            return Ok(());
        }

        self.input = input.to_owned();
        self.paths.clear();

        let input_path = std::path::PathBuf::from(input);

        let fallback_parent = input_path
            .parent()
            .map(|p| {
                if p.to_string_lossy() == "" {
                    std::path::PathBuf::from(".")
                } else {
                    p.to_owned()
                }
            })
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        let scan_dir = if input.ends_with('/') {
            input_path
        } else {
            fallback_parent.clone()
        };

        if !scan_dir.is_dir() {
            return Ok(());
        }
        

        let entries = match std::fs::read_dir(scan_dir) {
            Ok(read_dir) => Ok(read_dir),
            err => err
        }?
        .collect::<Result<Vec<_>, _>>()?;

        for entry in entries {
            let mut path = entry.path();
            if path.is_file() 
            && path.extension()
                .and_then(|ext| Some(ext.to_string_lossy()))
                .map(|ext| SUPPORT_CODE_TYPES.contains(&&*ext)) 
                .unwrap_or(false) {
                path.set_extension("yaml");
                let status = test_create_file(&path);
                if matches!(status, FileStatus::NotFound) {
                    self.paths.push(path.to_string_lossy().replace("\\", "/").trim_start_matches("./").to_string());
                }
            }
        }

        Ok(())
    }

    fn fuzzy_sort(&self, input: &str) -> Vec<(String, i64)> {
        fuzzy_sort(input, &self.paths)
    }
}

impl Autocomplete for YamlPathCompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        self.update_input(input)?;

        let matches = self.fuzzy_sort(input);
        Ok(matches.into_iter().take(15).map(|(path, _)| path).collect())
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        self.update_input(input)?;

        Ok(if let Some(suggestion) = highlighted_suggestion {
            Replacement::Some(suggestion)
        } else {
            let matches = self.fuzzy_sort(input);
            matches
                .first()
                .map(|(path, _)| Replacement::Some(path.clone()))
                .unwrap_or(Replacement::None)
        })
    }
}

fn fuzzy_sort(input: &str, vecs: &Vec<String>) -> Vec<(String, i64)> {
    let mut matches: Vec<(String, i64)> = vecs
        .iter()
        .filter_map(|path| {
            SkimMatcherV2::default()
                .smart_case()
                .fuzzy_match(path, input)
                .map(|score| (path.clone(), score))
        })
        .collect();

    matches.sort_by(|a, b| b.1.cmp(&a.1));
    matches
}