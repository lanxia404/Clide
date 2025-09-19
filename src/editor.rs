
use std::path::PathBuf;

pub struct Editor {
    pub path: Option<PathBuf>,
    pub content: Vec<String>,
    pub cursor_y: usize,
    pub cursor_x: usize,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            path: None,
            content: vec![String::from("~ Welcome to Clide ~")],
            cursor_y: 0,
            cursor_x: 0,
        }
    }

    pub fn open_file(&mut self, path: PathBuf) -> std::io::Result<()> {
        let content = std::fs::read_to_string(&path)?;
        self.content = content.lines().map(String::from).collect();
        self.path = Some(path);
        self.cursor_y = 0;
        self.cursor_x = 0;
        Ok(())
    }
}

