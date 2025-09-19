use crate::definitions::EditorPreferences;
use anyhow::{anyhow, Context, Result};
use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthChar;

/// A simple text editor providing insertion, deletion, and cursor/viewport syncing.
pub struct Editor {
    buffer: Rope,
    cursor_line: usize,
    cursor_col: usize,
    viewport_line: usize,
    viewport_height: usize,
    viewport_width: usize,
    viewport_line_offset: usize,
    file_path: Option<PathBuf>,
    dirty: bool,
    selection_anchor: Option<(usize, usize)>,
    preferences: EditorPreferences,
}

impl Editor {
    pub fn new(preferences: EditorPreferences) -> Self {
        Self {
            buffer: Rope::from_str(""),
            cursor_line: 0,
            cursor_col: 0,
            viewport_line: 0,
            viewport_height: 1,
            viewport_width: 1,
            viewport_line_offset: 0,
            file_path: None,
            dirty: false,
            selection_anchor: None,
            preferences,
        }
    }

    #[allow(dead_code)]
    pub fn with_placeholder(content: &str, preferences: EditorPreferences) -> Self {
        let mut editor = Self::new(preferences);
        editor.buffer = Rope::from_str(content);
        editor.clamp_cursor();
        editor.dirty = false;
        editor
    }

    pub fn new_document(&mut self) {
        self.buffer = Rope::from_str("");
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.viewport_line = 0;
        self.viewport_line_offset = 0;
        self.file_path = None;
        self.dirty = false;
        self.selection_anchor = None;
        self.ensure_cursor_visible_with_wrap();
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("無法讀取檔案: {}", path.display()))?;
        self.buffer = Rope::from_str(&contents);
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.viewport_line = 0;
        self.viewport_line_offset = 0;
        self.file_path = Some(path.to_path_buf());
        self.clamp_cursor();
        self.dirty = false;
        self.selection_anchor = None;
        Ok(())
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\r' {
            return;
        }
        self.delete_selection();
        let idx = self.char_index();
        self.buffer.insert_char(idx, ch);
        self.dirty = true;
        if ch == '\n' {
            self.cursor_line += 1;
            self.cursor_col = 0;
        } else {
            self.cursor_col += 1;
        }
        self.clamp_cursor();
    }

    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    pub fn backspace(&mut self) {
        if self.delete_selection() {
            return;
        }
        let idx = self.char_index();
        if idx == 0 {
            return;
        }
        let prev_idx = idx - 1;
        let ch = self.buffer.char(prev_idx);
        self.buffer.remove(prev_idx..idx);
        self.dirty = true;
        if ch == '\n' {
            if self.cursor_line > 0 {
                self.cursor_line -= 1;
                self.cursor_col = self.buffer.line(self.cursor_line).len_chars();
            }
        } else if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
        self.clamp_cursor();
    }

    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        let idx = self.char_index();
        if idx >= self.buffer.len_chars() {
            return;
        }
        let next_idx = idx + 1;
        self.buffer.remove(idx..next_idx);
        self.dirty = true;
        self.clamp_cursor();
    }

    pub fn move_cursor(&mut self, delta_line: isize, delta_col: isize) {
        self.clear_selection();
        let line_count = self.buffer.len_lines();
        let new_line = self
            .cursor_line
            .saturating_add_signed(delta_line)
            .min(line_count.saturating_sub(1));
        self.cursor_line = new_line;
        let line_len = self.buffer.line(self.cursor_line).len_chars();
        let col_signed = self.cursor_col as isize + delta_col;
        self.cursor_col = col_signed.clamp(0, line_len as isize) as usize;
        self.clamp_cursor();
    }

    pub fn move_to_line_start(&mut self) {
        self.cursor_col = 0;
        self.clear_selection();
        self.clamp_cursor();
    }

    pub fn move_to_line_end(&mut self) {
        let len = self.buffer.line(self.cursor_line).len_chars();
        self.cursor_col = len;
        self.clear_selection();
        self.clamp_cursor();
    }

    pub fn set_viewport(&mut self, height: usize, width: usize) {
        self.viewport_height = height.max(1);
        self.viewport_width = width.max(1);
        if self.viewport_line >= self.buffer.len_lines() {
            if self.buffer.len_lines() == 0 {
                self.viewport_line = 0;
            } else {
                self.viewport_line = self.buffer.len_lines().saturating_sub(1);
            }
            self.viewport_line_offset = 0;
        }
        self.normalize_viewport_top();
        self.ensure_cursor_visible_with_wrap();
    }

    pub fn visual_segments(&self, line_idx: usize, width: usize) -> Vec<String> {
        let mut text = self.buffer.line(line_idx).to_string();
        if text.ends_with('\n') {
            text.pop();
        }
        let expanded = self.expand_tabs(&text);
        self.wrap_text(&expanded, width.max(1))
    }

    pub fn cursor_visual_position(&self) -> (usize, usize, usize) {
        let width = self.viewport_width.max(1);
        let visual_col = self.cursor_visual_col();
        let subline = visual_col / width;
        let col_in_subline = visual_col % width;
        (self.cursor_line, subline, col_in_subline)
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn viewport_start(&self) -> usize {
        self.viewport_line
    }

    pub fn viewport_line_offset(&self) -> usize {
        self.viewport_line_offset
    }

    pub fn clear_selection(&mut self) {
        self.selection_anchor = None;
    }

    pub fn selection_display_ranges(&self, line_idx: usize) -> Vec<(usize, usize)> {
        let Some(((start_line, start_col), (end_line, end_col))) = self.selection_positions()
        else {
            return Vec::new();
        };

        if line_idx < start_line || line_idx > end_line {
            return Vec::new();
        }

        let line_len = self.line_length_without_newline(line_idx);
        let start_char = if line_idx == start_line {
            start_col.min(line_len)
        } else {
            0
        };
        let end_char = if line_idx == end_line {
            end_col.min(line_len)
        } else {
            line_len
        };

        if start_char >= end_char {
            return Vec::new();
        }

        let start_display = self.display_columns_up_to(line_idx, start_char);
        let end_display = self.display_columns_up_to(line_idx, end_char);
        if start_display >= end_display {
            Vec::new()
        } else {
            vec![(start_display, end_display)]
        }
    }

    pub fn save(&mut self) -> Result<PathBuf> {
        let path = self
            .file_path
            .clone()
            .ok_or_else(|| anyhow!("尚未指定檔案路徑"))?;
        self.write_to(&path)?;
        Ok(path)
    }

    pub fn save_as<P: AsRef<Path>>(&mut self, path: P) -> Result<PathBuf> {
        let path = path.as_ref().to_path_buf();
        self.write_to(&path)?;
        self.file_path = Some(path.clone());
        Ok(path)
    }

    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn total_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    pub fn buffer_content(&self) -> String {
        self.buffer.to_string()
    }

    fn write_to(&mut self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("無法建立目錄: {}", parent.display()))?;
            }

        let mut contents = String::new();
        for chunk in self.buffer.chunks() {
            contents.push_str(chunk);
        }
        fs::write(path, contents).with_context(|| format!("無法寫入檔案: {}", path.display()))?;
        self.dirty = false;
        Ok(())
    }

    fn char_index(&self) -> usize {
        let base = self.buffer.line_to_char(self.cursor_line);
        let line_len = self.buffer.line(self.cursor_line).len_chars();
        base + self.cursor_col.min(line_len)
    }

    fn clamp_cursor(&mut self) {
        if self.cursor_line >= self.buffer.len_lines() {
            self.cursor_line = self.buffer.len_lines().saturating_sub(1);
        }
        let line_len = self.buffer.line(self.cursor_line).len_chars();
        self.cursor_col = self.cursor_col.min(line_len);
        self.ensure_cursor_visible_with_wrap();
    }

    fn delete_selection(&mut self) -> bool {
        let Some(((start_line, start_col), (end_line, end_col))) = self.selection_positions()
        else {
            return false;
        };

        let start_idx = self.line_col_to_char_index(start_line, start_col);
        let end_idx = self.line_col_to_char_index(end_line, end_col);
        if start_idx == end_idx {
            self.selection_anchor = None;
            return false;
        }

        self.buffer.remove(start_idx..end_idx);
        self.cursor_line = start_line;
        self.cursor_col = start_col;
        self.selection_anchor = None;
        self.dirty = true;
        self.clamp_cursor();
        true
    }

    fn selection_positions(&self) -> Option<((usize, usize), (usize, usize))> {
        let anchor = self.selection_anchor?;
        let current = (self.cursor_line, self.cursor_col);
        if anchor == current {
            return None;
        }
        if Self::compare_positions(anchor, current) <= 0 {
            Some((anchor, current))
        } else {
            Some((current, anchor))
        }
    }

    fn compare_positions(a: (usize, usize), b: (usize, usize)) -> isize {
        if a.0 == b.0 {
            a.1 as isize - b.1 as isize
        } else {
            a.0 as isize - b.0 as isize
        }
    }

    fn line_col_to_char_index(&self, line: usize, col: usize) -> usize {
        let base = self.buffer.line_to_char(line);
        let len = self.buffer.line(line).len_chars();
        base + col.min(len)
    }

    fn line_length_without_newline(&self, line_idx: usize) -> usize {
        self.buffer
            .line(line_idx)
            .chars()
            .take_while(|ch| *ch != '\n')
            .count()
    }

    fn display_columns_up_to(&self, line_idx: usize, char_col: usize) -> usize {
        self.buffer
            .line(line_idx)
            .chars()
            .take_while(|ch| *ch != '\n')
            .take(char_col)
            .map(|ch| self.display_width(ch))
            .sum()
    }

    fn char_col_from_display(&self, line_idx: usize, display_col: usize) -> usize {
        let mut display = 0;
        let mut count = 0;
        for ch in self.buffer.line(line_idx).chars() {
            if ch == '\n' {
                break;
            }
            let w = self.display_width(ch);
            if display + w > display_col {
                break;
            }
            display += w;
            count += 1;
        }
        count
    }

    pub fn move_cursor_visual(
        &mut self,
        viewport_line: usize,
        viewport_offset: usize,
        width: usize,
        visual_row: usize,
        visual_col: usize,
        extend: bool,
    ) {
        if !extend {
            self.selection_anchor = None;
        }
        let previous = (self.cursor_line, self.cursor_col);
        let pos = self
            .position_for_visual(
                viewport_line,
                viewport_offset,
                width,
                visual_row,
                visual_col,
            )
            .unwrap_or_else(|| self.end_position());
        self.cursor_line = pos.0;
        self.cursor_col = pos.1;
        self.clamp_cursor();
        if extend {
            if self.selection_anchor.is_none() {
                self.selection_anchor = Some(previous);
            }
        } else {
            self.selection_anchor = None;
        }
    }

    pub fn position_for_visual(
        &self,
        viewport_line: usize,
        viewport_offset: usize,
        width: usize,
        mut visual_row: usize,
        mut visual_col: usize,
    ) -> Option<(usize, usize)> {
        if width == 0 {
            return None;
        }
        let mut line_idx = viewport_line;
        let mut offset = viewport_offset;
        let total_lines = self.total_lines();
        while line_idx < total_lines {
            let segments = self.visual_segments(line_idx, width);
            let seg_count = segments.len().max(1);
            for (segment_idx, _segment) in segments.iter().enumerate() {
                if offset > 0 {
                    offset -= 1;
                    continue;
                }
                if visual_row == 0 {
                    let segment_start = segment_idx * width;
                    visual_col = visual_col.min(width);
                    let target_display = segment_start + visual_col;
                    let char_col = self.char_col_from_display(line_idx, target_display);
                    return Some((line_idx, char_col));
                } else {
                    visual_row -= 1;
                }
            }
            if offset > 0 {
                offset = offset.saturating_sub(seg_count);
            }
            line_idx += 1;
        }
        None
    }

    fn end_position(&self) -> (usize, usize) {
        if self.total_lines() == 0 {
            (0, 0)
        } else {
            let last_line = self.total_lines().saturating_sub(1);
            let len = self.buffer.line(last_line).len_chars();
            (last_line, len)
        }
    }

    pub fn display_width(&self, ch: char) -> usize {
        if ch == '\t' {
            self.preferences.tab_width
        } else {
            UnicodeWidthChar::width(ch).unwrap_or(1).max(1)
        }
    }

    fn ensure_cursor_visible_with_wrap(&mut self) {
        let width = self.viewport_width.max(1);
        if self.cursor_line < self.viewport_line {
            self.viewport_line = self.cursor_line;
            self.viewport_line_offset = 0;
        }

        if self.viewport_line >= self.buffer.len_lines() {
            if self.buffer.len_lines() == 0 {
                self.viewport_line = 0;
            } else {
                self.viewport_line = self.buffer.len_lines().saturating_sub(1);
            }
            self.viewport_line_offset = 0;
        }

        self.normalize_viewport_top();

        let (_, cursor_subline, _) = self.cursor_visual_position();
        if self.cursor_line == self.viewport_line && self.viewport_line_offset > cursor_subline {
            self.viewport_line_offset = cursor_subline;
        }

        loop {
            let rows_to_cursor =
                self.rows_from_viewport_top(self.cursor_line, cursor_subline, width);
            if rows_to_cursor < self.viewport_height {
                break;
            }
            let excess = rows_to_cursor - (self.viewport_height - 1);
            if excess == 0 {
                break;
            }
            self.scroll_rows_down(excess, width);
            if self.viewport_line >= self.buffer.len_lines() {
                break;
            }
            self.normalize_viewport_top();
        }
    }

    fn rows_from_viewport_top(
        &self,
        target_line: usize,
        target_subline: usize,
        width: usize,
    ) -> usize {
        if target_line < self.viewport_line {
            return 0;
        }

        if self.viewport_line == target_line {
            return target_subline.saturating_sub(self.viewport_line_offset);
        }

        let mut rows = 0;
        let mut line = self.viewport_line;
        let offset = self.viewport_line_offset;

        let first_rows = self.visual_row_count(line, width);
        rows += first_rows.saturating_sub(offset);
        line += 1;

        while line < target_line {
            rows += self.visual_row_count(line, width);
            line += 1;
        }

        rows + target_subline
    }

    fn scroll_rows_down(&mut self, mut rows: usize, width: usize) {
        while rows > 0 {
            let current_rows = self.visual_row_count(self.viewport_line, width);
            let remaining_in_current = current_rows.saturating_sub(self.viewport_line_offset);
            if remaining_in_current == 0 {
                if self.viewport_line + 1 < self.buffer.len_lines() {
                    self.viewport_line += 1;
                    self.viewport_line_offset = 0;
                    continue;
                } else {
                    break;
                }
            }

            if rows < remaining_in_current {
                self.viewport_line_offset += rows;
                break;
            } else {
                rows -= remaining_in_current;
                if self.viewport_line + 1 < self.buffer.len_lines() {
                    self.viewport_line += 1;
                    self.viewport_line_offset = 0;
                } else {
                    self.viewport_line_offset = current_rows.saturating_sub(1);
                    break;
                }
            }
        }
    }

    fn normalize_viewport_top(&mut self) {
        let width = self.viewport_width.max(1);
        let rows = self.visual_row_count(self.viewport_line, width);
        if rows == 0 {
            self.viewport_line_offset = 0;
        } else if self.viewport_line_offset >= rows {
            self.viewport_line_offset = rows - 1;
        }
    }

    fn cursor_visual_col(&self) -> usize {
        let line = self.buffer.line(self.cursor_line).to_string();
        let mut col = 0;
        for (idx, ch) in line.chars().enumerate() {
            if ch == '\n' {
                break;
            }
            if idx >= self.cursor_col {
                break;
            }
            col += if ch == '\t' {
                self.preferences.tab_width
            } else {
                ch.width().unwrap_or(1)
            };
        }
        col
    }

    fn visual_row_count(&self, line_idx: usize, width: usize) -> usize {
        let segments = self.visual_segments(line_idx, width);
        segments.len().max(1)
    }

    fn expand_tabs(&self, text: &str) -> String {
        text.replace('\t', &" ".repeat(self.preferences.tab_width))
    }

    fn wrap_text(&self, text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![String::new()];
        }
        let mut segments = Vec::new();
        let mut current = String::new();
        let mut width_acc = 0;

        for ch in text.chars() {
            let w = self.display_width(ch);
            if width_acc + w > width && !current.is_empty() {
                segments.push(current.clone());
                current.clear();
                width_acc = 0;
            }
            current.push(ch);
            width_acc += w;
        }

        if current.is_empty() {
            segments.push(String::new());
        } else {
            segments.push(current);
        }
        segments
    }
}
