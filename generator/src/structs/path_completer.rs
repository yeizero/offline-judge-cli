use fs_err as fs;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use inquire::{
    CustomUserError,
    autocompletion::{Autocomplete, Replacement},
};

use crate::utils::{FileStatus, test_create_file};

#[derive(Clone, Default)]
pub struct YamlPathCompleter {
    input: String,
    paths: Vec<String>,
    pub supported_code_types: Vec<String>,
}

impl YamlPathCompleter {
    pub fn supported_code_types(mut self, types: Vec<String>) -> Self {
        self.supported_code_types = types;
        self
    }

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

        let entries = fs::read_dir(scan_dir)?.collect::<Result<Vec<_>, _>>()?;

        for entry in entries {
            let mut path = entry.path();
            if path.is_file()
                && path
                    .extension()
                    .map(|ext| ext.to_string_lossy())
                    .map(|ext| self.supported_code_types.iter().any(|s| s == ext.as_ref()))
                    .unwrap_or(false)
            {
                path.set_extension("yaml");
                let status = test_create_file(&path);
                if matches!(status, FileStatus::NotFound) {
                    self.paths.push(
                        path.to_string_lossy()
                            .replace("\\", "/")
                            .trim_start_matches("./")
                            .to_owned(),
                    );
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

fn fuzzy_sort(input: &str, vecs: &[String]) -> Vec<(String, i64)> {
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
