use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

use crate::judge::comparison::{StyledComparison, compare_styled};
use crate::judge::verdict::{JudgeStatus, JudgeVerdict, Limitation};
use crate::monitor::create_memory_monitor;
use crate::utils::{PrettyNumber, center_text};

mod comparison;
pub mod verdict;

const INFO_SPACE: usize = 30;

pub fn evaluate<'a>(
    runner: &mut Command,
    input: &'a str,
    ans: &'a str,
    limit: &Limitation,
) -> JudgeVerdict<'a> {
    let ans = ans.trim_end();
    let mut verdict: JudgeVerdict<'a> = JudgeVerdict::new(input);

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
            match compare_styled(&actual_output, ans) {
                StyledComparison::Same => {
                    verdict.status(JudgeStatus::AC);
                }
                StyledComparison::Diff(diff) => {
                    if !output.stderr.is_empty() {
                        verdict.status(JudgeStatus::RE(
                            String::from_utf8_lossy(&output.stderr).into(),
                        ))
                    } else {
                        verdict.status(JudgeStatus::WA(diff));
                    }
                }
            };
        }
        Err(e) => verdict.status(JudgeStatus::RE(e.to_string())),
    };

    if verdict.is_accept() {
        if let Some(max_time) = limit.max_time
            && elapsed_time.as_millis() > max_time.as_millis()
        {
            verdict.status(JudgeStatus::Tle(elapsed_time));
        }
        if let Some(max_memory) = limit.max_memory
            && let Some(memory_usage) = memory_usage_option
            && memory_usage > max_memory
        {
            verdict.status(JudgeStatus::Mle(memory_usage));
        }
    }

    verdict
}

pub fn print_test_label(round: u32) {
    println!(
        "{}\n",
        center_text(&format!("Test {round}"), INFO_SPACE, "_")
    );
}

pub fn print_test_info(verdict: &JudgeVerdict, limit: &Limitation) {
    match &verdict.status {
        JudgeStatus::AC => println!("✅ [AC] 答案正確！"),
        JudgeStatus::RE(msg) => println!("❌ [RE] {msg}"),
        JudgeStatus::Tle(_) => println!("❌ [TLE] 程式執行時間超過限制！"),
        JudgeStatus::Mle(_) => println!("❌ [MLE] 程式記憶體使用量超過限制！"),
        JudgeStatus::WA(diff) => {
            println!("❌ [WA] 答案比對失敗！");
            println!(
                "\n{}\n{}\n\n{}\n{}\n{}\n{}\n",
                center_text("Input", INFO_SPACE, "-"),
                verdict.input,
                center_text("Program Output", INFO_SPACE, "-"),
                diff.output,
                center_text("Expect Output", INFO_SPACE, "-"),
                diff.answer
            );
        }
    };

    if let Some(memory) = verdict.memory {
        println!();
        println!(
            "📊 記憶體使用量: {} KiB / {} KiB",
            memory,
            limit
                .max_memory
                .map_or_else(|| "無限制".to_string(), |i| i.prettify())
        );
    }
    if let Some(duration) = verdict.duration {
        if verdict.memory.is_none() {
            println!();
        }
        println!(
            "⏱️ 程式執行耗時: {} ms / {} ms",
            duration.as_millis(),
            limit
                .max_time
                .map_or_else(|| "無限制".to_string(), |i| i.as_millis().prettify())
        );
    }
}
