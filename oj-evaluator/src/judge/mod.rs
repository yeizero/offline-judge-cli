use shared::RawCommand;

use crate::judge::comparison::{StyledComparison, compare_styled};
use crate::judge::verdict::{JudgeStatus, JudgeVerdict, Limitation, TleType};
use crate::monitor::{JudgeMonitor, TimingStatus, load_monitor};
use crate::utils::{PrettyNumber, center_text};

mod comparison;
pub mod verdict;

const INFO_SPACE: usize = 30;

pub async fn evaluate<'a>(
    runner: &'a RawCommand,
    input: &'a str,
    ans: &'a str,
    limit: &Limitation,
) -> JudgeVerdict<'a> {
    match evaluate_with_system_error(runner, input, ans, limit).await {
        Ok(verdict) => verdict,
        Err(e) => {
            let mut verdict = JudgeVerdict::new(input);
            verdict.status(JudgeStatus::SE(e));
            verdict
        }
    }
}

async fn evaluate_with_system_error<'a>(
    runner: &'a RawCommand,
    input: &'a str,
    ans: &'a str,
    limit: &Limitation,
) -> anyhow::Result<JudgeVerdict<'a>> {
    let ans: &str = ans.trim_end();
    let mut verdict: JudgeVerdict<'a> = JudgeVerdict::new(input);

    let mut monitor = load_monitor(runner, input, limit).await?;

    match monitor.execute().await {
        Ok(TimingStatus::InTime(output)) => {
            verdict.duration(Some(output.duration));
            verdict.memory(output.memory);

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

            if verdict.is_accept() {
                if let Some(max_time) = limit.max_time
                    && output.duration.as_millis() > max_time.as_millis()
                {
                    verdict.status(JudgeStatus::Tle(TleType::Normal(output.duration)));
                }
                if let Some(max_memory) = limit.max_memory
                    && let Some(memory_usage) = output.memory
                    && memory_usage > max_memory
                {
                    verdict.status(JudgeStatus::Mle(memory_usage));
                }
            }
        }
        Ok(TimingStatus::Aborted(duration)) => {
            verdict.duration(Some(duration));
            verdict.status(JudgeStatus::Tle(TleType::Abort(duration)));
        }
        Err(e) => verdict.status(JudgeStatus::RE(e.to_string())),
    }

    Ok(verdict)
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
        JudgeStatus::SE(err) => println!("❌ [SE] 內部錯誤：{err}"),
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
