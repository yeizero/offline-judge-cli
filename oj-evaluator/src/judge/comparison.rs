use owo_colors::{OwoColorize, Style};
use similar::{ChangeTag, TextDiff};
use std::borrow::Cow;

struct TextChange<'a> {
    emphasized: bool,
    value: Cow<'a, str>,
}

fn to_unstyled_string(lines: &[TextChange]) -> String {
    lines
        .iter()
        .map(|change| change.value.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

#[derive(Debug)]
pub struct StyledDiff {
    pub output: String,
    pub answer: String,
}

#[derive(Debug)]
pub enum StyledComparison {
    Same,
    Diff(StyledDiff)
}

pub fn compare_styled(output: &str, answer: &str) -> StyledComparison {
    let output_lines: Vec<&str> = output.trim_end().lines().map(str::trim_end).collect();
    let answer_lines: Vec<&str> = answer.trim_end().lines().map(str::trim_end).collect();

    if output_lines == answer_lines {
        return StyledComparison::Same;
    }

    let diff = TextDiff::from_slices(&output_lines, &answer_lines);

    let mut output = String::with_capacity(output.len());
    let mut answer = String::with_capacity(answer.len());

    for op in diff.ops() {
        for change in diff.iter_inline_changes(op) {
            let changes: Vec<TextChange> = change
                .iter_strings_lossy()
                .map(|(emphasized, value)| TextChange { emphasized, value })
                .collect();

            if change.tag() == ChangeTag::Equal {
                let mut unstyled = to_unstyled_string(&changes);
                

                if change.missing_newline() {
                    unstyled.push('\n');
                }

                output.push_str(&unstyled);
                answer.push_str(&unstyled);
                continue;
            }

            let (target, style) = match change.tag() {
                ChangeTag::Insert => (&mut answer, Style::new().green()),
                ChangeTag::Delete => (&mut output, Style::new().red()),
                ChangeTag::Equal => unreachable!(),
            };

            let is_line_fully_changed = changes.iter().all(|change| !change.emphasized);

            if is_line_fully_changed {
                let mut unstyled = to_unstyled_string(&changes);

                if change.missing_newline() {
                    unstyled.push('\n');
                }

                target.push_str(&unstyled.style(style).to_string());

                continue;
            }

            for (emphasized, value) in change.iter_strings_lossy() {
                if emphasized {
                    target.push_str(&value.style(style).to_string());
                } else {
                    target.push_str(&value);
                }
            }

            if change.missing_newline() {
                target.push('\n');
            }
        }
    }

    output.truncate(output.trim_end().len());
    answer.truncate(answer.trim_end().len());

    StyledComparison::Diff(StyledDiff { output, answer })
}
