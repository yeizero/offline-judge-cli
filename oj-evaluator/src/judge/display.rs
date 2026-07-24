use prettytable::{
    Cell, Row, Table,
    format::{FormatBuilder, LinePosition, LineSeparator},
};

use crate::{
    judge::verdict::{JudgeStatus, JudgeVerdict, Limitation, SummaryInfo},
    utils::{PrettyNumber, center_text},
};

const INFO_SPACE: usize = 30;

pub struct JudgeReport {
    summary_info: SummaryInfo,
    report_table: Table,
}

impl JudgeReport {
    pub fn new() -> Self {
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

        Self {
            summary_info: SummaryInfo::default(),
            report_table,
        }
    }

    pub fn update(&mut self, verdict: JudgeVerdict, round: usize) {
        self.report_table.add_row(Row::new(vec![
            Cell::new(if verdict.is_accept() { "✅" } else { "❌" }),
            Cell::new(&round.to_string()),
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

        self.summary_info.update(verdict);
    }

    pub fn printstd(&self) {
        println!(
            "\n📝 總結: {:>33}",
            format!(
                "正確 {} 錯誤 {} 正確比 {}%",
                self.summary_info.success_rounds,
                self.summary_info.current_rounds - self.summary_info.success_rounds,
                self.summary_info.score()
            )
        );

        self.report_table.printstd();
        println!("🎯 {}", self.summary_info);
    }
}

pub fn print_test_label(round: usize) {
    println!(
        "{}\n",
        center_text(&format!("Test {round}"), INFO_SPACE, "_")
    );
}

fn truncate_output(segments: &[String], start_wrapped_idx: usize) -> String {
    const MAX_LINE: usize = 70000;
    if segments.len() <= MAX_LINE {
        return segments.join("");
    }

    let display_start = start_wrapped_idx.saturating_sub(2);
    let display_end = (display_start + MAX_LINE).min(segments.len());

    let mut result = String::new();
    if display_start > 0 {
        result.push_str("... (以上省略)\n");
    }

    result.push_str(&segments[display_start..display_end].join(""));

    if display_end < segments.len() {
        result.push_str("... (以下省略)");
    }

    result
}

pub fn print_test_info(verdict: &JudgeVerdict, limit: &Limitation) {
    match &verdict.status {
        JudgeStatus::AC => println!("✅ [AC] 答案正確！"),
        JudgeStatus::RE(msg) => println!("❌ [RE] {msg}"),
        JudgeStatus::SE(err) => println!("❌ [SE] 內部錯誤：{err}"),
        JudgeStatus::Tle(_) => println!("❌ [TLE] 程式執行時間超過限制！"),
        JudgeStatus::Mle(_) => println!("❌ [MLE] 程式記憶體使用量超過限制！"),
        JudgeStatus::WA(diff) => {
            let display_input = if verdict.input.len() > 250 || verdict.input.lines().count() > 10 {
                "(已隱藏過長內容)"
            } else {
                verdict.input
            };

            println!("❌ [WA] 答案比對失敗！");
            println!(
                "\n{}\n{}\n\n{}\n{}\n{}\n{}\n",
                center_text("Input", INFO_SPACE, "-"),
                display_input,
                center_text("Program Output", INFO_SPACE, "-"),
                truncate_output(&diff.output, diff.first_diff_segment_index),
                center_text("Expect Output", INFO_SPACE, "-"),
                truncate_output(&diff.answer, diff.first_diff_segment_index)
            );
        }
    }

    if let Some(memory) = verdict.memory {
        println!();
        println!(
            "📊 記憶體使用量: {} KiB / {} KiB",
            memory.prettify(),
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
            duration.as_millis().prettify(),
            limit
                .max_time
                .map_or_else(|| "無限制".to_string(), |i| i.as_millis().prettify())
        );
    }
}
