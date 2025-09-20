// src/components/editor/mod.rs

pub mod view;

// ... (The rest of the original src/editor.rs content)
use std::cmp::min;
use std::collections::HashMap;
use std::path::PathBuf;
use ratatui::text::Line;

#[derive(Clone)]
pub struct EditorState {
    content: Vec<String>,
    cursor_row: usize,
    cursor_col: usize,
}

pub struct Editor {
    pub path: Option<PathBuf>,
    pub content: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub vertical_scroll: usize,
    pub dirty: bool,
    pub version: i32,
    pub layout_cache: HashMap<usize, Vec<Line<'static>>>,
    pub terminal_width: u16,
    pub selection_start: Option<(usize, usize)>,
    pub undo_stack: Vec<EditorState>,
    pub redo_stack: Vec<EditorState>,
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            path: None,
            content: vec![String::from("Welcome to Clide!")],
            cursor_row: 0,
            cursor_col: 0,
            vertical_scroll: 0,
            dirty: false,
            version: 0,
            layout_cache: HashMap::new(),
            terminal_width: 0,
            selection_start: None,
            undo_stack: vec![EditorState { content: vec![String::from("Welcome to Clide!")], cursor_row: 0, cursor_col: 0 }],
            redo_stack: Vec::new(),
        }
    }

    pub fn set_content(&mut self, path: PathBuf, content: String) {
        self.content = content.lines().map(String::from).collect();
        self.path = Some(path);
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.vertical_scroll = 0;
        self.dirty = false;
        self.version = 1;
        self.layout_cache.clear();
        self.undo_stack = vec![self.current_state()];
        self.redo_stack.clear();
    }

    fn current_state(&self) -> EditorState {
        EditorState {
            content: self.content.clone(),
            cursor_row: self.cursor_row,
            cursor_col: self.cursor_col,
        }
    }

    pub fn push_undo_state(&mut self) {
        let last_state = self.undo_stack.last();
        let current_state = self.current_state();
        if last_state.is_none_or(|s| s.content != current_state.content) {
            self.undo_stack.push(current_state);
            self.redo_stack.clear();
        }
    }

    pub fn undo(&mut self) {
        if self.undo_stack.len() > 1 && let Some(state) = self.undo_stack.pop() {
            self.redo_stack.push(state);
            if let Some(last_state) = self.undo_stack.last() {
                self.restore_state(last_state.clone());
            }
        }
    }

    pub fn redo(&mut self) {
        if let Some(state) = self.redo_stack.pop() {
            self.undo_stack.push(state.clone());
            self.restore_state(state);
        }
    }

    fn restore_state(&mut self, state: EditorState) {
        self.content = state.content;
        self.cursor_row = state.cursor_row;
        self.cursor_col = state.cursor_col;
        self.layout_cache.clear();
        self.dirty = true;
    }

    pub fn move_cursor_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.clamp_cursor_col();
        }
    }

    pub fn move_cursor_down(&mut self) {
        if self.cursor_row < self.content.len() - 1 {
            self.cursor_row += 1;
            self.clamp_cursor_col();
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.content[self.cursor_row].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_col < self.content[self.cursor_row].len() {
            self.cursor_col += 1;
        } else if self.cursor_row < self.content.len() - 1 {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    fn clamp_cursor_col(&mut self) {
        let line_len = self.content.get(self.cursor_row).map_or(0, |line| line.len());
        self.cursor_col = min(self.cursor_col, line_len);
    }

    pub fn insert_text(&mut self, text: &str) {
        self.push_undo_state();
        let current_line = &mut self.content[self.cursor_row];
        current_line.insert_str(self.cursor_col, text);
        self.cursor_col += text.len();
        self.dirty = true;
        self.layout_cache.remove(&self.cursor_row);
    }

    pub fn insert_tab(&mut self) {
        self.push_undo_state();
        let current_line = &mut self.content[self.cursor_row];
        let tab = "    ";
        current_line.insert_str(self.cursor_col, tab);
        self.cursor_col += tab.len();
        self.dirty = true;
        self.layout_cache.remove(&self.cursor_row);
    }

    pub fn delete_char(&mut self) {
        self.push_undo_state();
        if self.cursor_col > 0 {
            let current_line = &mut self.content[self.cursor_row];
            current_line.remove(self.cursor_col - 1);
            self.cursor_col -= 1;
            self.dirty = true;
            self.layout_cache.remove(&self.cursor_row);
        } else if self.cursor_row > 0 {
            let prev_line = self.content.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.content[self.cursor_row].len();
            self.content[self.cursor_row].push_str(&prev_line);
            self.dirty = true;
            self.layout_cache.clear();
        }
    }

    pub fn delete_forward_char(&mut self) {
        self.push_undo_state();
        let line_len = self.content[self.cursor_row].len();
        if self.cursor_col < line_len {
            self.content[self.cursor_row].remove(self.cursor_col);
            self.dirty = true;
            self.layout_cache.remove(&self.cursor_row);
        } else if self.cursor_row < self.content.len() - 1 {
            let next_line = self.content.remove(self.cursor_row + 1);
            self.content[self.cursor_row].push_str(&next_line);
            self.dirty = true;
            self.layout_cache.clear();
        }
    }

    pub fn insert_newline(&mut self) {
        self.push_undo_state();
        let current_line = &mut self.content[self.cursor_row];
        let new_line_content = current_line.split_off(self.cursor_col);
        self.content.insert(self.cursor_row + 1, new_line_content);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.dirty = true;
        self.layout_cache.clear();
    }

    pub fn move_cursor_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.cursor_col = self.content[self.cursor_row].len();
    }

    pub fn move_cursor_page_up(&mut self, page_size: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(page_size);
        self.clamp_cursor_col();
    }

    pub fn move_cursor_page_down(&mut self, page_size: usize) {
        self.cursor_row = min(self.content.len() - 1, self.cursor_row + page_size);
        self.clamp_cursor_col();
    }

    pub fn move_cursor_to(&mut self, row: usize, col: usize) {
        self.cursor_row = min(row, self.content.len() - 1);
        self.cursor_col = col;
        self.clamp_cursor_col();
    }
}
