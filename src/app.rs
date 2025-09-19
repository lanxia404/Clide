use crate::editor::Editor;
use crate::event::Event;
use crate::file_tree::FileTree;
use crate::i18n::{English, SimplifiedChinese, TraditionalChinese, Language};
use crate::lsp::LspClient;
use crate::syntax::SyntaxHighlighter;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::layout::Rect;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ... (CurrentLanguage and IconSet enums are unchanged)
pub enum IconSet {
    Unicode,
    NerdFont,
}
#[derive(Debug, PartialEq, Eq)]
enum CurrentLanguage {
    English,
    SimplifiedChinese,
    TraditionalChinese,
}

impl CurrentLanguage {
    fn next(&self) -> Self {
        match self {
            Self::English => Self::SimplifiedChinese,
            Self::SimplifiedChinese => Self::TraditionalChinese,
            Self::TraditionalChinese => Self::English,
        }
    }
}

#[derive(PartialEq, Eq)]
pub enum Focus {
    FileTree,
    Editor,
}

pub struct App {
    pub running: bool,
    lang_state: CurrentLanguage,
    pub lang: Box<dyn Language>,
    pub file_tree: FileTree,
    pub editor: Editor,
    pub syntax_highlighter: SyntaxHighlighter,
    pub lsp_client: Option<LspClient>,
    pub icon_set: IconSet,
    pub focus: Focus,
    // UI areas
    pub file_tree_area: Rect,
    pub editor_area: Rect,
    // For double click detection
    last_click: Option<(Instant, MouseEvent)>,
}

impl App {
    pub fn new() -> Result<Self> {
        let initial_path = Path::new(".");
        let file_tree = FileTree::new(initial_path)?;
        let lsp_client = LspClient::new().ok();
        if let Some(client) = &lsp_client {
            client.initialize(initial_path)?;
        }

        let icon_set = match env::var("CLIDE_ICONS") {
            Ok(val) if val.to_lowercase() == "nerd" => IconSet::NerdFont,
            _ => IconSet::Unicode,
        };

        Ok(Self {
            running: true,
            lang_state: CurrentLanguage::TraditionalChinese,
            lang: Box::new(TraditionalChinese),
            file_tree,
            editor: Editor::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            lsp_client,
            icon_set,
            focus: Focus::FileTree,
            file_tree_area: Rect::default(),
            editor_area: Rect::default(),
            last_click: None,
        })
    }

    pub fn tick(&mut self) {}

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event),
            Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        // Double click logic
        let is_double_click = if let Some((last_time, last_event)) = self.last_click {
            let now = Instant::now();
            now.duration_since(last_time) < Duration::from_millis(300) &&
            event.column == last_event.column && event.row == last_event.row
        } else {
            false
        };

        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            if is_double_click {
                self.handle_double_click(event);
                self.last_click = None; // Reset after double click
            } else {
                self.last_click = Some((Instant::now(), event));
            }
        }

        if self.file_tree_area.intersects(Rect {
            x: event.column,
            y: event.row,
            width: 1,
            height: 1,
        }) {
            self.file_tree.handle_mouse_event(event);
        }
    }

    fn handle_double_click(&mut self, event: MouseEvent) {
        if self.file_tree_area.intersects(Rect { x: event.column, y: event.row, width: 1, height: 1 }) {
            let path = self.file_tree.get_selected_path();
            if path.is_file() {
                self.open_file(path);
            } else {
                self.file_tree.toggle_expansion();
            }
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        // Global keybindings
        if key_event.code == KeyCode::Tab {
            self.toggle_focus();
            return;
        }
        if key_event.code == KeyCode::Char('l') && key_event.modifiers == KeyModifiers::CONTROL {
            self.toggle_language();
            return;
        }
        if key_event.code == KeyCode::Char('q') && key_event.modifiers == KeyModifiers::CONTROL {
            self.quit();
            return;
        }

        match self.focus {
            Focus::FileTree => self.handle_file_tree_keys(key_event),
            Focus::Editor => self.handle_editor_keys(key_event),
        }
    }

    fn handle_file_tree_keys(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Enter => self.handle_enter_on_file_tree(),
            _ => self.file_tree.handle_key_event(key_event),
        }
    }

    fn handle_editor_keys(&mut self, _key_event: KeyEvent) {
        // TODO: Pass keys to editor
    }

    fn handle_enter_on_file_tree(&mut self) {
        let path = self.file_tree.get_selected_path();
        if path.ends_with("..") {
            if let Some(parent) = path.parent().and_then(|p| p.parent()) {
                self.change_root_directory(parent);
            }
            return;
        }
        if path.is_dir() {
            self.change_root_directory(&path);
        } else {
            self.open_file(path);
        }
    }

    fn change_root_directory(&mut self, new_root: &Path) {
        if let Ok(new_file_tree) = FileTree::new(new_root) {
            self.file_tree = new_file_tree;
        }
    }

    fn open_file(&mut self, path: PathBuf) {
        if self.editor.open_file(path.clone()).is_ok() {
            if let Some(client) = &self.lsp_client {
                client.did_open(&path).unwrap_or(());
            }
            self.focus = Focus::Editor;
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::FileTree => Focus::Editor,
            Focus::Editor => Focus::FileTree,
        }
    }

    fn toggle_language(&mut self) {
        self.lang_state = self.lang_state.next();
        self.lang = match self.lang_state {
            CurrentLanguage::English => Box::new(English),
            CurrentLanguage::SimplifiedChinese => Box::new(SimplifiedChinese),
            CurrentLanguage::TraditionalChinese => Box::new(TraditionalChinese),
        };
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}
