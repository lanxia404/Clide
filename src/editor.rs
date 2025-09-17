use anyhow::{Context, Result};
use ropey::Rope;
use std::fs;
use std::path::{Path, PathBuf};

/// 簡易文字編輯器，提供插入、刪除以及游標與視窗同步功能。
pub struct Editor {
    buffer: Rope,
    cursor_line: usize,
    cursor_col: usize,
    viewport_line: usize,
    viewport_height: usize,
    file_path: Option<PathBuf>,
    dirty: bool,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Rope::from_str(""),
            cursor_line: 0,
            cursor_col: 0,
            viewport_line: 0,
            viewport_height: 1,
            file_path: None,
            dirty: false,
        }
    }

    pub fn with_placeholder(content: &str) -> Self {
        let mut editor = Self::new();
        editor.buffer = Rope::from_str(content);
        editor.clamp_cursor();
        editor.dirty = false;
        editor
    }

    pub fn open_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path)
            .with_context(|| format!("無法讀取檔案: {}", path.display()))?;
        self.buffer = Rope::from_str(&contents);
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.viewport_line = 0;
        self.file_path = Some(path.to_path_buf());
        self.clamp_cursor();
        self.dirty = false;
        Ok(())
    }

    pub fn insert_char(&mut self, ch: char) {
        if ch == '\r' {
            return;
        }
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

    pub fn move_cursor(&mut self, delta_line: isize, delta_col: isize) {
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
    }

    pub fn move_to_line_end(&mut self) {
        let len = self.buffer.line(self.cursor_line).len_chars();
        self.cursor_col = len;
    }

    pub fn set_viewport_height(&mut self, height: usize) {
        self.viewport_height = height.max(1);
        self.ensure_cursor_visible();
    }

    pub fn lines_in_viewport(&mut self) -> Vec<String> {
        self.ensure_cursor_visible();
        let end = (self.viewport_line + self.viewport_height).min(self.buffer.len_lines());
        (self.viewport_line..end)
            .map(|line_idx| {
                let mut line = self.buffer.line(line_idx).to_string();
                if line.ends_with('\n') {
                    line.pop();
                }
                line
            })
            .collect()
    }

    pub fn cursor(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn viewport_start(&self) -> usize {
        self.viewport_line
    }

    pub fn file_path(&self) -> Option<&Path> {
        self.file_path.as_deref()
    }

    #[allow(dead_code)]
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn total_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    fn ensure_cursor_visible(&mut self) {
        if self.cursor_line < self.viewport_line {
            self.viewport_line = self.cursor_line;
        } else if self.cursor_line >= self.viewport_line + self.viewport_height {
            self.viewport_line = self.cursor_line + 1 - self.viewport_height;
        }
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
    }
}
