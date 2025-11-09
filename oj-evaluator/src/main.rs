#![deny(clippy::all)]
#![deny(clippy::if_then_some_else_none)]
#![deny(clippy::empty_enum_variants_with_brackets)]
#![deny(clippy::empty_structs_with_brackets)]
#![deny(clippy::separated_literal_suffix)]
#![deny(clippy::semicolon_outside_block)]
#![deny(clippy::non_zero_suggestions)]
#![deny(clippy::string_lit_chars_any)]
#![deny(clippy::use_self)]
#![deny(clippy::useless_let_if_seq)]
#![deny(clippy::branches_sharing_code)]
#![deny(clippy::equatable_if_let)]

mod compile;
mod config;
mod judge;
mod logger;
mod monitor;
mod reader;
mod utils;

use std::{
    io::{Write, stdout},
    process,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use compile::prepare_command;
use judge::{
    evaluate, print_test_info, print_test_label,
    verdict::{CompileError, Limitation, SummaryInfo},
};
use prettytable::{
    Cell, Row, Table,
    format::{FormatBuilder, LinePosition, LineSeparator},
};
use reader::{TestInfo, resolve_args};
use shared::ShellCommand;
use utils::PrettyNumber;

use crate::{
    config::TEMP_DIR,
    reader::{EvaluatorConfig, ensure_dir_exists, read_config},
};

#[tokio::main]
async fn main() {
    let mut info = resolve_args().unwrap_or_else(|e| {
        println!("❌ [SE] {e}");
        process::exit(1);
    });

    let config = read_config().unwrap_or_else(|e| {
        println!("❌ [SE] {e}");
        process::exit(1);
    });
    info.merge_config(&config);

    ensure_dir_exists(TEMP_DIR.as_path()).unwrap();

    let Some(runner) = compile_source_code(&info, &config).await else {
        process::exit(1);
    };

    log::debug!("runner: {runner:?}");

    if info.do_judge {
        judge(info, runner).await;
    } else {
        execute(runner);
    }
}

async fn compile_source_code(info: &TestInfo, config: &EvaluatorConfig) -> Option<ShellCommand> {
    let profile = config
        .languages
        .iter()
        .find(|lang| lang.extension == info.file_type);
    let Some(profile) = profile else {
        println!(
            "❌ [SE] 未知原始碼副檔名 {} ，請選擇 config.yaml 中含有的類型",
            info.file_type
        );
        return None;
    };

    if profile.compile.is_none() {
        return match prepare_command(&info.file, profile, || {}).await {
            Ok(cmd) => Some(cmd),
            Err(e) => {
                match e {
                    CompileError::SE(msg) => println!("❌ [SE] {msg}"),
                    CompileError::CE(msg) => println!("❌ [CE] {msg}"),
                };
                None
            }
        };
    }

    let is_timer_stop = Arc::new(AtomicBool::new(false));
    let timer_task = {
        let is_timer_stop_read = Arc::clone(&is_timer_stop);
        tokio::spawn(async move {
            let timer = Instant::now();
            while !is_timer_stop_read.load(Ordering::Relaxed) {
                print!("\r🔨 正在編譯檔案 / {:.2}s", timer.elapsed().as_secs_f64());
                let _ = stdout().flush().inspect_err(|e| log::debug!("Flush Error: {e}"));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };

    let result = {
        let is_timer_stop_write = Arc::clone(&is_timer_stop);
        prepare_command(&info.file, profile, move || {
            println!();
            is_timer_stop_write.store(true, Ordering::Relaxed);
        })
        .await
    };

    if is_timer_stop
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        println!();
    }

    let _ = timer_task.await;

    match result {
        Ok(cmd) => Some(cmd),
        Err(e) => {
            match e {
                CompileError::SE(msg) => println!("❌ [SE] {msg}"),
                CompileError::CE(msg) => println!("❌ [CE] {msg}"),
            };
            None
        }
    }
}

async fn judge(info: TestInfo, runner: ShellCommand) {
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

    if let Some(warmup) = info.warmup_times
        && warmup > 0
        && let Some(case) = info.cases.first()
    {
        for _ in 0..warmup {
            evaluate(&runner, &case.input, &case.answer, &limit).await;
        }
    }

    for case in info.cases.iter() {
        current_test_round += 1;
        print_test_label(current_test_round);

        let verdict = evaluate(&runner, &case.input, &case.answer, &limit).await;

        print_test_info(&verdict, &limit);

        report_table.add_row(Row::new(vec![
            Cell::new(if verdict.is_accept() { "✅" } else { "❌" }),
            Cell::new(&current_test_round.to_string()),
            Cell::new(&verdict.duration.map_or_else(
                || "Unknown".to_string(),
                |value| value.as_millis().prettify(),
            )),
            Cell::new(
                &verdict
                    .memory
                    .map_or_else(|| "Unknown".to_string(), |value| value.prettify()),
            ),
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

    println!("🎯 {summary_info}");
}

fn execute(runner: ShellCommand) {
    println!("⚙️ 正在運行程式");
    let _ = runner.build().status();
}
