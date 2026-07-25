use std::ops::Range;

use similar::{ChangeTag, TextDiff};

const CONTEXT_LINES: usize = 2;
const LINE_WINDOW_BYTES: usize = 4 * 1024;
const MAX_INLINE_CHANGES_PER_LINE: usize = 64;
const MAX_STYLED_SEGMENTS_PER_SIDE: usize = 128;
const SPLIT_LEN: usize = 80;

#[derive(Debug)]
pub struct WrongAnswer<'case> {
    pub output: String,
    pub answer: &'case str,
    pub style_guide: StyleGuide,
}

#[derive(Debug)]
pub struct StyleGuide {
    pub output: Vec<StyleRange>,
    pub answer: Vec<StyleRange>,
    pub(crate) output_anchor: TextAnchor,
    pub(crate) answer_anchor: TextAnchor,
}

#[derive(Debug)]
pub struct StyleRange {
    pub range: Range<usize>,
    pub(crate) background: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TextAnchor {
    pub(crate) line: usize,
    pub(crate) byte: usize,
}

#[derive(Debug)]
pub enum Comparison<'case> {
    Same,
    Diff(WrongAnswer<'case>),
}

#[derive(Debug, Clone, PartialEq)]
enum LineChange<'a> {
    Empty,
    Equal(&'a str),
    Diff {
        output: Vec<(bool, &'a str)>,
        answer: Vec<(bool, &'a str)>,
    },
}

#[derive(Clone, Copy)]
struct Difference {
    line: usize,
    output_byte_in_line: usize,
    answer_byte_in_line: usize,
}

#[derive(Clone, Copy)]
struct Line<'a> {
    text: &'a str,
    start: usize,
}

fn normalized_lines(text: &str) -> impl Iterator<Item = &str> {
    text.trim_end().lines().map(str::trim_end)
}

fn normalized_lines_with_offsets(text: &str) -> impl Iterator<Item = Line<'_>> {
    normalized_lines(text).map(move |line| Line {
        text: line,
        start: line.as_ptr() as usize - text.as_ptr() as usize,
    })
}

fn first_byte_difference(left: &str, right: &str) -> (usize, usize) {
    let common = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    let mut left_byte = common.min(left.len());
    let mut right_byte = common.min(right.len());

    while left_byte > 0 && !left.is_char_boundary(left_byte) {
        left_byte -= 1;
    }
    while right_byte > 0 && !right.is_char_boundary(right_byte) {
        right_byte -= 1;
    }

    (left_byte, right_byte)
}

fn first_normalized_difference(output: &str, answer: &str) -> Option<Difference> {
    let mut output_lines = normalized_lines(output);
    let mut answer_lines = normalized_lines(answer);
    let mut line = 0;

    loop {
        match (output_lines.next(), answer_lines.next()) {
            (None, None) => return None,
            (Some(left), Some(right)) if left == right => line += 1,
            (Some(left), Some(right)) => {
                let (output_byte, answer_byte) = first_byte_difference(left, right);
                return Some(Difference {
                    line,
                    output_byte_in_line: output_byte,
                    answer_byte_in_line: answer_byte,
                });
            }
            (Some(_), None) | (None, Some(_)) => {
                return Some(Difference {
                    line,
                    output_byte_in_line: 0,
                    answer_byte_in_line: 0,
                });
            }
        }
    }
}

fn line_window(line: Line<'_>, center: usize) -> Line<'_> {
    let mut window_start = center.saturating_sub(SPLIT_LEN);
    while window_start > 0 && !line.text.is_char_boundary(window_start) {
        window_start -= 1;
    }
    let mut window_end = if center == 0 {
        LINE_WINDOW_BYTES / 2
    } else {
        window_start.saturating_add(LINE_WINDOW_BYTES)
    }
    .min(line.text.len());
    while window_end > window_start && !line.text.is_char_boundary(window_end) {
        window_end -= 1;
    }

    Line {
        text: &line.text[window_start..window_end],
        start: line.start + window_start,
    }
}

fn bounded_context(text: &str, difference_line: usize, difference_byte: usize) -> Vec<Line<'_>> {
    let start = difference_line.saturating_sub(CONTEXT_LINES);
    normalized_lines_with_offsets(text)
        .enumerate()
        .skip(start)
        .take(CONTEXT_LINES * 2 + 1)
        .map(|(index, line)| {
            if index == difference_line {
                line_window(line, difference_byte)
            } else {
                line_window(line, 0)
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines, reason = "TODO CONSIDER")]
pub fn compare(output: String, answer: &str) -> Comparison<'_> {
    let Some(difference) = first_normalized_difference(&output, answer) else {
        return Comparison::Same;
    };

    let output_anchor = TextAnchor {
        line: difference.line,
        byte: normalized_lines_with_offsets(&output)
            .nth(difference.line)
            .map_or_else(|| output.trim_end().len(), |line| line.start)
            + difference.output_byte_in_line,
    };
    let answer_anchor = TextAnchor {
        line: difference.line,
        byte: normalized_lines_with_offsets(answer)
            .nth(difference.line)
            .map_or_else(|| answer.trim_end().len(), |line| line.start)
            + difference.answer_byte_in_line,
    };

    let output_window = bounded_context(&output, difference.line, difference.output_byte_in_line);
    let answer_window = bounded_context(answer, difference.line, difference.answer_byte_in_line);
    let output_raw_lines: Vec<&str> = output_window.iter().map(|line| line.text).collect();
    let answer_raw_lines: Vec<&str> = answer_window.iter().map(|line| line.text).collect();
    let diff = TextDiff::from_slices(&output_raw_lines, &answer_raw_lines);
    let mut lines = vec![LineChange::Empty; output_raw_lines.len().max(answer_raw_lines.len())];

    for op in diff.ops() {
        for change in diff.iter_inline_changes(op) {
            match change.tag() {
                ChangeTag::Equal => {
                    #[expect(clippy::unwrap_used, reason = "must be Some due to the Equal tag")]
                    let old_idx = change.old_index().unwrap();
                    #[expect(clippy::unwrap_used)]
                    let new_idx = change.new_index().unwrap();
                    if old_idx == new_idx {
                        lines[old_idx] = LineChange::Equal(output_raw_lines[old_idx]);
                    } else {
                        push_to_diff(
                            &mut lines[old_idx],
                            DiffTarget::Output,
                            false,
                            output_raw_lines[old_idx],
                        );
                        push_to_diff(
                            &mut lines[new_idx],
                            DiffTarget::Answer,
                            false,
                            answer_raw_lines[new_idx],
                        );
                    }
                }
                ChangeTag::Delete => {
                    #[expect(clippy::unwrap_used, reason = "must be Some due to the Delete tag")]
                    let old_idx = change.old_index().unwrap();
                    for (emph, val) in change.values() {
                        push_to_diff(&mut lines[old_idx], DiffTarget::Output, *emph, val);
                    }
                }
                ChangeTag::Insert => {
                    #[expect(clippy::unwrap_used, reason = "must be Some due to the Insert tag")]
                    let new_idx = change.new_index().unwrap();
                    for (emph, val) in change.values() {
                        push_to_diff(&mut lines[new_idx], DiffTarget::Answer, *emph, val);
                    }
                }
            }
        }
    }

    let mut output_styles = Vec::new();
    let mut answer_styles = Vec::new();

    for (line_index, line) in lines.into_iter().enumerate() {
        if let LineChange::Diff {
            mut output,
            mut answer,
        } = line
        {
            bound_inline_changes(&mut output, output_raw_lines.get(line_index).copied());
            bound_inline_changes(&mut answer, answer_raw_lines.get(line_index).copied());
            let has_emphasis = (output.len() > 1 && output.iter().any(|(emph, _)| *emph))
                || (answer.len() > 1 && answer.iter().any(|(emph, _)| *emph));

            if let Some(source_line) = output_window.get(line_index) {
                append_style_ranges(&mut output_styles, &output, *source_line, has_emphasis);
            }
            if let Some(source_line) = answer_window.get(line_index) {
                append_style_ranges(&mut answer_styles, &answer, *source_line, has_emphasis);
            }
        }
    }

    Comparison::Diff(WrongAnswer {
        output,
        answer,
        style_guide: StyleGuide {
            output: output_styles,
            answer: answer_styles,
            output_anchor,
            answer_anchor,
        },
    })
}

fn append_style_ranges(
    target: &mut Vec<StyleRange>,
    changes: &[(bool, &str)],
    source_line: Line<'_>,
    has_emphasis: bool,
) {
    let mut byte = source_line.start;

    for (emphasis, text) in changes {
        let range = byte..byte + text.len();
        byte = range.end;

        if (!has_emphasis || *emphasis) && target.len() < MAX_STYLED_SEGMENTS_PER_SIDE {
            target.push(StyleRange {
                range,
                background: text.trim().is_empty(),
            });
        }
    }
}

fn bound_inline_changes<'a>(changes: &mut Vec<(bool, &'a str)>, complete_line: Option<&'a str>) {
    if changes.len() > MAX_INLINE_CHANGES_PER_LINE
        && let Some(complete_line) = complete_line
    {
        *changes = vec![(true, complete_line)];
    }
}

#[derive(Clone, Copy)]
enum DiffTarget {
    Output,
    Answer,
}

fn push_to_diff<'a>(line: &mut LineChange<'a>, write_to: DiffTarget, emph: bool, text: &'a str) {
    if matches!(line, LineChange::Empty) {
        *line = LineChange::Diff {
            output: vec![],
            answer: vec![],
        };
    }
    if let LineChange::Diff { output, answer } = line {
        match write_to {
            DiffTarget::Output => output.push((emph, text)),
            DiffTarget::Answer => answer.push((emph, text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Comparison, MAX_STYLED_SEGMENTS_PER_SIDE, WrongAnswer, compare};

    fn wrong_answer(output: String, answer: &str) -> WrongAnswer<'_> {
        let Comparison::Diff(wrong_answer) = compare(output, answer) else {
            panic!("comparison must differ");
        };
        wrong_answer
    }

    #[test]
    fn wrong_answer_retains_complete_raw_output_and_borrowed_answer() {
        let output = format!(
            "first difference\n{}\noutput tail",
            "output\n".repeat(10_000)
        );
        let answer = format!(
            "expected first line\n{}\nanswer tail",
            "answer\n".repeat(10_000)
        );
        let answer_ptr = answer.as_ptr();

        let wrong_answer = wrong_answer(output.clone(), &answer);

        assert_eq!(wrong_answer.output, output);
        assert_eq!(wrong_answer.answer, answer);
        assert_eq!(wrong_answer.answer.as_ptr(), answer_ptr);
    }

    #[test]
    fn style_ranges_are_ansi_free_in_bounds_and_on_utf8_boundaries() {
        let output = format!("{}🦊{}", "🦀".repeat(2_000), "界".repeat(2_000));
        let answer = format!("{}🦁{}", "🦀".repeat(2_000), "界".repeat(2_000));
        let wrong_answer = wrong_answer(output, &answer);

        assert!(!wrong_answer.output.contains('\u{1b}'));
        assert!(!wrong_answer.answer.contains('\u{1b}'));
        assert!(
            wrong_answer.style_guide.output.len() <= MAX_STYLED_SEGMENTS_PER_SIDE,
            "output ranges must remain bounded"
        );
        assert!(
            wrong_answer.style_guide.answer.len() <= MAX_STYLED_SEGMENTS_PER_SIDE,
            "answer ranges must remain bounded"
        );

        for styled_range in &wrong_answer.style_guide.output {
            assert!(styled_range.range.start <= styled_range.range.end);
            assert!(styled_range.range.end <= wrong_answer.output.len());
            assert!(
                wrong_answer
                    .output
                    .is_char_boundary(styled_range.range.start)
            );
            assert!(wrong_answer.output.is_char_boundary(styled_range.range.end));
        }
        for styled_range in &wrong_answer.style_guide.answer {
            assert!(styled_range.range.start <= styled_range.range.end);
            assert!(styled_range.range.end <= wrong_answer.answer.len());
            assert!(
                wrong_answer
                    .answer
                    .is_char_boundary(styled_range.range.start)
            );
            assert!(wrong_answer.answer.is_char_boundary(styled_range.range.end));
        }
    }

    #[test]
    fn preserves_trailing_whitespace_semantics() {
        assert!(matches!(
            compare("a  \n\n".to_owned(), "a\n"),
            Comparison::Same
        ));
    }

    #[test]
    fn preserves_normalized_classification_semantics() {
        let cases = [
            ("", " \n", true),
            ("a \n b\t\n", "a\n b\n", true),
            ("a\n\nb", "a\nb", false),
            (" a", "a", false),
            ("a\r\nb\r\n", "a\nb\n", true),
        ];

        for (output, answer, expected_same) in cases {
            assert_eq!(
                matches!(compare(output.to_owned(), answer), Comparison::Same),
                expected_same,
                "output={output:?}, answer={answer:?}"
            );
        }
    }

    #[test]
    fn insertion_and_deletion_differences_produce_guidance_on_the_present_side() {
        for (output, answer) in [
            ("before\nextra\nafter", "before\nafter"),
            ("before\nafter", "before\nextra\nafter"),
        ] {
            let wrong_answer = wrong_answer(output.to_owned(), answer);
            assert!(
                !wrong_answer.style_guide.output.is_empty()
                    || !wrong_answer.style_guide.answer.is_empty()
            );
        }
    }
}
