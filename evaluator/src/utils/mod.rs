use num_format::ToFormattedString;

use crate::config::NUMBER_FORMAT;

pub const TEMP_FILE_EXE: &str = "output.exe";

pub fn center_text(text: &str, total_length: usize, placeholder: &str) -> String {
    let text_length = text.len();
    if text_length >= total_length {
        return text.to_string();
    }

    let padding_length = (total_length - text_length) / 2;
    let left_padding = placeholder.repeat(padding_length);
    let right_padding = placeholder.repeat(total_length - text_length - padding_length);

    format!("{left_padding} {text} {right_padding}")
}

pub trait PrettyNumber {
    fn prettify(&self) -> String;
}

impl<T> PrettyNumber for T
where
    T: ToFormattedString,
{
    fn prettify(&self) -> String {
        self.to_formatted_string(&NUMBER_FORMAT)
    }
}
