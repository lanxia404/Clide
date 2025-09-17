use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use walkdir::WalkDir;

use crate::editor::Editor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    FileTree,
    Editor,
    Terminal,
    Agent,
}

impl FocusArea {
    pub fn label(&self) -> &'static str {
        match self {
            FocusArea::FileTree => "File Tree",
            FocusArea::Editor => "Editor",
            FocusArea::Terminal => "Terminal",
            FocusArea::Agent => "Agent",
        }
    }
}

pub struct App {
    pub should_quit: bool,
    pub focus: FocusArea,
    pub editor: Editor,
    pub file_tree: FileTree,
    pub terminal: TerminalPane,
    pub agent: AgentPanel,
    pub status_message: String,
    pub workspace_root: PathBuf,
    last_tick: Instant,
    tick_rate: Duration,
}

impl App {
    pub fn new(workspace_root: PathBuf) -> Result<Self> {
        let canonical_root = workspace_root.canonicalize().unwrap_or(workspace_root);
        let mut file_tree = FileTree::from_root(canonical_root.clone());
        if file_tree.is_empty() {
            file_tree.populate_with_placeholder();
        }

        let editor = Editor::with_placeholder(
            "// Welcome to Clide - command line IDE prototype\n\
             // Left: tree | Center: editor + terminal | Right: agent\n\
             fn main() {\n    println!(\"Ready for takeoff!\");\n}\n",
        );

        let mut terminal = TerminalPane::default();
        terminal.append_line("> cargo run    // integrated terminal mock");
        terminal.append_line("build pending: feature coming soon");

        let agent = AgentPanel::with_placeholder();

        Ok(Self {
            should_quit: false,
            focus: FocusArea::Editor,
            editor,
            file_tree,
            terminal,
            agent,
            status_message: String::from("Ctrl+Q exit, Tab cycle panes, type to edit"),
            workspace_root: canonical_root,
            last_tick: Instant::now(),
            tick_rate: Duration::from_millis(250),
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('q'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            (KeyCode::Tab, _) => {
                self.cycle_focus(1);
                return;
            }
            (KeyCode::BackTab, _) => {
                self.cycle_focus(-1);
                return;
            }
            _ => {}
        }

        match self.focus {
            FocusArea::Editor => self.handle_editor_key(key),
            FocusArea::FileTree => self.handle_file_tree_key(key),
            FocusArea::Terminal => self.handle_terminal_key(key),
            FocusArea::Agent => self.handle_agent_key(key),
        }
    }

    pub fn on_tick(&mut self) {
        if self.last_tick.elapsed() >= self.tick_rate {
            self.last_tick = Instant::now();
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(ch) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                match ch {
                    's' => self.status_message = "Save is not implemented".into(),
                    'a' => self.status_message = "Agent request is not implemented".into(),
                    _ => {}
                }
            }
            KeyCode::Char(ch) => {
                self.editor.insert_char(ch);
            }
            KeyCode::Enter => self.editor.insert_newline(),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Left => self.editor.move_cursor(0, -1),
            KeyCode::Right => self.editor.move_cursor(0, 1),
            KeyCode::Up => self.editor.move_cursor(-1, 0),
            KeyCode::Down => self.editor.move_cursor(1, 0),
            KeyCode::Home => self.editor.move_to_line_start(),
            KeyCode::End => self.editor.move_to_line_end(),
            KeyCode::PageUp => self.editor.move_cursor(-10, 0),
            KeyCode::PageDown => self.editor.move_cursor(10, 0),
            _ => {}
        }
    }

    fn handle_file_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.file_tree.move_selection(-1),
            KeyCode::Down => self.file_tree.move_selection(1),
            KeyCode::Enter => {
                if let Some(entry) = self.file_tree.selected_entry() {
                    if entry.is_dir {
                        self.status_message = format!("Directory: {}", entry.path.display());
                    } else {
                        match self.editor.open_file(&entry.path) {
                            Ok(_) => {
                                self.status_message = format!("Opened file: {}", entry.path.display());
                            }
                            Err(err) => {
                                self.status_message = format!("Open failed: {err}");
                            }
                        }
                    }
                }
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.file_tree.refresh();
                self.status_message = String::from("Tree refreshed");
            }
            _ => {}
        }
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.terminal.scroll(-1),
            KeyCode::Down => self.terminal.scroll(1),
            KeyCode::PageUp => self.terminal.scroll(-5),
            KeyCode::PageDown => self.terminal.scroll(5),
            _ => {}
        }
    }

    fn handle_agent_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.agent.move_selection(-1),
            KeyCode::Down => self.agent.move_selection(1),
            KeyCode::Enter => {
                if let Some(selected) = self.agent.selected_message() {
                    self.status_message = format!("Agent suggestion pending: {}", selected.title);
                }
            }
            _ => {}
        }
    }

    fn cycle_focus(&mut self, direction: isize) {
        let order = [
            FocusArea::FileTree,
            FocusArea::Editor,
            FocusArea::Terminal,
            FocusArea::Agent,
        ];
        let current_index = order
            .iter()
            .position(|area| *area == self.focus)
            .unwrap_or(1);
        let len = order.len() as isize;
        let mut new_index = current_index as isize + direction;
        if new_index < 0 {
            new_index += len;
        }
        new_index %= len;
        self.focus = order[new_index as usize];
        self.status_message = format!("Focus: {}", self.focus.label());
    }
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub depth: usize,
    pub is_dir: bool,
}

pub struct FileTree {
    entries: Vec<FileEntry>,
    selected: usize,
    root: PathBuf,
}

impl FileTree {
    pub fn from_root(root: PathBuf) -> Self {
        let mut tree = Self {
            entries: Vec::new(),
            selected: 0,
            root,
        };
        tree.refresh();
        tree
    }

    pub fn refresh(&mut self) {
        let mut entries = Vec::new();
        for entry in WalkDir::new(&self.root)
            .max_depth(3)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.path() == self.root {
                continue;
            }
            let depth = entry.depth().saturating_sub(1);
            let is_hidden = entry
                .file_name()
                .to_str()
                .map(|name| name.starts_with('.'))
                .unwrap_or(false);
            if is_hidden {
                continue;
            }
            let file_type = entry.file_type();
            entries.push(FileEntry {
                name: entry
                    .file_name()
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
                path: entry.path().to_path_buf(),
                depth,
                is_dir: file_type.is_dir(),
            });
        }
        if entries.is_empty() {
            entries.push(FileEntry {
                name: String::from("(empty)"),
                path: self.root.clone(),
                depth: 0,
                is_dir: true,
            });
        }
        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn populate_with_placeholder(&mut self) {
        self.entries = vec![FileEntry {
            name: String::from("src/main.rs"),
            path: self.root.join("src").join("main.rs"),
            depth: 1,
            is_dir: false,
        }];
        self.selected = 0;
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        let mut new_index = self.selected as isize + delta;
        if new_index < 0 {
            new_index = 0;
        }
        if new_index >= len {
            new_index = len - 1;
        }
        self.selected = new_index as usize;
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

}

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

pub struct AgentMessage {
    pub title: String,
    pub detail: String,
}

pub struct AgentPanel {
    messages: Vec<AgentMessage>,
    selected: usize,
}

impl AgentPanel {
    pub fn with_placeholder() -> Self {
        let messages = vec![
            AgentMessage {
                title: String::from("Agent status"),
                detail: String::from("Awaiting AI edits"),
            },
            AgentMessage {
                title: String::from("Change preview"),
                detail: String::from("main.rs line 10: add println!"),
            },
        ];
        Self {
            messages,
            selected: 0,
        }
    }

    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.messages.is_empty() {
            return;
        }
        let len = self.messages.len() as isize;
        let mut new_index = self.selected as isize + delta;
        if new_index < 0 {
            new_index = 0;
        }
        if new_index >= len {
            new_index = len - 1;
        }
        self.selected = new_index as usize;
    }

    pub fn selected_message(&self) -> Option<&AgentMessage> {
        self.messages.get(self.selected)
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }
}
