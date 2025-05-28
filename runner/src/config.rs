use std::path::PathBuf;
use std::process::Command;
use std::env;
use num_format::Locale;
use once_cell::sync::Lazy;

use crate::reader::ensure_dir_exists;

pub const CPP_COMPILER: &str = "g++";
pub const PYTHON_RUNNER: &str = "python";
pub const JAVA_COMPILER: &str = "javac";
pub const JAVA_RUNNER: &str = "java";
pub const C_COMPILER: &str = "gcc";
pub const RUST_COMPILER: &str = "rustc";
pub const GO_COMPILER: &str = "go";

pub fn resolve_cpp_args(command: &mut Command) -> &mut Command {
    command
        // .arg("-fsanitize=address")
        // .arg("-fsanitize=undefined")
        // .arg("-Wall")
        // .arg("-Wextra")
        // .arg("-Wconversion")
        .arg("-g")
        .arg("-O2")
        .arg("-std=gnu++11")
        .arg("-static")
        .arg("-lm")
}

pub fn resolve_java_args(command: &mut Command) -> &mut Command {
    command
        .arg("-client")
        .arg("-Xss8m")
        .arg("-Xmx1024m")
}

pub fn resolve_c_args(command: &mut Command) -> &mut Command {
    command
        .arg("-g")
        .arg("-O2")
        .arg("-std=gnu99")
        .arg("-static")
        .arg("-lm")
}

pub static TEMP_DIR: Lazy<PathBuf> = Lazy::new(|| {
    ensure_dir_exists(env::temp_dir().join(env!("CARGO_PKG_NAME")))
});

pub static NUMBER_FORMAT: Locale = Locale::en;