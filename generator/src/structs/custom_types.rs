use std::{fmt, str::FromStr};

#[derive(Debug)]
pub struct OptionalInput<T> {
    pub value: Option<T>,
}

impl<T> OptionalInput<T> {
    pub fn new(value: Option<T>) -> Self {
        OptionalInput { value }
    }
}

impl<T> Clone for OptionalInput<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        OptionalInput {
            value: self.value.clone(),
        }
    }
}

impl<T> FromStr for OptionalInput<T>
where
    T: FromStr,
{
    type Err = <T as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().parse::<T>() {
            Ok(value) => Ok(OptionalInput::new(Some(value))),
            Err(e) => {
                if s.is_empty() {
                    Ok(OptionalInput::new(None))
                } else {
                    Err(e)
                }
            }
        }
    }
}

impl<T> fmt::Display for OptionalInput<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            Some(v) => write!(f, "{}", v),
            None => write!(f, ""),
        }
    }
}

pub struct LabelWithOptionIndex {
    pub label: String,
    pub index: Option<usize>,
}

impl fmt::Display for LabelWithOptionIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label)
    }
}

impl LabelWithOptionIndex {
    pub fn new(index: Option<usize>, label: String) -> Self {
        Self { label, index }
    }
}
