mod args;
mod cache_state;
mod configure;
mod error;
mod test_cases;
mod utils;
pub use args::{TestInfo, resolve_args};
pub use cache_state::FileCacheState;
use clap::Parser;
pub use configure::{EvaluatorConfig, LanguageProfile, read_config};
pub use utils::ensure_dir_exists;

use crate::{
    logger::init_logger,
    reader::{args::Args, error::ReaderError},
};

pub fn load_test_info() -> Result<TestInfo, ReaderError> {
    let args = Args::parse();

    init_logger(if args.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Warn
    });

    let config = read_config()?;

    let info = resolve_args(args, config)?;

    log::debug!("{info:?}");

    Ok(info)
}
