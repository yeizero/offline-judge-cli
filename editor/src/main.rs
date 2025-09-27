// main.rs

use crossterm::{
    cursor::{self, MoveTo},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    style::Print,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use either::Either;
use ropey::{Rope, RopeSlice};
use unicode_width::UnicodeWidthChar;

use std::cmp::min;
use std::io::{Result, Write, stdout};

// 定義一個行號佔用的固定寬度，包括分隔符
const LINE_NUMBER_WIDTH: usize = 7; // "XXXX │ " (4 digits + space + | + space)

struct Editor {
    text: Rope,
    cursor: usize,
    tmp_x: Option<usize>,
    stdout: std::io::Stdout,
    terminal_width: u16,  // 終端機寬度
    terminal_height: u16, // 終端機高度
    scroll_offset: usize, // 垂直捲動偏移，表示編輯器顯示的第一個邏輯行索引
}

impl Editor {
    fn new() -> Self {
        let (cols, rows) = terminal::size().unwrap_or((80, 24)); // 獲取終端機大小
        Editor {
            text: Rope::new(),
            cursor: 0,
            tmp_x: None,
            stdout: stdout(),
            terminal_width: cols,
            terminal_height: rows,
            scroll_offset: 0,
        }
    }

    fn run(&mut self) -> Result<()> {
        execute!(self.stdout, EnterAlternateScreen)?;
        terminal::enable_raw_mode()?;

        self.draw_editor()?;

        loop {
            self.refresh_cursor()?;

            match event::read()? {
                Event::Key(ev) if ev.kind != KeyEventKind::Release => match ev.code {
                    KeyCode::Esc => break,
                    KeyCode::Char(ch) => {
                        self.text.insert_char(self.cursor, ch);
                        self.cursor += 1;
                        self.scroll_to_cursor();
                        self.draw_editor()?;
                    }
                    KeyCode::Enter => {
                        self.text.insert_char(self.cursor, '\n');
                        self.cursor += 1;
                        self.scroll_to_cursor();
                        self.draw_editor()?;
                    }
                    KeyCode::Backspace => {
                        if self.cursor > 0 {
                            self.text.remove((self.cursor - 1)..self.cursor);
                            self.cursor -= 1;
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        }
                    }
                    KeyCode::Up => {
                        let content_width = self.content_width();
                        let y = self.text.char_to_line(self.cursor);
                        let start_idx = self.text.line_to_char(y);
                        let x = self.cursor - start_idx;
                        if x >= content_width {
                            let inner_y = x / content_width;
                            let inner_x = x % content_width;
                            self.cursor = start_idx
                                + (inner_y - 1) * content_width
                                + self.tmp_x.unwrap_or(inner_x);
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        } else if y > 0 {
                            let prev_start_idx = self.text.line_to_char(y - 1);
                            let prev_len = self.text.line(y - 1).len_chars_without_ending();
                            // TODO: 更正目前行為(跳到視覺行)
                            self.cursor = prev_start_idx + min(self.tmp_x.unwrap_or(x), prev_len);
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        }

                        if self.tmp_x.is_none() {
                            self.tmp_x = Some(x % content_width);
                        }
                    }
                    KeyCode::Down => {
                        let content_width = self.content_width();
                        let y = self.text.char_to_line(self.cursor);
                        let start_idx = self.text.line_to_char(y);
                        let x = self.cursor - start_idx;

                        let line = self.text.line(y);
                        let visual_line_count = line.chunk_by_width_cjk(content_width).count();

                        let inner_y = x / content_width;
                        let inner_x = x % content_width;

                        if inner_y < visual_line_count - 1 {
                            self.cursor = min(
                                start_idx
                                    + (inner_y + 1) * content_width
                                    + self.tmp_x.unwrap_or(inner_x),
                                start_idx + line.len_chars_without_ending(),
                            );
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        } else if y < self.text.len_lines() - 1 {
                            let next_start_idx = self.text.line_to_char(y + 1);
                            let next_len = self.text.line(y + 1).len_chars_without_ending();
                            // TODO: 更正目前行為(跳到視覺行)
                            self.cursor = next_start_idx + min(self.tmp_x.unwrap_or(x), next_len);
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        }

                        if self.tmp_x.is_none() {
                            self.tmp_x = Some(x % content_width);
                        }
                    }
                    KeyCode::Left => {
                        self.tmp_x = None;
                        if self.cursor > 0 {
                            self.cursor -= 1;
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        }
                    }
                    KeyCode::Right => {
                        self.tmp_x = None;
                        if self.cursor < self.text.len_chars() {
                            self.cursor += 1;
                            self.scroll_to_cursor();
                            self.draw_editor()?;
                        }
                    }
                    KeyCode::Home => {
                        self.cursor = self.text.line_to_char(self.text.char_to_line(self.cursor));
                    }
                    KeyCode::End => {
                        let line_idx = self.text.char_to_line(self.cursor);
                        self.cursor =
                            self.text.line_to_char(line_idx) + self.text.line(line_idx).len_chars();
                    }
                    KeyCode::PageUp => {
                        self.cursor = 0;
                        self.scroll_to_cursor();
                        self.draw_editor()?;
                    }
                    KeyCode::PageDown => {
                        self.cursor = self.text.len_chars();
                        self.scroll_to_cursor();
                        self.draw_editor()?;
                    }
                    _ => {}
                },
                Event::Resize(cols, rows) => {
                    self.terminal_width = cols;
                    self.terminal_height = rows;
                    self.scroll_to_cursor();
                    self.draw_editor()?;
                }
                _ => {}
            }
        }

        terminal::disable_raw_mode()?;
        execute!(self.stdout, LeaveAlternateScreen)?;
        Ok(())
    }

    /// 計算從 `start_logical_line` 開始到 `end_logical_line` (不包含) 之間所有視覺行的總高度
    fn get_total_visual_height_between(
        &self,
        start_logical_line: usize,
        end_logical_line: usize,
    ) -> u16 {
        let mut total_height = 0;
        for i in start_logical_line..end_logical_line {
            total_height += self
                .text
                .line(i)
                .chunk_by_width_cjk(self.content_width())
                .count();
        }
        total_height.try_into().unwrap()
    }

    /// 計算從 `scroll_offset` 開始到指定邏輯行 `target_y` (不包含) 之間所有視覺行的總高度
    fn calculate_visual_offset_from_scroll(&self, target_y: usize) -> u16 {
        self.get_total_visual_height_between(self.scroll_offset, target_y)
    }

    fn content_width(&self) -> usize {
        self.terminal_width as usize - LINE_NUMBER_WIDTH - 1
    }

    fn get_visual_height_for_line(&self, line_idx: usize) -> u16 {
        self.text
            .line(line_idx)
            .chunk_by_width_cjk(self.content_width())
            .count()
            .try_into()
            .unwrap()
    }

    // 統一的捲動函式，現在實作「視窗化」捲動行為：
    // 確保光標在螢幕範圍內，並嘗試在文件夠長時將其置中
    fn scroll_to_cursor(&mut self) {
        let term_height = self.terminal_height;
        if term_height == 0 {
            return;
        }

        let y = self.text.char_to_line(self.cursor);

        let cursor_line_visual_height = self.get_visual_height_for_line(y);

        if cursor_line_visual_height == 0 {
            return;
        } // 或至少是 1

        // 計算光標行當前相對於 `scroll_offset` 的視覺起始 Y 座標
        let visual_y_on_screen = self.calculate_visual_offset_from_scroll(y);

        // --- 2. 處理光標超出螢幕下方 ---
        // 光標行結束的視覺 Y 座標
        let cursor_line_visual_end_y = visual_y_on_screen + cursor_line_visual_height;
        if cursor_line_visual_end_y > term_height {
            // 如果光標行完全超出螢幕下方
            // 向上調整 `scroll_offset`，直到光標行剛好在螢幕底部
            let mut new_scroll_offset = y;
            let mut current_total_height = cursor_line_visual_height;

            while new_scroll_offset > 0 && current_total_height <= term_height {
                new_scroll_offset -= 1;
                current_total_height += self.get_visual_height_for_line(new_scroll_offset);
            }
            // 確保不會因多加一行導致捲動太多
            while current_total_height > term_height && new_scroll_offset < y {
                current_total_height -= self.get_visual_height_for_line(new_scroll_offset);
                new_scroll_offset += 1;
            }
            self.scroll_offset = new_scroll_offset;
            return; // 已經處理，可以返回
        }

        // --- 3. 處理文件夠長時的置中 (當光標在可見範圍內) ---
        // 只有當光標沒有超出螢幕頂部或底部時才嘗試置中
        // 這裡的邏輯是確保光標行與螢幕中央行的距離不要過大
        // 我們想要 `visual_y_on_screen` 接近 `term_height / 2`
        // 理想情況下，`scroll_offset` 應該是什麼？

        // 計算從 `self.y` 往前看，多少行可以填滿半個螢幕
        let mut ideal_scroll_offset = y;
        let mut visual_height_needed_above = 0;
        let middle_threshold = term_height / 2; // 這是視覺行數

        while ideal_scroll_offset > 0 && visual_height_needed_above < middle_threshold {
            ideal_scroll_offset -= 1;
            visual_height_needed_above += self.get_visual_height_for_line(ideal_scroll_offset);
        }

        // 確保 `ideal_scroll_offset` 不會讓視窗超出文件開頭
        // if ideal_scroll_offset < 0 { ideal_scroll_offset = 0; }

        // 現在我們有了理想的 `scroll_offset` (讓光標行居中)
        // 但是我們還需要考慮文件是否足夠長來實現這個置中。

        // 計算如果以 `ideal_scroll_offset` 開始繪製，整個文件會佔用多少視覺行
        // let total_doc_visual_height = self.get_total_visual_height_between(0, self.lines.len());

        // 計算從 `ideal_scroll_offset` 開始到文件末尾所需的視覺高度
        let height_from_ideal_offset_to_end =
            self.get_total_visual_height_between(ideal_scroll_offset, self.text.len_lines());

        if height_from_ideal_offset_to_end <= term_height {
            // 如果從理想的 `scroll_offset` 到文件末尾都顯示了，並且總高度沒超過螢幕
            // 這意味著我們已經看到文件底部了，所以 `scroll_offset` 應該盡可能往上，讓文件底部貼著螢幕底部
            let mut final_scroll_offset_for_bottom = self.text.len_lines();
            let mut visual_height_from_bottom = 0;
            while final_scroll_offset_for_bottom > 0 && visual_height_from_bottom < term_height {
                final_scroll_offset_for_bottom -= 1;
                visual_height_from_bottom +=
                    self.get_visual_height_for_line(final_scroll_offset_for_bottom);
            }
            // 微調
            while visual_height_from_bottom > term_height
                && final_scroll_offset_for_bottom < self.text.len_lines()
            {
                visual_height_from_bottom -=
                    self.get_visual_height_for_line(final_scroll_offset_for_bottom);
                final_scroll_offset_for_bottom += 1;
            }
            self.scroll_offset = final_scroll_offset_for_bottom;
        } else {
            // 文件夠長，可以實現置中，使用之前計算的 `ideal_scroll_offset`
            self.scroll_offset = ideal_scroll_offset;
        }
    }

    fn draw_editor(&mut self) -> Result<()> {
        let term_height = self.terminal_height;
        let content_width = self.content_width();

        let mut current_screen_y = 0;

        for i in self
            .get_total_visual_height_between(0, self.scroll_offset)
            .into()..self.text.len_lines()
        {
            if current_screen_y >= term_height {
                break;
            }

            let line = self.text.line(i);

            let mut is_first = true;

            for visual_line in line.chunk_by_width_cjk(content_width) {
                queue!(
                    self.stdout,
                    MoveTo(0, current_screen_y),
                    Clear(ClearType::CurrentLine)
                )?;

                if is_first {
                    queue!(self.stdout, Print(format_args!("{:>4} │ ", i + 1)))?;
                    is_first = false;
                } else {
                    queue!(self.stdout, Print(format_args!("{:>4} │ ", " ")))?;
                }

                queue!(
                    self.stdout,
                    Print(visual_line.trim_end())
                )?;

                current_screen_y += 1;
                if current_screen_y >= term_height {
                    break;
                }
            }
        }

        for y_to_clear in current_screen_y..term_height {
            queue!(
                self.stdout,
                MoveTo(0, y_to_clear),
                Clear(ClearType::CurrentLine)
            )?;
        }

        self.stdout.flush()?;
        Ok(())
    }

    fn refresh_cursor(&mut self) -> Result<()> {
        let content_width = self.content_width();
        let line_idx = self.text.char_to_line(self.cursor);
        let current_line = self.text.line(line_idx);
        let start_idx = self.text.line_to_char(line_idx);

        let mut visual_y_offset_in_line: i32 = 0;
        let mut visual_x_offset_in_line = 0;

        for (idx, ch) in current_line.chars().enumerate() {
            if idx == self.cursor - start_idx {
                break;
            }

            let w = ch.width_cjk().unwrap_or(0);
            if visual_x_offset_in_line + w > content_width {
                visual_y_offset_in_line += 1;
                visual_x_offset_in_line = w;
            } else {
                visual_x_offset_in_line += w;
            }
        }

        let visual_lines_before_current_logical_line =
            self.calculate_visual_offset_from_scroll(line_idx);
        let screen_y = visual_lines_before_current_logical_line + visual_y_offset_in_line as u16;

        queue!(
            self.stdout,
            cursor::MoveTo(
                (visual_x_offset_in_line + LINE_NUMBER_WIDTH) as u16,
                screen_y
            )
        )?;
        self.stdout.flush()
    }
}

fn main() -> Result<()> {
    let mut editor = Editor::new();
    editor.run()?;
    println!("\nRESULT: {}", editor.text);
    Ok(())
}

pub trait RopeSliceExt<'a> {
    fn chunk_by_width_cjk(&'a self, max_width: usize) -> impl Iterator<Item = RopeSlice<'a>>;
    /// Total number of chars in the RopeSlice, excluding a trailing \n.
    ///
    /// Runs in O(log n) time.
    fn len_chars_without_ending(&'a self) -> usize;
    fn trim_end(&'a self) -> RopeTrimEnd<'a>;
}

impl<'a> RopeSliceExt<'a> for RopeSlice<'a> {
    fn chunk_by_width_cjk(&'a self, max_width: usize) -> impl Iterator<Item = RopeSlice<'a>> {
        if self.len_chars() == 0 {
            return Either::Left(std::iter::once(*self));
        }

        let mut chars = self.chars().enumerate().peekable();

        Either::Right(std::iter::from_fn(move || {
            let start_idx = chars.peek()?.0;
            let mut current_width = 0;
            let mut end_idx = start_idx;

            while let Some((idx, ch)) = chars.peek() {
                let w = ch.width_cjk().unwrap_or(1);
                if current_width + w > max_width {
                    break;
                }
                current_width += w;
                end_idx = *idx + 1; // slice 的結尾是 exclusive
                chars.next();
            }

            Some(self.slice(start_idx..end_idx))
        }))
    }
    fn len_chars_without_ending(&'a self) -> usize {
        let len = self.len_chars();
        if len != 0 && self.char(len - 1) == '\n' {
            len - 1
        } else {
            len
        }
    }
    fn trim_end(&'a self) -> RopeTrimEnd<'a> {
        RopeTrimEnd(*self)
    }
}

pub struct RopeTrimEnd<'a>(RopeSlice<'a>);

impl<'a> std::fmt::Display for RopeTrimEnd<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.chunks().peekable();
        while let Some(item) = iter.next() {
            if iter.peek().is_none() {
                write!(f, "{}", item.trim_end())?;
            } else {
                write!(f, "{item}")?;
            }
        };
        Ok(())
    }
}
