mod config;
mod logger;
mod measure;
mod reader;

use std::process::{self, Command};

use measure::{
    CompileError, Limitation, PrettyNumber, SummaryInfo, evaluate, prepare_command,
    print_test_info, print_test_label,
};
use prettytable::{
    Cell, Row, Table,
    format::{FormatBuilder, LinePosition, LineSeparator},
};
use reader::{TestInfo, resolve_args};

use crate::reader::{EvaluatorConfig, read_config};

fn main() {
    let info = match resolve_args() {
        Ok(i) => i,
        Err(e) => {
            println!("❌ [SE] {}", e);
            process::exit(1);
        }
    };
    let config = match read_config() {
        Ok(i) => i,
        Err(e) => {
            println!("❌ [SE] {}", e);
            process::exit(1);
        }
    };

    let Some(runner) = compile_source_code(&info, &config) else {
        process::exit(1);
    };

    log::debug!("runner: {:?}", runner);

    if info.do_judge {
        judge(info, runner);
    } else {
        execute(runner);
    }
}

fn compile_source_code(info: &TestInfo, config: &EvaluatorConfig) -> Option<Command> {
    let profile = config.languages.iter().find(|lang| lang.extension == info.file_type);
    let Some(profile) = profile else {
        println!("❌ [SE] 未知原始碼副檔名 {} ，請選擇 config.yaml 中含有的類型", info.file_type);
        return None;
    };

    if profile.compile.is_some() {
        println!("🔨 正在編譯檔案");
    }

    match prepare_command(&info.file, profile) {
        Ok(i) => Some(i),
        Err(e) => {
            match e {
                CompileError::SE(msg) => println!("❌ [SE] {}", msg),
                CompileError::CE(msg) => println!("❌ [CE] {}", msg),
            };
            None
        }
    }
}

fn judge(info: TestInfo, mut runner: Command) {
    let mut limit = Limitation::default();

    if let Some(time) = info.max_time {
        limit.max_time(Some(time));
    }

    if let Some(memory) = info.max_memory {
        limit.max_memory(Some(memory));
    }

    let test_rounds: usize = info.cases.len();
    let mut summary_info = SummaryInfo::default();
    let mut current_test_round: u32 = 0;

    let mut report_table = Table::new();
    report_table.set_format(
        FormatBuilder::new()
            .padding(1, 1)
            .separator(LinePosition::Title, LineSeparator::new('=', '+', '|', '|'))
            .separator(
                LinePosition::Bottom,
                LineSeparator::new('-', '+', '\'', '\''),
            )
            .separator(LinePosition::Top, LineSeparator::new('-', '+', '.', '.'))
            .borders('|')
            .build(),
    );
    report_table.set_titles(Row::new(vec![
        Cell::new(""),
        Cell::new("測資"),
        Cell::new("用時 (ms)"),
        Cell::new("記憶體 (KiB)"),
        Cell::new("結果"),
    ]));

    for case in info.cases.iter() {
        current_test_round += 1;
        print_test_label(current_test_round);

        let verdict = evaluate(&mut runner, &case.input, &case.answer, &limit);

        print_test_info(&verdict, &limit);

        report_table.add_row(Row::new(vec![
            Cell::new(if verdict.is_accept() { "✅" } else { "❌" }),
            Cell::new(&current_test_round.to_string()),
            Cell::new(&match verdict.duration {
                Some(value) => value.as_millis().prettify(),
                None => "Unknown".to_owned(),
            }),
            Cell::new(&match verdict.memory {
                Some(value) => value.prettify(),
                None => "Unknown".to_owned(),
            }),
            Cell::new(verdict.status.to_str_short()),
        ]));

        summary_info.update(verdict);
    }

    println!(
        "\n📝 總結: {:>33}",
        format!(
            "正確 {} 錯誤 {} 正確比 {}%",
            summary_info.success_rounds,
            test_rounds - summary_info.success_rounds,
            summary_info.score()
        )
    );
    report_table.printstd();

    println!("{}", format!("🎯 {}", summary_info));
}

fn execute(mut runner: Command) {
    println!("⚙️ 正在運行程式");
    let _ = runner.status();
}
