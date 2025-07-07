use std::path::PathBuf;
use std::env;
use num_format::Locale;
use once_cell::sync::Lazy;

use crate::reader::ensure_dir_exists;

pub static TEMP_DIR: Lazy<PathBuf> = Lazy::new(|| {
    ensure_dir_exists(env::temp_dir().join(env!("CARGO_PKG_NAME")))
});

pub static NUMBER_FORMAT: Locale = Locale::en;