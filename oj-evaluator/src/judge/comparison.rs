use owo_colors::OwoColorize;
use similar::{ChangeTag, TextDiff};

#[derive(Debug)]
pub struct StyledDiff {
    pub output: Vec<String>,
    pub answer: Vec<String>,
    pub first_diff_segment_index: usize,
}

#[derive(Debug)]
pub enum StyledComparison {
    Same,
    Diff(StyledDiff),
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

fn process_segments(
    text: &str,
    is_last_segment: bool,
    style_fn: impl Fn(String) -> String,
) -> Vec<String> {
    const SPLIT_LEN: usize = 80;
    
    let mut result = Vec::with_capacity((text.len() / SPLIT_LEN) + 1);

    if text.is_empty() {
        if is_last_segment {
            result.push("\n".to_string());
        }
        return result;
    }

    let mut current_pos = 0;
    while current_pos < text.len() {
        let mut target_end = (current_pos + SPLIT_LEN).min(text.len());
        if target_end < text.len() {
            let search_start = current_pos + SPLIT_LEN.saturating_sub(10);
            let search_range = &text[search_start..target_end];
            
            if let Some(pos) = search_range.rfind(' ') {
                let candidate_end = search_start + pos;
                if candidate_end > current_pos {
                    target_end = candidate_end;
                }
            }
        }

        while !text.is_char_boundary(target_end) {
            target_end -= 1;
        }

        if target_end <= current_pos {
            target_end = current_pos + 1;
            while target_end < text.len() && !text.is_char_boundary(target_end) {
                target_end += 1;
            }
        }

        let segment = &text[current_pos..target_end];
        let mut s = style_fn(segment.to_string());

        current_pos = target_end;

        if is_last_segment && current_pos == text.len() {
            s.push('\n');
        }

        result.push(s);
    }

    result
}

#[allow(clippy::too_many_lines, reason = "TODO CONSIDER")]
pub fn compare_styled(output: &str, answer: &str) -> StyledComparison {
    let output_raw_lines: Vec<&str> = output.trim_end().lines().map(str::trim_end).collect();
    let answer_raw_lines: Vec<&str> = answer.trim_end().lines().map(str::trim_end).collect();

    if output_raw_lines == answer_raw_lines {
        return StyledComparison::Same;
    }

    let diff = TextDiff::from_slices(&output_raw_lines, &answer_raw_lines);
    let mut lines = vec![LineChange::Empty; output_raw_lines.len().max(answer_raw_lines.len())];

    for op in diff.ops() {
        for change in diff.iter_inline_changes(op) {
            match change.tag() {
                ChangeTag::Equal => {
                    #[expect(clippy::unwrap_used, reason="must be Some due to the Equal tag")]
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
                    #[expect(clippy::unwrap_used, reason="must be Some due to the Delete tag")]
                    let old_idx = change.old_index().unwrap();
                    for (emph, val) in change.values() {
                        push_to_diff(&mut lines[old_idx], DiffTarget::Output, *emph, val);
                    }
                }
                ChangeTag::Insert => {
                    #[expect(clippy::unwrap_used, reason="must be Some due to the Insert tag")]
                    let new_idx = change.new_index().unwrap();
                    for (emph, val) in change.values() {
                        push_to_diff(&mut lines[new_idx], DiffTarget::Answer, *emph, val);
                    }
                }
            }
        }
    }

    let mut final_output = Vec::with_capacity(lines.len());
    let mut final_answer = Vec::with_capacity(lines.len());
    let mut first_diff_idx = 0;
    let mut met_diff = false;

    for line in lines {
        match line {
            LineChange::Empty => {}
            LineChange::Equal(text) => {
                let segments = process_segments(text, true, |s| s);
                if !met_diff {
                    first_diff_idx += segments.len();
                }
                for s in segments {
                    final_answer.push(s.clone());
                    final_output.push(s);
                }
            }
            LineChange::Diff { output, answer } => {
                met_diff = true;
                let has_emph = (output.len() > 1 && output.iter().any(|(e, _)| *e))
                    || (answer.len() > 1 && answer.iter().any(|(e, _)| *e));

                for (i, (emph, text)) in output.iter().enumerate() {
                    let is_last = i == output.len() - 1;
                    final_output.extend(process_segments(text, is_last, |s| {
                        if *emph || !has_emph {
                            if s.trim().is_empty() {
                                s.on_red().to_string()
                            } else {
                                s.red().to_string()
                            }
                        } else {
                            s
                        }
                    }));
                }

                for (i, (emph, text)) in answer.iter().enumerate() {
                    let is_last = i == answer.len() - 1;
                    final_answer.extend(process_segments(text, is_last, |s| {
                        if *emph || !has_emph {
                            if s.trim().is_empty() {
                                s.on_green().to_string()
                            } else {
                                s.green().to_string()
                            }
                        } else {
                            s
                        }
                    }));
                }
            }
        }
    }

    StyledComparison::Diff(StyledDiff {
        output: final_output,
        answer: final_answer,
        first_diff_segment_index: first_diff_idx,
    })
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
