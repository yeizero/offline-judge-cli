use code_file::generate_code_file;
use setting_reader::{apply_setting, flatten_config, read_setting};
use inquire::{error::InquireResult, InquireError};
use judge_config::generate_judge_config;
use owo_colors::OwoColorize;

mod utils;
mod judge_config;
mod structs;
mod setting_reader;
mod code_file;
mod advance;

fn main() {
    let setting_result = read_setting();
    if let Err(e) = &setting_result {
        warn!("錯誤，已忽略設置檔案", e);
    }
    let setting = flatten_config(setting_result.ok());
    apply_setting(&setting);

    let judge_config_path_option = resolve_inquire_error(generate_judge_config(&setting));
    if let Some(judge_config_path) = judge_config_path_option {
        resolve_inquire_error(generate_code_file(judge_config_path));
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