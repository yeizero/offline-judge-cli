mod config;
mod logger;
mod measure;
mod reader;

use std::process::{self, Command};

use measure::{
    compile, measure, print_test_info, print_test_label, CompileError, Limitation, PrettyNumber, SummaryInfo
};
use prettytable::{
    format::{FormatBuilder, LinePosition, LineSeparator},
    Cell, Row, Table,
};
use reader::{resolve_args, FileType, TestInfo};

fn main() {
    let info = match resolve_args() {
        Ok(i) => i,
        Err(e) => {
            println!("❌ [SE] {}", e);
            process::exit(1);
        }
    };

    let Some(runner) = compile_source_code(&info) else {
        process::exit(1);
    };

    log::debug!("runner: {:?}", runner);

    if info.do_judge {
        judge(info, runner);
    } else {
        execute(runner);
    }
}

fn compile_source_code(info: &TestInfo) -> Option<Command> {
    if !matches!(info.file_type, FileType::Python) {
        println!("🔨 正在編譯檔案");
    }

    let compile = match &info.file_type {
        FileType::C => compile::resolve_c,
        FileType::Cpp => compile::resolve_cpp,
        FileType::Java => compile::resolve_java,
        FileType::Python => compile::resolve_python,
        FileType::Rust => compile::resolve_rust,
        FileType::Go => compile::resolve_go,
        FileType::Unknown(ext) => {
            println!("❌ [SE] 無法編譯副檔名 為 '{ext}' 的檔案，請用 --type 指定檔案類型");
            return None;
        }
    };

    Some(match compile(&info.file) {
        Ok(i) => i,
        Err(e) => {
            match e {
                CompileError::SE(msg) => println!("❌ [SE] {}", msg),
                CompileError::CE(msg) => println!("❌ [CE] {}", msg),
            };

            process::exit(1);
        }
    })
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

        let verdict = measure(&mut runner, &case.input, &case.answer, &limit);

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
