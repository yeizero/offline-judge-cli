use prettytable::{Cell, Row, Table, format::{FormatBuilder, LinePosition, LineSeparator}};

use crate::{judge::verdict::{JudgeStatus, JudgeVerdict, Limitation, SummaryInfo}, utils::{PrettyNumber, center_text}};

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
