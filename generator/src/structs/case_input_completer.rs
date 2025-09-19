use inquire::{
    CustomUserError,
    autocompletion::{Autocomplete, Replacement},
};

pub const OPEN_EDITOR_MAGIC: &str = "開啟編輯器\u{200B}";

#[derive(Clone)]
pub struct CaseInputCompleter;

impl Autocomplete for CaseInputCompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, CustomUserError> {
        if input.trim().is_empty() {
            Ok(vec![OPEN_EDITOR_MAGIC.to_owned()])
        } else {
            Ok(Vec::new())
        }
    }

    fn get_completion(
        &mut self,
        _input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, CustomUserError> {
        Ok(highlighted_suggestion)
    }
}
