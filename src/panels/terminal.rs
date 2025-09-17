#[derive(Default)]
pub struct TerminalPane {
    lines: Vec<String>,
    scroll: isize,
}

impl TerminalPane {
    pub fn append_line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn scroll(&mut self, delta: isize) {
        if self.lines.is_empty() {
            self.scroll = 0;
            return;
        }
        let max_scroll = self.lines.len().saturating_sub(1) as isize;
        let new_scroll = (self.scroll + delta).clamp(0, max_scroll);
        self.scroll = new_scroll;
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll as usize
    }
}
