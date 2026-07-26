use std::ops::Range;

use owo_colors::OwoColorize;
use prettytable::{
    Cell, Row, Table,
    format::{FormatBuilder, LinePosition, LineSeparator},
};
use shared::tr;

use crate::{
    judge::comparison::{StyleRange, TextAnchor, WrongAnswer},
    judge::verdict::{JudgeStatus, JudgeVerdict, Limitation, SummaryInfo},
    utils::{PrettyNumber, center_text},
};

const INFO_SPACE: usize = 30;
const CONTEXT_LINES: usize = 2;
const LINE_WINDOW_BYTES: usize = 4 * 1024;
const SPLIT_LEN: usize = 80;
const MAX_DISPLAY_SEGMENTS: usize = 7;
const MAX_STYLED_SEGMENTS_PER_SIDE: usize = 128;
const MAX_STYLED_BYTES_PER_SIDE: usize = 16 * 1024;

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
            Cell::new(tr!(JudgeReportTestCaseHeader)),
            Cell::new(tr!(JudgeReportTimeHeader)),
            Cell::new(tr!(JudgeReportMemoryHeader)),
            Cell::new(tr!(JudgeReportResultHeader)),
        ]));

        Self {
            summary_info: SummaryInfo::default(),
            report_table,
        }
    }

    pub fn update(&mut self, verdict: &JudgeVerdict<'_>, round: usize) {
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
            "\n📝 {} {:>33}",
            tr!(JudgeSummaryLabel),
            format!(
                "{}",
                tr!(JudgeSummary {
                    correct: self.summary_info.success_rounds,
                    incorrect: self.summary_info.current_rounds - self.summary_info.success_rounds,
                    ratio: self.summary_info.score()
                })
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

#[derive(Debug)]
struct FormattedWrongAnswer {
    output: String,
    answer: String,
}

#[derive(Clone, Copy)]
enum SideColor {
    Output,
    Answer,
}

#[derive(Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
}

#[derive(Clone)]
struct Segment {
    range: Range<usize>,
    newline: bool,
    prefix_omitted: bool,
    suffix_omitted: bool,
}

#[derive(Clone, Copy)]
struct LineWindow<'a> {
    line: Line<'a>,
    prefix_omitted: bool,
    suffix_omitted: bool,
}

fn normalized_lines_with_offsets(text: &str) -> impl Iterator<Item = Line<'_>> {
    text.trim_end().lines().map(move |line| {
        let line = line.trim_end();
        Line {
            text: line,
            start: line.as_ptr() as usize - text.as_ptr() as usize,
        }
    })
}

fn format_wrong_answer(wrong_answer: &WrongAnswer<'_>) -> FormattedWrongAnswer {
    FormattedWrongAnswer {
        output: format_side(
            &wrong_answer.output,
            wrong_answer.style_guide.output_anchor,
            &wrong_answer.style_guide.output,
            SideColor::Output,
        ),
        answer: format_side(
            wrong_answer.answer,
            wrong_answer.style_guide.answer_anchor,
            &wrong_answer.style_guide.answer,
            SideColor::Answer,
        ),
    }
}

fn format_side(text: &str, anchor: TextAnchor, styles: &[StyleRange], color: SideColor) -> String {
    let normalized_end = text.trim_end().len();
    let context_start_line = anchor.line.saturating_sub(CONTEXT_LINES);
    let context: Vec<Line<'_>> = normalized_lines_with_offsets(text)
        .skip(context_start_line)
        .take(CONTEXT_LINES * 2 + 1)
        .collect();
    let mut segments = Vec::new();
    let mut anchor_segment = None;

    for (relative_line, line) in context.iter().enumerate() {
        let line_index = context_start_line + relative_line;
        let center = if line_index == anchor.line {
            anchor.byte.saturating_sub(line.start).min(line.text.len())
        } else {
            0
        };
        let window = line_window(*line, center);
        let first_segment = segments.len();
        wrap_line(window, &mut segments);

        if line_index == anchor.line {
            anchor_segment = segments[first_segment..]
                .iter()
                .position(|segment| {
                    segment.range.start <= anchor.byte
                        && (anchor.byte < segment.range.end
                            || (segment.newline && anchor.byte == segment.range.end))
                })
                .map(|index| first_segment + index)
                .or(Some(first_segment));
        }
    }

    if segments.is_empty() {
        segments.push(Segment {
            range: anchor.byte..anchor.byte,
            newline: true,
            prefix_omitted: anchor.byte > 0,
            suffix_omitted: anchor.byte < normalized_end,
        });
    }

    let anchor_segment = anchor_segment.unwrap_or(segments.len());
    let display_start = anchor_segment
        .saturating_sub(2)
        .min(segments.len().saturating_sub(1));
    let display_end = (display_start + MAX_DISPLAY_SEGMENTS).min(segments.len());
    let selected = &segments[display_start..display_end];
    let has_above_omission = selected
        .first()
        .is_some_and(|segment| segment.range.start > 0);
    let has_below_omission = selected
        .last()
        .is_some_and(|segment| segment.range.end < normalized_end);

    let mut result = String::new();
    if has_above_omission {
        result.push_str(tr!(DiffAboveOmitted));
        result.push('\n');
    }

    for segment in selected.iter().take(MAX_STYLED_SEGMENTS_PER_SIDE) {
        append_styled_segment(&mut result, text, segment, styles, color);
        if result.len() >= MAX_STYLED_BYTES_PER_SIDE {
            result.truncate(previous_char_boundary(&result, MAX_STYLED_BYTES_PER_SIDE));
            break;
        }
    }

    let below_omission = tr!(DiffBelowOmitted);
    if has_below_omission && result.len() + below_omission.len() <= MAX_STYLED_BYTES_PER_SIDE {
        result.push_str(below_omission);
    }

    result
}

fn line_window(line: Line<'_>, center: usize) -> LineWindow<'_> {
    let mut start = center.saturating_sub(SPLIT_LEN);
    while start > 0 && !line.text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = start.saturating_add(LINE_WINDOW_BYTES).min(line.text.len());
    while end > start && !line.text.is_char_boundary(end) {
        end -= 1;
    }
    LineWindow {
        line: Line {
            text: &line.text[start..end],
            start: line.start + start,
        },
        prefix_omitted: start > 0,
        suffix_omitted: end < line.text.len(),
    }
}

fn wrap_line(window: LineWindow<'_>, segments: &mut Vec<Segment>) {
    let line = window.line;
    if line.text.is_empty() {
        segments.push(Segment {
            range: line.start..line.start,
            newline: true,
            prefix_omitted: window.prefix_omitted,
            suffix_omitted: window.suffix_omitted,
        });
        return;
    }

    let line_end = line.start + line.text.len();
    let mut byte = line.start;
    while byte < line_end {
        let relative_byte = byte - line.start;
        let remaining = &line.text[relative_byte..];
        let relative_end = wrapped_end(remaining);
        let end = byte + relative_end;
        segments.push(Segment {
            range: byte..end,
            newline: end == line_end,
            prefix_omitted: byte == line.start && window.prefix_omitted,
            suffix_omitted: end == line_end && window.suffix_omitted,
        });
        byte = end;
    }
}

fn wrapped_end(text: &str) -> usize {
    if text.len() <= SPLIT_LEN {
        return text.len();
    }

    let mut end = SPLIT_LEN;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let mut search_start = SPLIT_LEN.saturating_sub(10).min(end);
    while search_start < end && !text.is_char_boundary(search_start) {
        search_start += 1;
    }
    if let Some(space) = text[search_start..end].rfind(' ') {
        let candidate = search_start + space;
        if candidate > 0 {
            return candidate;
        }
    }
    end
}

fn append_styled_segment(
    result: &mut String,
    text: &str,
    segment: &Segment,
    styles: &[StyleRange],
    color: SideColor,
) {
    if segment.prefix_omitted {
        result.push_str("... (line prefix omitted) ");
    }

    let mut cursor = segment.range.start;
    let mut styled_empty_line = false;
    for style in styles {
        if style.range.end < segment.range.start || style.range.start > segment.range.end {
            continue;
        }
        let start = style.range.start.max(segment.range.start);
        let end = style.range.end.min(segment.range.end);
        if start > cursor {
            result.push_str(&text[cursor..start]);
        }
        if start == end {
            if segment.range.is_empty() && style.background {
                append_colored(result, "\n", true, color);
                styled_empty_line = true;
            }
        } else {
            append_colored(result, &text[start..end], style.background, color);
        }
        cursor = cursor.max(end);
    }
    if cursor < segment.range.end {
        result.push_str(&text[cursor..segment.range.end]);
    }

    if segment.suffix_omitted {
        result.push_str(" ... (line suffix omitted)");
    }
    if segment.newline && !styled_empty_line {
        result.push('\n');
    }
}

fn append_colored(result: &mut String, text: &str, background: bool, color: SideColor) {
    let styled = match (color, background) {
        (SideColor::Output, true) => text.on_red().to_string(),
        (SideColor::Output, false) => text.red().to_string(),
        (SideColor::Answer, true) => text.on_green().to_string(),
        (SideColor::Answer, false) => text.green().to_string(),
    };
    result.push_str(&styled);
}

fn previous_char_boundary(text: &str, mut byte: usize) -> usize {
    byte = byte.min(text.len());
    while byte > 0 && !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

pub fn print_test_info(verdict: &JudgeVerdict, limit: &Limitation) {
    match &verdict.status {
        JudgeStatus::AC => println!("✅ [AC] {}", tr!(VerdictACInfo)),
        JudgeStatus::RE(msg) => println!("❌ [RE] {msg}"),
        JudgeStatus::SE(err) => {
            println!("❌ [SE] {}", tr!(VerdictSEInfo { error: err }));
        }
        JudgeStatus::Ole(_) => println!("❌ [OLE] {}", tr!(VerdictOLEInfo)),
        JudgeStatus::Tle(_) => println!("❌ [TLE] {}", tr!(VerdictTLEInfo)),
        JudgeStatus::Mle(_) => println!("❌ [MLE] {}", tr!(VerdictMLEInfo)),
        JudgeStatus::WA(diff) => {
            let display_input = if verdict.input.len() > 250 || verdict.input.lines().count() > 10 {
                tr!(HiddenLongInput)
            } else {
                verdict.input
            };
            let formatted = format_wrong_answer(diff);

            println!("❌ [WA] {}", tr!(VerdictWAInfo));
            println!(
                "\n{}\n{}\n\n{}\n{}\n{}\n{}\n",
                center_text("Input", INFO_SPACE, "-"),
                display_input,
                center_text("Program Output", INFO_SPACE, "-"),
                formatted.output,
                center_text("Expect Output", INFO_SPACE, "-"),
                formatted.answer
            );
        }
    }

    if let Some(memory) = verdict.memory {
        println!();
        println!(
            "📊 {}",
            tr!(MemoryUsage {
                used: memory.prettify(),
                limit: limit
                    .max_memory
                    .map_or_else(|| tr!(Unlimited).to_string(), |i| i.prettify())
            })
        );
    }
    if let Some(duration) = verdict.duration {
        if verdict.memory.is_none() {
            println!();
        }
        println!(
            "⏱️ {}",
            tr!(ExecutionTime {
                used: duration.as_millis().prettify(),
                limit: limit
                    .max_time
                    .map_or_else(|| tr!(Unlimited).to_string(), |i| i.as_millis().prettify())
            })
        );
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::{MAX_STYLED_BYTES_PER_SIDE, format_wrong_answer};
    use crate::judge::comparison::{Comparison, WrongAnswer, compare};

    fn wrong_answer(output: String, answer: &str) -> WrongAnswer<'_> {
        let Comparison::Diff(wrong_answer) = compare(output, answer) else {
            panic!("comparison must differ");
        };
        wrong_answer
    }

    fn alternating_tokens(changed: &str) -> String {
        format!("same {changed} ").repeat(1_024)
    }

    #[test]
    fn reproduction_with_many_output_lines_shows_output_below_omission() {
        let mut output = String::new();
        for number in 0..100_000 {
            let _ = writeln!(output, "{number}");
        }
        let wrong_answer = wrong_answer(output, "0\n");

        let rendered = format_wrong_answer(&wrong_answer);
        let below_omission = shared::tr!(DiffBelowOmitted);

        assert!(wrong_answer.output.starts_with("0\n1\n2\n"));
        assert!(wrong_answer.output.ends_with("99999\n"));
        assert_eq!(wrong_answer.answer, "0\n");
        assert!(rendered.output.contains(below_omission));
        assert!(!rendered.answer.contains(below_omission));
    }

    #[test]
    fn difference_after_large_common_prefix_shows_above_omission() {
        let prefix = "same\n".repeat(100_000);
        let output = format!("{prefix}wrong\n");
        let answer = format!("{prefix}right\n");
        let wrong_answer = wrong_answer(output, &answer);

        let rendered = format_wrong_answer(&wrong_answer);
        let above_omission = shared::tr!(DiffAboveOmitted);

        assert!(rendered.output.contains(above_omission));
        assert!(rendered.answer.contains(above_omission));
        assert!(rendered.output.contains("wrong"));
        assert!(rendered.answer.contains("right"));
    }

    #[test]
    fn output_and_answer_omissions_are_independent() {
        let output = format!("wrong\n{}", "output tail\n".repeat(100));
        let answer = format!("right\n{}", "answer tail\n".repeat(2));
        let wrong_answer = wrong_answer(output, &answer);

        let rendered = format_wrong_answer(&wrong_answer);
        let below_omission = shared::tr!(DiffBelowOmitted);

        assert!(rendered.output.contains(below_omission));
        assert!(!rendered.answer.contains(below_omission));
    }

    #[test]
    fn huge_ascii_and_utf8_single_line_mismatches_remain_visible() {
        for (output, answer, output_mismatch, answer_mismatch) in [
            (
                format!("{}X{}", "a".repeat(1_000_000), "b".repeat(1_000_000)),
                format!("{}Y{}", "a".repeat(1_000_000), "b".repeat(1_000_000)),
                "X",
                "Y",
            ),
            (
                format!("{}🦊{}", "🦀".repeat(2_000), "界".repeat(2_000)),
                format!("{}🦁{}", "🦀".repeat(2_000), "界".repeat(2_000)),
                "🦊",
                "🦁",
            ),
        ] {
            let wrong_answer = wrong_answer(output, &answer);
            let rendered = format_wrong_answer(&wrong_answer);
            assert!(rendered.output.contains(output_mismatch));
            assert!(rendered.answer.contains(answer_mismatch));
        }
    }

    #[test]
    fn alternating_inline_differences_stay_within_rendering_bounds() {
        let output = alternating_tokens("output-mismatch");
        let answer = alternating_tokens("answer-mismatch");
        let wrong_answer = wrong_answer(output, &answer);

        let rendered = format_wrong_answer(&wrong_answer);

        assert!(rendered.output.contains("output-mismatch"));
        assert!(rendered.answer.contains("answer-mismatch"));
        assert!(rendered.output.len() <= MAX_STYLED_BYTES_PER_SIDE);
        assert!(rendered.answer.len() <= MAX_STYLED_BYTES_PER_SIDE);
    }

    #[test]
    fn rendering_does_not_mutate_or_add_ansi_to_stored_raw_strings() {
        let output = "raw output mismatch\nraw output tail".to_owned();
        let answer = "raw answer mismatch\nraw answer tail";
        let wrong_answer = wrong_answer(output.clone(), answer);

        let rendered = format_wrong_answer(&wrong_answer);

        assert!(rendered.output.contains('\u{1b}'));
        assert!(rendered.answer.contains('\u{1b}'));
        assert_eq!(wrong_answer.output, output);
        assert_eq!(wrong_answer.answer, answer);
        assert!(!wrong_answer.output.contains('\u{1b}'));
        assert!(!wrong_answer.answer.contains('\u{1b}'));
    }
}
