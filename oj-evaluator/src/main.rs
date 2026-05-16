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
    time::{Duration, Instant},
};

use compile::prepare_command;
use futures::{self, stream::StreamExt};
use judge::{
    evaluate,
    verdict::{CompileError, Limitation},
};
use reader::{TestInfo, resolve_args};
use shared::ShellCommand;
use tokio::time::interval;

use crate::{
    config::TEMP_DIR,
    judge::{
        display::{JudgeReport, print_test_info, print_test_label},
        verdict::JudgeVerdict,
    },
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

    if profile.compile.is_none() || (cache_state.is_fresh() && !info.force_compile) {
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

    let timer_task = async {
        let timer = Instant::now();
        loop {
            print!("\r🔨 正在編譯檔案 / {:.2}s", timer.elapsed().as_secs_f64());
            let _ = stdout().flush();
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    let compile_task = prepare_command(&info.file, profile, false, || {
        println!();
    });

    let result = tokio::select! {
        result = compile_task => result,
        _ = timer_task => {
            unreachable!();
        }
    };

    match result {
        Ok(cmd) => {
            let _ = cache_state
                .save()
                .inspect_err(|e| log::debug!("Write cache failed {e}"));            
            Some(cmd)
        },
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
    let mut report = JudgeReport::new();

    if let Some(warmup) = info.warmup_times
        && warmup > 0
        && let Some(case) = info.cases.first()
    {
        let mut short_limit = limit;
        short_limit.max_time(Some(Duration::from_millis(300)));
        for _ in 0..warmup {
            evaluate(&runner, &case.input, &case.answer, &short_limit).await;
        }
    }

    let concurrency_limit = num_cpus::get();
    let mut evaluation_stream = futures::stream::iter(info.cases.iter().enumerate())
        .map(|(idx, case)| {
            let runner = &runner;
            let limit = &limit;
            async move {
                let verdict = evaluate(runner, &case.input, &case.answer, limit).await;
                (idx + 1, verdict)
            }
        })
        .buffer_unordered(concurrency_limit);

    let mut solving_round = 1;
    let mut waiting_verdicts: HashMap<usize, JudgeVerdict<'_>> = HashMap::new();
    let mut ticker: tokio::time::Interval = interval(Duration::from_millis(100));
    let mut round_start_time = Instant::now();

    print_test_label(1);

    while solving_round <= test_rounds {
        tokio::select! {
            Some((round, verd)) = evaluation_stream.next() => {
                waiting_verdicts.insert(round, verd);
                while let Some(verdict) = waiting_verdicts.remove(&solving_round) {
                    round_start_time = Instant::now();

                    print_test_info(&verdict, &limit);
                    report.update(verdict, round);

                    solving_round += 1;
                    if solving_round > test_rounds {
                        break;
                    }
                    print_test_label(solving_round);
                }
            }

            _ = ticker.tick() => {
                if solving_round <= test_rounds {
                    let elapsed = round_start_time.elapsed();
                    if elapsed > Duration::from_millis(250) {
                        print!("執行中... {:.2}s\r", elapsed.as_secs_f64());
                        std::io::stdout().flush().unwrap();
                    }
                }
            }
        }
    }

    report.printstd();
}

fn execute(runner: ShellCommand) {
    println!("⚙️ 正在運行程式");
    let _ = runner.build().status();
}
