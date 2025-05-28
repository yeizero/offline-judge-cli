use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use memory::create_memory_monitor;
pub use structs::{CompileError, JudgeStatus, JudgeVerdict, Limitation, SummaryInfo};
pub use utils::PrettyNumber;
use utils::{center_text, compare_lines_ignoring_line_endings};

pub mod compile;
mod structs;
mod utils;

mod memory;

const INFO_SPACE: usize = 30;

pub fn measure<'a>(
    runner: &mut Command,
    input: &'a str,
    ans: &'a str,
    limit: &Limitation,
) -> JudgeVerdict<'a> {
    let ans = ans.trim_end();
    let mut verdict: JudgeVerdict<'a> = JudgeVerdict::new(input, ans);

    let mut child = runner
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("無法啟動執行檔");

    let start_time = Instant::now();

    let pid = child.id();

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).unwrap();
    }

    let get_memory_usage = create_memory_monitor(pid);

    let output_result = child.wait_with_output();

    let elapsed_time = start_time.elapsed();
    let memory_usage_option = get_memory_usage();

    verdict.duration(Some(elapsed_time));
    verdict.memory(memory_usage_option);

    match output_result {
        Ok(output) => {
            let actual_output = String::from_utf8_lossy(&output.stdout);
            if compare_lines_ignoring_line_endings(&actual_output, ans) {
                verdict.status(JudgeStatus::AC)
            } else {
                if !output.stderr.is_empty() {
                    verdict.status(JudgeStatus::RE(
                        String::from_utf8_lossy(&output.stderr).into(),
                    ))
                } else {
                    verdict.status(JudgeStatus::WA(actual_output.to_string()));
                }
            }
        }
        Err(e) => verdict.status(JudgeStatus::RE(e.to_string())),
    };

    if verdict.is_accept() {
        if let Some(max_time) = limit.max_time {
            if elapsed_time.as_millis() > max_time.as_millis() {
                verdict.status(JudgeStatus::TLE(elapsed_time));
            }
        }
        if let Some(max_memory) = limit.max_memory {
            if let Some(memory_usage) = memory_usage_option {
                if memory_usage > max_memory {
                    verdict.status(JudgeStatus::MLE(memory_usage));
                }
            }
        }
    }

    verdict
}

pub fn print_test_label(round: u32) {
    println!(
        "{}\n",
        center_text(&format!("Test {}", round), INFO_SPACE, "_")
    );
}

pub fn print_test_info(verdict: &JudgeVerdict, limit: &Limitation) {
    match &verdict.status {
        JudgeStatus::AC => println!("✅ [AC] 答案正確！"),
        JudgeStatus::RE(msg) => println!("❌ [RE] {}", msg),
        JudgeStatus::TLE(_) => println!("❌ [TLE] 程式執行時間超過限制！"),
        JudgeStatus::MLE(_) => println!("❌ [MLE] 程式記憶體使用量超過限制！"),
        JudgeStatus::WA(response) => {
            println!("❌ [WA] 答案比對失敗！");
            println!(
                "\n{}\n{}\n\n{}\n{}\n\n{}\n{}",
                center_text("Input", INFO_SPACE, "-"),
                verdict.input,
                center_text("Program Output", INFO_SPACE, "-"),
                response,
                center_text("Expect Output", INFO_SPACE, "-"),
                verdict.answer
            );
        }
    };

    if let Some(memory) = verdict.memory {
        println!();
        println!(
            "📊 記憶體使用量: {} KiB / {} KiB",
            memory,
            match limit.max_memory {
                Some(i) => i.prettify(),
                None => "無限".to_owned(),
            }
        );
    }
    if let Some(duration) = verdict.duration {
        if verdict.memory.is_none() {
            println!();
        }
        println!(
            "⏱️ 程式執行耗時: {} ms / {} ms",
            duration.as_millis(),
            match limit.max_time {
                Some(i) => i.as_millis().prettify(),
                None => "無限".to_owned(),
            }
        );
    }
}
