use std::fs;
use std::path::PathBuf;

pub struct Editor {
    pub path: Option<PathBuf>,
    pub content: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            path: None,
            content: vec![String::from("Welcome to Clide!")],
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    pub fn open_file(&mut self, path: PathBuf) -> Result<(), std::io::Error> {
        let content_str = fs::read_to_string(&path)?;
        self.content = content_str.lines().map(String::from).collect();
        self.path = Some(path);
        self.cursor_row = 0;
        self.cursor_col = 0;
        Ok(())
    }
}