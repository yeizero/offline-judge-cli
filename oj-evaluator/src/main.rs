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
    collections::HashMap,
    io::{Write, stdout},
    process::ExitCode,
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
use tokio::{sync::Semaphore, task::JoinSet, time::interval};
use utils::PrettyNumber;

use crate::{
    config::TEMP_DIR,
    judge::verdict::JudgeVerdict,
    reader::{EvaluatorConfig, FileCacheState, ensure_dir_exists, read_config},
};

#[tokio::main]
async fn main() -> ExitCode {
    let mut info = match resolve_args() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("❌ [SE] {e}");
            return ExitCode::FAILURE;
        }
    };

    let config = match read_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("❌ [SE] {e}");
            return ExitCode::FAILURE;
        }
    };
    info.merge_config(&config);

    if let Err(e) = ensure_dir_exists(TEMP_DIR.as_path()) {
        eprintln!("❌ [SE] {e}");
        return ExitCode::FAILURE;
    }

    let Some(runner) = compile_source_code(&info, &config).await else {
        return ExitCode::FAILURE;
    };

    log::debug!("runner: {runner:?}");

    if info.do_judge {
        judge(info, runner).await;
    } else {
        execute(runner);
    }

    ExitCode::SUCCESS
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

    let cache_state = match FileCacheState::new(&info.file) {
        Ok(state) => state,
        Err(e) => {
            println!("{e}");
            return None;
        }
    };

    if profile.compile.is_none() || cache_state.is_fresh() {
        if profile.compile.is_some() {
            println!("📦 重複使用編譯檔案");
        }

        return match prepare_command(&info.file, profile, true, || {}).await {
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
                let _ = stdout()
                    .flush()
                    .inspect_err(|e| log::debug!("Flush Error: {e}"));
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
    };

    let result = {
        let is_timer_stop_write = Arc::clone(&is_timer_stop);
        prepare_command(&info.file, profile, false, move || {
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

    let _ = cache_state
        .save()
        .inspect_err(|e| log::debug!("Write cache failed {e}"));

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

struct ArcCases {
    input: Arc<String>,
    answer: Arc<String>,
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

    let runner_arc = Arc::new(runner);
    let cases_list: Vec<ArcCases> = info
        .cases
        .into_iter()
        .map(|case| ArcCases {
            input: Arc::new(case.input),
            answer: Arc::new(case.answer),
        })
        .collect();

    if let Some(warmup) = info.warmup_times
        && warmup > 0
        && let Some(case) = cases_list.first()
    {
        let mut short_limit = limit;
        short_limit.max_time(Some(Duration::from_millis(300)));

        for _ in 0..warmup {
            evaluate(
                Arc::clone(&runner_arc),
                Arc::clone(&case.input),
                Arc::clone(&case.answer),
                &short_limit,
            )
            .await;
        }
    }

    let concurrency_limit = num_cpus::get();
    let semaphore = Arc::new(Semaphore::new(concurrency_limit));
    let mut join_set: JoinSet<(usize, JudgeVerdict)> = JoinSet::new();

    for (idx, case) in cases_list.iter().enumerate() {
        let runner = Arc::clone(&runner_arc);
        let input = Arc::clone(&case.input);
        let answer = Arc::clone(&case.answer);
        let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();

        join_set.spawn(async move {
            let _permit = permit;
            (idx + 1, evaluate(runner, input, answer, &limit).await)
        });
    }

    let mut solving_round = 1;
    let mut waiting_verdict: HashMap<usize, JudgeVerdict> = HashMap::new();

    let mut ticker: tokio::time::Interval = interval(Duration::from_millis(100));
    let mut round_start_time = Instant::now();

    print_test_label(1);
    loop {
        tokio::select! {
            Some(result) = join_set.join_next() => {
                let (round, verd) = match result {
                    Ok(i) => i,
                    Err(e) => {
                        log::error!("A judge task failed: {e}");
                        return;
                    }
                };
                waiting_verdict.insert(round, verd);
                while let Some(verdict) = waiting_verdict.remove(&solving_round) {
                    round_start_time = Instant::now();

                    print_test_info(&verdict, &limit);

                    report_table.add_row(Row::new(vec![
                        Cell::new(if verdict.is_accept() { "✅" } else { "❌" }),
                        Cell::new(&solving_round.to_string()),
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

                    solving_round += 1;
                    if solving_round <= test_rounds {
                        print_test_label(solving_round);
                    } else {
                        break;
                    }
                }
            }

            _ = ticker.tick() => {
                if solving_round <= test_rounds {
                    let elapsed = round_start_time.elapsed();
                    if elapsed > Duration::from_millis(250) {
                        print!("執行中... {:.2}s\r", elapsed.as_secs_f64());
                        std::io::stdout().flush().unwrap();
                    }
                } else {
                    break;
                }
            }
        }
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
