use num_format::Locale;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

pub static TEMP_DIR: LazyLock<PathBuf> =
    LazyLock::new(|| env::temp_dir().join(env!("CARGO_PKG_NAME")));

pub const NUMBER_FORMAT: Locale = Locale::en;
