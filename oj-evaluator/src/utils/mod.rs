use std::borrow::Cow;

use num_format::ToFormattedString;

use crate::config::NUMBER_FORMAT;

pub const TEMP_FILE_EXE: &str = "output.exe";

pub fn center_text<'a>(text: &'a str, total_length: usize, placeholder: &'a str) -> Cow<'a, str> {
    let text_length = text.len();
    if text_length >= total_length {
        return Cow::Borrowed(text);
    }

    let padding_length = (total_length - text_length) / 2;
    let left_padding = placeholder.repeat(padding_length);
    let right_padding = placeholder.repeat(total_length - text_length - padding_length);

    Cow::Owned(format!("{left_padding} {text} {right_padding}"))
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
