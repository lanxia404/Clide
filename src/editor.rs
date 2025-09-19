use std::cmp::min;
use std::fs;
use std::path::PathBuf;

pub struct Editor {
    pub path: Option<PathBuf>,
    pub content: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub vertical_scroll: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            path: None,
            content: vec![String::from("Welcome to Clide!")],
            cursor_row: 0,
            cursor_col: 0,
            vertical_scroll: 0,
        }
    }

    pub fn open_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let content_str = fs::read_to_string(&path)?;
        self.content = content_str.lines().map(String::from).collect();
        self.path = Some(path);
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.vertical_scroll = 0;
        Ok(())
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
            // Move to the end of the previous line
            self.cursor_row -= 1;
            self.cursor_col = self.content[self.cursor_row].len();
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_col < self.content[self.cursor_row].len() {
            self.cursor_col += 1;
        } else if self.cursor_row < self.content.len() - 1 {
            // Move to the start of the next line
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    // Ensures the cursor column is not beyond the end of the current line
    fn clamp_cursor_col(&mut self) {
        let line_len = self.content[self.cursor_row].len();
        self.cursor_col = min(self.cursor_col, line_len);
    }

    // Adjusts the vertical scroll to keep the cursor in view
    pub fn scroll(&mut self, view_height: usize) {
        if self.cursor_row < self.vertical_scroll {
            self.vertical_scroll = self.cursor_row;
        } else if self.cursor_row >= self.vertical_scroll + view_height {
            self.vertical_scroll = self.cursor_row - view_height + 1;
        }
    }
}
