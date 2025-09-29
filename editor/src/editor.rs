use anyhow::Result;
use arboard::Clipboard;
use crossterm::{
    cursor::{self, MoveTo},
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use either::Either;
use ropey::{Rope, RopeSlice, iter::Chars};
use std::collections::{HashMap, HashSet};
use std::io::{Write, stdout};
use std::time::Duration;
use std::{
    cmp::{max, min},
    mem,
};
use unicode_width::UnicodeWidthChar;

use crate::command::{Command, InputEvent, default_keymap};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ScrollOffset {
    /// 頂部第一個可見的邏輯行索引
    logical_line: usize,
    /// 在該邏輯行內的視覺行偏移
    visual_offset_in_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandEffect {
    /// Will do visual update and cancel selection
    ///
    /// (start, old_count, new_count)
    TextChanged(usize, usize, usize),
    /// Will do visual update about cursor
    CursorDirty,
    /// Will not run handle_selection()
    SelectionFixed,
    /// Will not do any visual update
    None,
}

pub struct Editor {
    text: Rope,
    cursor: usize,
    selection_anchor: Option<usize>,
    tmp_x: Option<usize>,
    stdout: std::io::Stdout,
    terminal_width: u16,
    terminal_height: u16,
    scroll_offset: ScrollOffset,
    cumulative_visual_heights: Vec<u32>,
    is_dirty: bool,
    full_redraw_request: bool,
    dirty_lines: HashSet<usize>,
    should_quit: bool,
    pub keymap: HashMap<InputEvent, Command>,
}

impl Editor {
    const LINE_NUMBER_WIDTH: usize = 7; // "XXXX │ " (4 digits + space + | + space)
    const STATUS_BAR_HEIGHT: u16 = 1;

    /// create with empty rope
    pub fn new() -> Self {
        Self::from_rope(Rope::new())
    }

    pub fn from_rope(rope: Rope) -> Self {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        Self {
            text: rope,
            cursor: 0,
            selection_anchor: None,
            tmp_x: None,
            stdout: stdout(),
            terminal_width: cols,
            terminal_height: rows,
            scroll_offset: ScrollOffset::default(),
            cumulative_visual_heights: vec![0],
            is_dirty: true,
            full_redraw_request: true,
            dirty_lines: HashSet::new(),
            should_quit: false,
            keymap: default_keymap(),
        }
    }

    fn rebuild_height_cache(&mut self) {
        self.cumulative_visual_heights.clear();
        self.cumulative_visual_heights.push(0);
        if self.text.len_lines() == 0 {
            return;
        }
        let mut total_height: u32 = 0;
        for i in 0..self.text.len_lines() {
            let line_height = max(
                1,
                self.text
                    .line(i)
                    .chunk_by_width_cjk(self.content_width())
                    .count(),
            ) as u32;
            total_height += line_height;
            self.cumulative_visual_heights.push(total_height);
        }
    }

    fn update_height_cache(
        &mut self,
        start_line: usize,
        old_line_count: usize,
        new_line_count: usize,
    ) -> i64 {
        if self.text.len_lines() == 0 {
            self.cumulative_visual_heights = vec![0];
            return 0;
        }

        let old_span_height = self.cumulative_visual_heights[start_line + old_line_count]
            - self.cumulative_visual_heights[start_line];

        let mut new_heights = Vec::with_capacity(new_line_count);
        let mut new_span_height: u32 = 0;
        for i in 0..new_line_count {
            let line_idx = start_line + i;
            let h = max(
                1,
                self.text
                    .line(line_idx)
                    .chunk_by_width_cjk(self.content_width())
                    .count(),
            ) as u32;
            new_heights.push(h);
            new_span_height += h;
        }

        let delta = new_span_height as i64 - old_span_height as i64;

        let mut new_cumulative_heights = Vec::with_capacity(new_line_count);
        let mut current_cumulative = self.cumulative_visual_heights[start_line];
        for h in new_heights {
            current_cumulative += h;
            new_cumulative_heights.push(current_cumulative);
        }

        self.cumulative_visual_heights.splice(
            start_line + 1..start_line + 1 + old_line_count,
            new_cumulative_heights,
        );

        if delta != 0 {
            for i in (start_line + 1 + new_line_count)..self.cumulative_visual_heights.len() {
                self.cumulative_visual_heights[i] =
                    (self.cumulative_visual_heights[i] as i64 + delta) as u32;
            }
        }

        // NEW: 回傳計算出的高度變化量
        delta
    }

    fn get_selection_range(&self) -> Option<(usize, usize)> {
        self.selection_anchor.map(|anchor| {
            if self.cursor < anchor {
                (self.cursor, anchor)
            } else {
                (anchor, self.cursor)
            }
        })
    }

    fn char_idx_to_visual_pos_in_line(
        &self,
        line_idx: usize,
        char_offset: usize,
    ) -> (usize, usize) {
        let line = self.text.line(line_idx);
        let content_width = self.content_width();
        let mut visual_x = 0;
        let mut visual_y = 0;
        for (i, ch) in line.chars().enumerate() {
            if i >= char_offset {
                break;
            }
            let w = ch.width_cjk().unwrap_or(1);
            if visual_x + w > content_width {
                visual_y += 1;
                visual_x = w;
            } else {
                visual_x += w;
            }
        }
        (visual_x, visual_y)
    }

    fn visual_pos_to_char_idx_in_line(
        &self,
        line_idx: usize,
        target_vx: usize,
        target_vy: usize,
    ) -> usize {
        let line = self.text.line(line_idx);
        let content_width = self.content_width();
        let mut current_vx = 0;
        let mut current_vy = 0;
        let mut last_char_idx = 0;
        for (i, ch) in line.chars().enumerate() {
            if current_vy > target_vy {
                return last_char_idx;
            }
            last_char_idx = i;
            let w = ch.width_cjk().unwrap_or(1);
            if current_vy == target_vy && current_vx >= target_vx {
                return i;
            }
            if current_vx + w > content_width {
                current_vy += 1;
                current_vx = w;
            } else {
                current_vx += w;
            }
        }
        line.len_chars_without_ending()
    }

    fn logical_to_absolute_visual(&self, offset: ScrollOffset) -> u32 {
        // 獲取該邏輯行之前所有行的總視覺高度
        let height_before = self.get_total_visual_height_between(0, offset.logical_line);
        height_before + offset.visual_offset_in_line as u32
    }

    fn absolute_visual_to_logical(&self, abs_visual_y: u32) -> ScrollOffset {
        // 處理邊界情況：如果文件為空或 Y 為 0
        if self.text.len_lines() == 0 || abs_visual_y == 0 {
            return ScrollOffset::default();
        }

        // 使用二分搜尋在 `cumulative_visual_heights` 中快速定位邏輯行。
        // `binary_search` 會找到第一個 `>` 或 `==` 目標值的位置。
        // `partition_point` 在這種情況下更直觀：找到第一個 `>` 目標值的位置。
        let logical_line = self
            .cumulative_visual_heights
            .partition_point(|&h| h <= abs_visual_y)
            .saturating_sub(1); // partition_point 返回的是插入點，所以減1才是目標區間

        // 確保找到的行號不會越界
        let logical_line = logical_line.min(self.text.len_lines() - 1);

        // 獲取該邏輯行之前的總視覺高度
        let height_before = self.get_total_visual_height_between(0, logical_line);

        // 計算在該邏輯行內的視覺偏移
        let visual_offset_in_line = abs_visual_y.saturating_sub(height_before) as usize;

        ScrollOffset {
            logical_line,
            visual_offset_in_line,
        }
    }

    fn screen_to_char_idx(&self, screen_x: u16, screen_y: u16) -> Option<usize> {
        if screen_y >= self.content_height() {
            return None; // 點擊在狀態列或下方
        }

        // --- Y 軸轉換 (此部分邏輯正確，保持不變) ---
        let screen_top_abs_y = self.logical_to_absolute_visual(self.scroll_offset);
        let target_abs_y = screen_top_abs_y + screen_y as u32;
        let target_logical_pos = self.absolute_visual_to_logical(target_abs_y);
        let logical_line_idx = target_logical_pos.logical_line;

        if logical_line_idx >= self.text.len_lines() {
            return Some(self.text.len_chars());
        }

        // --- X 軸轉換 (重寫此部分邏輯) ---
        let line = self.text.line(logical_line_idx);
        let line_start_char_idx = self.text.line_to_char(logical_line_idx);
        let content_width = self.content_width();

        if (screen_x as usize) < Self::LINE_NUMBER_WIDTH {
            return Some(line_start_char_idx); // 點擊行號，定位到行首
        }
        let target_visual_x = (screen_x as usize).saturating_sub(Self::LINE_NUMBER_WIDTH);

        let mut current_visual_y = 0;
        let mut current_visual_x = 0;

        for (char_offset, ch) in line.chars().enumerate() {
            // 檢查是否已到達目標視覺行
            if current_visual_y == target_logical_pos.visual_offset_in_line {
                // 在目標視覺行內，尋找 X 座標
                // 比較 ch 的中點，使用者體驗更好
                let char_width = ch.width_cjk().unwrap_or(1);
                if current_visual_x + char_width / 2 >= target_visual_x {
                    return Some(line_start_char_idx + char_offset);
                }
            }

            // --- 無條件地、為每個字元更新視覺佈局 ---
            let char_width = ch.width_cjk().unwrap_or(1);
            if current_visual_x + char_width > content_width {
                // 換行
                current_visual_y += 1;
                current_visual_x = char_width;
            } else {
                // 不換行
                current_visual_x += char_width;
            }

            // 如果當前字元是換行符，迴圈會自然結束
            if ch == '\n' {
                break;
            }
        }

        // 如果遍歷完畢 (點擊在行尾空白處)，將游標定位到該邏輯行的內容末尾
        Some(line_start_char_idx + line.len_chars_without_ending())
    }

    fn handle_selection(&mut self, in_selection: bool) {
        if in_selection {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(self.cursor);
            }
        } else {
            if let Some((start_char, end_char)) = self.get_selection_range() {
                let start_line = self.text.char_to_line(start_char);
                let end_line = self.text.char_to_line(end_char);
                for i in start_line..=end_line {
                    self.dirty_lines.insert(i);
                }
            }
            self.selection_anchor = None;
        }
    }

    fn copy_selection_to_clipboard(&mut self) -> Result<()> {
        if let Some((start, end)) = self.get_selection_range() {
            Clipboard::new()?.set_text(self.text.slice(start..end).to_string())?;
        }
        Ok(())
    }

    fn delete_selection(&mut self) -> Option<(usize, usize)> {
        if let Some((start, end)) = self.get_selection_range() {
            let start_line = self.text.char_to_line(start);
            let end_line = self.text.char_to_line(end);
            let old_line_count = end_line - start_line + 1;

            self.text.remove(start..end);

            self.cursor = start;

            self.selection_anchor = None;

            return Some((start_line, old_line_count));
        }
        None
    }

    fn paste_from_clipboard(&mut self) -> Result<Option<(usize, usize)>> {
        self.delete_selection();

        let text_to_paste = Clipboard::new()?.get_text()?;

        let start_line = self.text.char_to_line(self.cursor);

        // 插入文字
        self.text.insert(self.cursor, &text_to_paste);

        // 更新游標位置
        self.cursor += text_to_paste.chars().count();

        // 計算影響的新行數
        let new_lines_in_paste = text_to_paste.lines().count();
        // 如果貼上的文字不以換行符結尾，lines() 會少算一行
        let new_line_count = if new_lines_in_paste > 0 && !text_to_paste.ends_with('\n') {
            new_lines_in_paste
        } else if new_lines_in_paste > 0 {
            // 如果貼上的內容包含換行，它會分割當前行
            new_lines_in_paste
        } else {
            1 // 沒有換行符，只影響當前行
        };

        Ok(Some((start_line, new_line_count)))
    }

    pub fn run(&mut self) -> Result<()> {
        execute!(self.stdout, EnterAlternateScreen, EnableMouseCapture)?;

        terminal::enable_raw_mode()?;
        self.rebuild_height_cache();

        while !self.should_quit {
            self.scroll_to_cursor();

            if self.is_dirty {
                queue!(self.stdout, cursor::Hide)?;
                self.draw_updates()?;
                self.refresh_cursor()?;
                queue!(self.stdout, cursor::Show)?;
                self.stdout.flush()?;
                self.is_dirty = false;
            }

            self.handle_event(event::read()?)?;

            if self.is_dirty && !self.should_quit {
                while event::poll(Duration::from_secs(0))? {
                    self.handle_event(event::read()?)?;
                }
            }
        }

        terminal::disable_raw_mode()?;
        execute!(self.stdout, LeaveAlternateScreen, DisableMouseCapture)?;
        Ok(())
    }

    fn handle_event(&mut self, event: Event) -> Result<()> {
        match event {
            Event::Key(ev) => {
                if !ev.is_release() {
                    self.handle_key_event(ev)
                } else {
                    Ok(())
                }
            }
            Event::Mouse(ev) => {
                match ev.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        self.handle_selection(false);
                        if let Some(char_idx) = self.screen_to_char_idx(ev.column, ev.row) {
                            self.cursor = char_idx;
                            self.tmp_x = None;
                        }
                        self.is_dirty = true;
                    }
                    MouseEventKind::Drag(event::MouseButton::Left) => {
                        self.handle_selection(true);
                        if let Some(char_idx) = self.screen_to_char_idx(ev.column, ev.row) {
                            let old_line = self.text.char_to_line(self.cursor);
                            let new_line = self.text.char_to_line(char_idx);

                            for i in min(old_line, new_line)..=max(old_line, new_line) {
                                self.dirty_lines.insert(i);
                            }

                            self.cursor = char_idx;
                            self.tmp_x = None;
                        }
                        self.is_dirty = true;
                    }
                    MouseEventKind::Up(event::MouseButton::Left) => {
                        // 當滑鼠放開時，如果錨點和游標在同一個位置，
                        // 意味著這只是一次點擊，而非拖曳選取，所以清除選取。
                        if self.selection_anchor == Some(self.cursor) {
                            self.selection_anchor = None;
                        }
                        self.is_dirty = true;
                    }
                    MouseEventKind::ScrollUp => {
                        self.curor_move_up();
                        self.is_dirty = true;
                    }
                    MouseEventKind::ScrollDown => {
                        self.cursor_move_down();
                        self.is_dirty = true;
                    }
                    _ => {}
                }
                Ok(())
            }
            Event::Resize(cols, rows) => {
                let old_width = self.terminal_width;

                self.terminal_width = cols;
                self.terminal_height = rows;

                if cols != old_width {
                    self.rebuild_height_cache();
                }

                self.full_redraw_request = true;

                Ok(())
            }
            _ => {
                self.is_dirty = false;
                Ok(())
            }
        }
    }

    fn handle_key_event(&mut self, ev: KeyEvent) -> Result<()> {
        let cursor_line_before = self.text.char_to_line(self.cursor);
        let cursor_before = self.cursor;
        let input: InputEvent = ev.into();
        let effect = if let InputEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
        } = input
        {
            self.execute_command(Command::InputChar(c))?
        } else if let Some(&command) = self.keymap.get(&input) {
            self.execute_command(command)?
        } else {
            CommandEffect::None
        };

        // 1. 處理因文字修改觸發的髒行
        if let CommandEffect::TextChanged(start, old_count, new_count) = effect {
            let delta = self.update_height_cache(start, old_count, new_count);
            if delta != 0 {
                // 高度變化，擠壓下方所有可見行
                for i in start..self.text.len_lines() {
                    self.dirty_lines.insert(i);
                }
            } else {
                // 高度不變，只重繪被修改的行
                for i in 0..new_count {
                    self.dirty_lines.insert(start + i);
                }
            }
        }

        // 2. 處理因游標移動觸發的髒行
        if effect != CommandEffect::None {
            self.is_dirty = true;
            self.dirty_lines.insert(cursor_line_before);
            let cursor_line_after = self.text.char_to_line(self.cursor);
            self.dirty_lines.insert(cursor_line_after);

            let cursor_now = self.cursor;
            self.cursor = cursor_before;
            if effect == CommandEffect::SelectionFixed {
                // Pass
            } else if matches!(effect, CommandEffect::TextChanged(..)) {
                self.handle_selection(false);
            } else {
                self.handle_selection(ev.modifiers.contains(KeyModifiers::SHIFT));
            }
            self.cursor = cursor_now;
        }

        Ok(())
    }

    fn execute_command(&mut self, command: Command) -> Result<CommandEffect> {
        Ok(match command {
            Command::InputChar(ch) => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    self.text.insert_char(self.cursor, ch);
                    self.cursor += 1;
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    let start_line = self.text.char_to_line(self.cursor);
                    self.text.insert_char(self.cursor, ch);
                    self.cursor += 1;
                    CommandEffect::TextChanged(start_line, 1, 1)
                }
            }
            Command::DeleteLeft => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else if self.cursor > 0 {
                    let start_char_before_delete = self.cursor - 1;
                    let start_line = self.text.char_to_line(start_char_before_delete);
                    let old_total_lines = self.text.len_lines();
                    self.text.remove((self.cursor - 1)..self.cursor);
                    self.cursor -= 1;
                    let new_total_lines = self.text.len_lines();
                    let old_line_count = old_total_lines - new_total_lines + 1;
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    CommandEffect::CursorDirty
                }
            }
            Command::DeleteRight => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else if self.cursor < self.text.len_chars() {
                    let start_char_before_delete = self.cursor;
                    let start_line = self.text.char_to_line(start_char_before_delete);
                    let old_total_lines = self.text.len_lines();
                    self.text.remove(self.cursor..(self.cursor + 1));
                    let new_total_lines = self.text.len_lines();
                    let old_line_count = old_total_lines - new_total_lines + 1;
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    CommandEffect::CursorDirty
                }
            }
            Command::DeleteWordRight => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else if self.cursor < self.text.len_chars() {
                    let mut chars = self.text.chars_at(self.cursor);
                    let initial_char = chars.next().unwrap();
                    let kind = classify_char(initial_char);
                    let mut total_offset = 1;

                    if kind != CharKind::Newline {
                        let (offset, _) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                    }

                    let start_delete = self.cursor;
                    let end_delete = self.cursor + total_offset;

                    let start_line = self.text.char_to_line(start_delete);
                    let end_line_before_delete = self
                        .text
                        .char_to_line(end_delete.min(self.text.len_chars()));
                    let old_line_count = end_line_before_delete - start_line + 1;

                    self.text.remove(start_delete..end_delete);

                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    CommandEffect::CursorDirty
                }
            }
            Command::DeleteWordLeft => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else if self.cursor > 0 {
                    let mut chars = self.text.chars_at(self.cursor).reversed();
                    let initial_char = chars.next().unwrap();
                    let kind = classify_char(initial_char);
                    let mut total_offset = 1;

                    if kind != CharKind::Newline {
                        let (offset, _) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                    }

                    let end_delete = self.cursor;
                    let start_delete = self.cursor - total_offset;

                    let start_line = self.text.char_to_line(start_delete);
                    let end_line_before_delete = self.text.char_to_line(end_delete);
                    let old_line_count = end_line_before_delete - start_line + 1;

                    self.text.remove(start_delete..end_delete);
                    self.cursor = start_delete;

                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    CommandEffect::CursorDirty
                }
            }
            Command::InputEnter => {
                self.tmp_x = None;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    self.text.insert_char(self.cursor, '\n');
                    self.cursor += 1;
                    CommandEffect::TextChanged(start_line, old_line_count, 2)
                } else {
                    let start_line = self.text.char_to_line(self.cursor);
                    self.text.insert_char(self.cursor, '\n');
                    self.cursor += 1;
                    CommandEffect::TextChanged(start_line, 1, 2)
                }
            }
            Command::CursorUp => {
                self.curor_move_up();
                CommandEffect::CursorDirty
            }
            Command::CursorDown => {
                self.cursor_move_down();
                CommandEffect::CursorDirty
            }
            Command::CursorLeft => {
                self.tmp_x = None;
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                CommandEffect::CursorDirty
            }
            Command::CursorRight => {
                self.tmp_x = None;
                if self.cursor < self.text.len_chars() {
                    self.cursor += 1;
                }
                CommandEffect::CursorDirty
            }
            Command::CursorWordLeft => {
                self.tmp_x = None;
                if self.cursor > 0 {
                    let mut chars = self.text.chars_at(self.cursor).reversed();

                    let initial_char = chars.next().unwrap();
                    let mut kind = classify_char(initial_char);
                    let mut total_offset = 1;

                    if kind == CharKind::Whitespace {
                        let (offset, next_kind) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                        if next_kind.is_some() {
                            total_offset += 1;
                        }
                        kind = next_kind.unwrap_or(CharKind::Newline);
                    }

                    if kind != CharKind::Newline {
                        let (offset, _) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                    }

                    self.cursor -= total_offset;
                }
                CommandEffect::CursorDirty
            }
            Command::CursorWordRight => {
                self.tmp_x = None;
                if self.cursor < self.text.len_chars() {
                    let mut chars = self.text.chars_at(self.cursor);

                    let initial_char = chars.next().unwrap();
                    let mut kind = classify_char(initial_char);
                    let mut total_offset = 1;

                    if kind == CharKind::Whitespace {
                        let (offset, next_kind) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                        if next_kind.is_some() {
                            total_offset += 1;
                        }
                        kind = next_kind.unwrap_or(CharKind::Newline);
                    }

                    if kind != CharKind::Newline {
                        let (offset, _) = consume_while_kind(&mut chars, kind);
                        total_offset += offset;
                    }

                    self.cursor += total_offset;
                }
                CommandEffect::CursorDirty
            }
            Command::CursorHome => {
                self.tmp_x = None;
                self.cursor = self.text.line_to_char(self.text.char_to_line(self.cursor));
                CommandEffect::CursorDirty
            }
            Command::CursorEnd => {
                self.tmp_x = None;
                let line_idx = self.text.char_to_line(self.cursor);
                let line_len = self.text.line(line_idx).len_chars_without_ending();
                self.cursor = self.text.line_to_char(line_idx) + line_len;
                CommandEffect::CursorDirty
            }
            Command::CursorPageUp => {
                self.cursor = 0;
                CommandEffect::CursorDirty
            }
            Command::CursorPageDown => {
                self.cursor = self.text.len_chars();
                CommandEffect::CursorDirty
            }
            Command::SelectAll => {
                self.tmp_x = None;
                self.selection_anchor = Some(0);
                self.cursor = self.text.len_chars();
                self.full_redraw_request = true;
                CommandEffect::SelectionFixed
            }
            Command::SelectLine => {
                self.tmp_x = None;
                let cursor_line_idx = self.text.char_to_line(self.cursor);
                let line_start_char_idx = self.text.line_to_char(cursor_line_idx);
                let next_line_start_char_idx = if cursor_line_idx + 1 < self.text.len_lines() {
                    self.text.line_to_char(cursor_line_idx + 1)
                } else {
                    self.text.len_chars()
                };

                self.selection_anchor = Some(line_start_char_idx);
                self.cursor = next_line_start_char_idx;

                self.dirty_lines.insert(cursor_line_idx);
                if cursor_line_idx + 1 < self.text.len_lines() {
                    self.dirty_lines.insert(cursor_line_idx + 1);
                }
                CommandEffect::SelectionFixed
            }
            Command::TextCopy => {
                self.copy_selection_to_clipboard()?;
                CommandEffect::None
            }
            Command::TextCopyAndClearSelection => {
                self.copy_selection_to_clipboard()?;
                self.handle_selection(false);
                CommandEffect::CursorDirty
            }
            Command::TextPaste => {
                if let Some((start_line, new_line_count)) = self.paste_from_clipboard()? {
                    CommandEffect::TextChanged(start_line, 1, new_line_count)
                } else {
                    CommandEffect::None
                }
            }
            Command::TextCut => {
                self.copy_selection_to_clipboard()?;
                if let Some((start_line, old_line_count)) = self.delete_selection() {
                    CommandEffect::TextChanged(start_line, old_line_count, 1)
                } else {
                    CommandEffect::None
                }
            }
            Command::Exit => {
                self.should_quit = true;
                CommandEffect::None
            }
        })
    }

    fn curor_move_up(&mut self) {
        let y = self.text.char_to_line(self.cursor);
        let start_idx = self.text.line_to_char(y);
        let x_offset = self.cursor - start_idx;
        let (vx, vy) = self.char_idx_to_visual_pos_in_line(y, x_offset);
        let target_vx = *self.tmp_x.get_or_insert(vx);

        if vy > 0 {
            let new_offset = self.visual_pos_to_char_idx_in_line(y, target_vx, vy - 1);
            self.cursor = start_idx + new_offset;
        } else if y > 0 {
            let prev_line_height = self.get_visual_height_for_line(y - 1);
            let target_vy = if prev_line_height > 0 {
                prev_line_height - 1
            } else {
                0
            };
            let prev_start_idx = self.text.line_to_char(y - 1);
            let new_offset =
                self.visual_pos_to_char_idx_in_line(y - 1, target_vx, target_vy as usize);
            self.cursor = prev_start_idx + new_offset;
        }
    }

    fn cursor_move_down(&mut self) {
        let y = self.text.char_to_line(self.cursor);
        let start_idx = self.text.line_to_char(y);
        let x_offset = self.cursor - start_idx;
        let (vx, vy) = self.char_idx_to_visual_pos_in_line(y, x_offset);
        let current_line_height = self.get_visual_height_for_line(y);
        let target_vx = *self.tmp_x.get_or_insert(vx);

        if vy < current_line_height as usize - 1 {
            let new_offset = self.visual_pos_to_char_idx_in_line(y, target_vx, vy + 1);
            self.cursor = start_idx + new_offset;
        } else if y < self.text.len_lines().saturating_sub(1) {
            let next_start_idx = self.text.line_to_char(y + 1);
            let new_offset = self.visual_pos_to_char_idx_in_line(y + 1, target_vx, 0);
            self.cursor = next_start_idx + new_offset;
        }
    }

    fn get_total_visual_height_between(&self, start: usize, end: usize) -> u32 {
        if end >= self.cumulative_visual_heights.len() || start > end {
            return 0;
        }
        self.cumulative_visual_heights[end] - self.cumulative_visual_heights[start]
    }

    fn content_width(&self) -> usize {
        let w = (self.terminal_width as usize)
            .saturating_sub(Self::LINE_NUMBER_WIDTH)
            .saturating_sub(1);
        if w == 0 { 1_000_000_000 } else { w }
    }

    fn content_height(&self) -> u16 {
        self.terminal_height.saturating_sub(Self::STATUS_BAR_HEIGHT)
    }

    fn get_visual_height_for_line(&self, line_idx: usize) -> u16 {
        if line_idx + 1 >= self.cumulative_visual_heights.len() {
            return 1;
        }
        (self.cumulative_visual_heights[line_idx + 1] - self.cumulative_visual_heights[line_idx])
            as u16
    }

    fn scroll_to_cursor(&mut self) {
        let old_offset = self.scroll_offset;
        let content_height = self.content_height() as u32;
        if content_height == 0 {
            return;
        }

        // 1. 計算游標的絕對視覺 Y
        let cursor_logical_y = self.text.char_to_line(self.cursor);
        let start_of_line_char_idx = self.text.line_to_char(cursor_logical_y);
        let (_, visual_offset_in_line) = self
            .char_idx_to_visual_pos_in_line(cursor_logical_y, self.cursor - start_of_line_char_idx);
        let cursor_abs_y = self.logical_to_absolute_visual(ScrollOffset {
            logical_line: cursor_logical_y,
            visual_offset_in_line,
        });

        // 2. 計算當前螢幕的絕對視覺邊界
        let screen_top_abs_y = self.logical_to_absolute_visual(self.scroll_offset);
        let screen_bottom_abs_y = screen_top_abs_y + content_height - 1;

        // 3. 決定新的螢幕頂部絕對視覺 Y
        let new_top_abs_y;
        if cursor_abs_y < screen_top_abs_y {
            // 硬性捲動：游標在上方
            new_top_abs_y = cursor_abs_y;
        } else if cursor_abs_y > screen_bottom_abs_y {
            // 硬性捲動：游標在下方
            new_top_abs_y = cursor_abs_y - content_height + 1;
        } else {
            // 軟性捲動
            let middle_offset = content_height / 2;
            let ideal_top_abs_y = cursor_abs_y.saturating_sub(middle_offset);
            let total_doc_visual_height =
                self.get_total_visual_height_between(0, self.text.len_lines());
            let remaining_height = total_doc_visual_height.saturating_sub(ideal_top_abs_y);

            if remaining_height < content_height {
                // 貼底邏輯
                new_top_abs_y = total_doc_visual_height.saturating_sub(content_height);
            } else {
                // 置中邏輯
                new_top_abs_y = ideal_top_abs_y;
            }
        }

        // 4. 將新的絕對視覺 Y 轉換回 ScrollOffset 並更新狀態
        let final_offset = self.absolute_visual_to_logical(new_top_abs_y);
        self.scroll_offset = final_offset;
        if self.scroll_offset != old_offset {
            self.full_redraw_request = true;
        }
    }

    fn draw_updates(&mut self) -> Result<()> {
        if self.full_redraw_request {
            self.dirty_lines.clear();

            // --- 新的、更精確的髒行計算邏輯 ---
            let mut y: u16 = 0;
            // 處理第一個 (可能部分可見的) 邏輯行
            let first_line_idx = self.scroll_offset.logical_line;
            if first_line_idx < self.text.len_lines() {
                self.dirty_lines.insert(first_line_idx);
                let line_height = self.get_visual_height_for_line(first_line_idx);
                y += line_height.saturating_sub(self.scroll_offset.visual_offset_in_line as u16);
            }
            // 處理後續的邏輯行
            for i in (first_line_idx + 1)..self.text.len_lines() {
                if y >= self.content_height() {
                    break;
                }
                self.dirty_lines.insert(i);
                y += self.get_visual_height_for_line(i);
            }
            // --- 邏輯結束 ---

            self.full_redraw_request = false;
        }
        for line_idx in mem::take(&mut self.dirty_lines) {
            if line_idx < self.text.len_lines() {
                self.draw_single_line(line_idx)?;
            }
        }
        self.cleanup_bottom()?;
        self.draw_status_bar()?;

        Ok(())
    }

    fn draw_status_bar(&mut self) -> Result<()> {
        let content_height = self.content_height();
        let line_idx = self.text.char_to_line(self.cursor);

        queue!(
            self.stdout,
            MoveTo(0, content_height),
            Clear(ClearType::CurrentLine),
            Print(format_args!(
                "Ln {}, Col {}",
                line_idx + 1,
                self.cursor - self.text.line_to_char(line_idx) + 1
            ))
        )?;

        Ok(())
    }

    fn draw_single_line(&mut self, line_idx: usize) -> Result<()> {
        let screen_top_abs_y = self.logical_to_absolute_visual(self.scroll_offset);
        let line_top_abs_y = self.get_total_visual_height_between(0, line_idx);
        let content_width = self.content_width();
        let line = self.text.line(line_idx);
        let line_start_char = self.text.line_to_char(line_idx);
        let selection = self.get_selection_range();

        let mut is_first_chunk_of_line = true;
        let mut char_offset_in_line = 0;

        // 使用 enumerate() 來獲取 visual_offset_in_line
        for (visual_offset_in_line, visual_line_chunk) in
            line.chunk_by_width_cjk(content_width).enumerate()
        {
            let current_abs_y = line_top_abs_y + visual_offset_in_line as u32;

            if current_abs_y < screen_top_abs_y {
                char_offset_in_line += visual_line_chunk.len_chars();
                continue;
            }

            let screen_y = current_abs_y.saturating_sub(screen_top_abs_y) as u16;
            if screen_y >= self.content_height() {
                break;
            }

            // --- 繪製行號和清空行 ---
            queue!(
                self.stdout,
                MoveTo(0, screen_y),
                Clear(ClearType::CurrentLine)
            )?;
            if is_first_chunk_of_line {
                queue!(self.stdout, Print(format_args!("{:>4} │ ", line_idx + 1)))?;
                is_first_chunk_of_line = false;
            } else {
                queue!(self.stdout, Print(format_args!("{:>4} │ ", " ")))?;
            }

            let chunk_abs_start = line_start_char + char_offset_in_line;
            let chunk_len = visual_line_chunk.len_chars_without_ending();
            let chunk_to_draw = visual_line_chunk.slice(..chunk_len);

            if chunk_to_draw.len_chars() == 0 {
                if let Some((sel_start, sel_end)) = selection {
                    // 檢查這個空行的位置 (chunk_abs_start) 是否在選取範圍內
                    if chunk_abs_start >= sel_start && chunk_abs_start < sel_end {
                        // 如果是，繪製一個反白的空格
                        queue!(
                            self.stdout,
                            SetAttribute(Attribute::Reverse),
                            Print(" "),
                            SetAttribute(Attribute::Reset)
                        )?;
                    }
                }
            } else {
                // --- 分段式渲染邏輯 (適用於非空行) ---
                if let Some((sel_start, sel_end)) = selection {
                    let overlap_start = sel_start.max(chunk_abs_start);
                    let overlap_end = sel_end.min(chunk_abs_start + chunk_len);

                    if overlap_start < overlap_end {
                        // 有交集
                        let chunk_sel_start = overlap_start - chunk_abs_start;
                        let chunk_sel_end = overlap_end - chunk_abs_start;

                        queue!(self.stdout, Print(chunk_to_draw.slice(..chunk_sel_start)))?;
                        queue!(self.stdout, SetAttribute(Attribute::Reverse))?;
                        queue!(
                            self.stdout,
                            Print(chunk_to_draw.slice(chunk_sel_start..chunk_sel_end))
                        )?;
                        queue!(self.stdout, SetAttribute(Attribute::Reset))?;
                        queue!(self.stdout, Print(chunk_to_draw.slice(chunk_sel_end..)))?;
                    } else {
                        // 無交集
                        queue!(self.stdout, Print(chunk_to_draw))?;
                    }
                } else {
                    // 完全沒有選取
                    queue!(self.stdout, Print(chunk_to_draw))?;
                }
            }

            char_offset_in_line += visual_line_chunk.len_chars();
        }
        Ok(())
    }

    fn cleanup_bottom(&mut self) -> Result<()> {
        let mut drawn_height: u16 = 0;
        let content_height = self.content_height();

        let first_line_idx = self.scroll_offset.logical_line;
        if first_line_idx < self.text.len_lines() {
            let first_line_total_height = self.get_visual_height_for_line(first_line_idx);
            drawn_height += first_line_total_height
                .saturating_sub(self.scroll_offset.visual_offset_in_line as u16);
        }

        for line_idx in (first_line_idx + 1)..self.text.len_lines() {
            if drawn_height >= content_height {
                break;
            }
            drawn_height += self.get_visual_height_for_line(line_idx);
        }

        let final_drawn_height = drawn_height.min(content_height);
        for y_to_clear in final_drawn_height..content_height {
            queue!(
                self.stdout,
                MoveTo(0, y_to_clear),
                Clear(ClearType::CurrentLine)
            )?;
        }
        Ok(())
    }

    fn refresh_cursor(&mut self) -> Result<()> {
        let line_idx = self.text.char_to_line(self.cursor);
        if line_idx >= self.text.len_lines() {
            return Ok(());
        }

        let start_idx = self.text.line_to_char(line_idx);
        let (vx, vy_in_line) =
            self.char_idx_to_visual_pos_in_line(line_idx, self.cursor - start_idx);

        let cursor_abs_y = self.logical_to_absolute_visual(ScrollOffset {
            logical_line: line_idx,
            visual_offset_in_line: vy_in_line,
        });

        let screen_top_abs_y = self.logical_to_absolute_visual(self.scroll_offset);

        // 只有當游標在可視範圍內時才計算
        if cursor_abs_y >= screen_top_abs_y {
            let screen_y = cursor_abs_y.saturating_sub(screen_top_abs_y) as u16;
            if screen_y < self.content_height() {
                let screen_x = (vx + Self::LINE_NUMBER_WIDTH) as u16;
                queue!(self.stdout, MoveTo(screen_x, screen_y))?;
            }
        }

        Ok(())
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }
}

pub trait RopeSliceExt<'a> {
    fn chunk_by_width_cjk(&'a self, max_width: usize) -> impl Iterator<Item = RopeSlice<'a>>;
    /// Total number of chars in the RopeSlice, excluding a trailing \n.
    ///
    /// Runs in O(log len(slice)) time.
    fn len_chars_without_ending(&'a self) -> usize;
    // fn trim_end(&'a self) -> RopeTrimEnd<'a>;
}

impl<'a> RopeSliceExt<'a> for RopeSlice<'a> {
    fn chunk_by_width_cjk(&'a self, max_width: usize) -> impl Iterator<Item = RopeSlice<'a>> {
        if self.len_chars() == 0 || max_width == 0 {
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
                end_idx = *idx + 1;
                chars.next();
            }
            if start_idx == end_idx && chars.peek().is_some() {
                end_idx = start_idx + 1;
                chars.next();
            }
            Some(self.slice(start_idx..end_idx))
        }))
    }
    fn len_chars_without_ending(&'a self) -> usize {
        let len = self.len_chars();
        if len > 0 && self.char(len - 1) == '\n' {
            if len > 1 && self.char(len - 2) == '\r' {
                len - 2
            } else {
                len - 1
            }
        } else {
            len
        }
    }
    // fn trim_end(&'a self) -> RopeTrimEnd<'a> {
    //     RopeTrimEnd(*self)
    // }
}

#[derive(Debug, PartialEq, Eq)]
enum CharKind {
    Word,
    Symbol,
    Whitespace,
    Newline, // 將換行符視為一個獨立的類別
}

fn classify_char(c: char) -> CharKind {
    if c.is_alphanumeric() || c == '_' {
        CharKind::Word
    } else if c.is_whitespace() {
        if c == '\n' || c == '\r' {
            CharKind::Newline
        } else {
            CharKind::Whitespace
        }
    } else {
        CharKind::Symbol
    }
}

/// (offset, next_kind)
fn consume_while_kind(iter: &mut Chars<'_>, kind: CharKind) -> (usize, Option<CharKind>) {
    let mut offset = 0;
    for c in iter {
        let current_kind = classify_char(c);
        if current_kind != kind {
            return (offset, Some(current_kind));
        }
        offset += 1;
    }
    (offset, None)
}

// struct RopeTrimEnd<'a>(RopeSlice<'a>);

// impl<'a> RopeTrimEnd<'a> {
//     fn as_slice(&self) -> RopeSlice<'a> {
//         for (i, ch) in self.0.chars_at(self.0.len_chars()).reversed().enumerate() {
//             if !ch.is_whitespace() {
//                 return self.0.slice(0..(self.0.len_chars() - i));
//             }
//         }
//         self.0.slice(0..0)
//     }
// }

// impl<'a> std::fmt::Display for RopeTrimEnd<'a> {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let mut iter = self.0.chunks().peekable();
//         while let Some(item) = iter.next() {
//             if iter.peek().is_none() {
//                 write!(f, "{}", item.trim_end())?;
//             } else {
//                 write!(f, "{item}")?;
//             }
//         }
//         Ok(())
//     }
// }
