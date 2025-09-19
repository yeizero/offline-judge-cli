use code_file::generate_code_file;
use configure::{apply_config, read_config};
use inquire::{InquireError, error::InquireResult};
use owo_colors::OwoColorize;
use test_cases::generate_test_case;

mod advanced;
mod code_file;
mod configure;
mod structs;
mod test_cases;
mod utils;

fn main() {
    let config_result = read_config();
    if let Err(e) = &config_result {
        warn!("錯誤，已忽略設置檔案", e);
    }
    let config = config_result.unwrap_or_default();
    apply_config(&config);

    let judge_config_path_option = resolve_inquire_error(generate_test_case(&config));
    if let Some(judge_config_path) = judge_config_path_option {
        resolve_inquire_error(generate_code_file(judge_config_path, &config));
    }
}

fn resolve_inquire_error<T>(result: InquireResult<T>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            eprintln!("\n{}", "> Operation terminated".red());
            None
        }
        Err(e) => {
            eprintln!("{}", e);
            None
        }
    }
}
