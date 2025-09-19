
use crate::editor::Editor;
use crate::event::Event;
use crate::file_tree::FileTree;
use crate::i18n::{English, SimplifiedChinese, TraditionalChinese, Language};
use crate::lsp::LspClient;
use crate::syntax::SyntaxHighlighter;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::Path;

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

pub struct App {
    pub running: bool,
    lang_state: CurrentLanguage,
    pub lang: Box<dyn Language>,
    pub file_tree: FileTree,
    pub editor: Editor,
    pub syntax_highlighter: SyntaxHighlighter,
    pub lsp_client: Option<LspClient>,
}

impl App {
    pub fn new() -> Result<Self> {
        let initial_path = Path::new(".");
        let file_tree = FileTree::new(initial_path)?;
        let lsp_client = LspClient::new().ok();
        if let Some(client) = &lsp_client {
            client.initialize(initial_path)?;
        }

        Ok(Self {
            running: true,
            lang_state: CurrentLanguage::English,
            lang: Box::new(English),
            file_tree,
            editor: Editor::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            lsp_client,
        })
    }

    pub fn tick(&mut self) {}

    pub fn handle_event(&mut self, event: Event) {
        if let Event::Key(key_event) = event {
            self.handle_key_event(key_event);
        }
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            // App control
            KeyCode::Char('q') => self.quit(),
            KeyCode::Char('c') if key_event.modifiers == KeyModifiers::CONTROL => self.quit(),
            KeyCode::Char('l') => self.toggle_language(),
            KeyCode::Enter => self.open_selected_file(),
            
            // Pass to file tree
            KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right => {
                self.file_tree.handle_key_event(key_event);
            }
            _ => {}
        }
    }
    
    fn open_selected_file(&mut self) {
        let path = self.file_tree.get_selected_path();
        if path.is_file() {
            if self.editor.open_file(path.clone()).is_ok() {
                if let Some(client) = &self.lsp_client {
                    client.did_open(&path).unwrap_or(());
                }
            }
        } else {
            self.file_tree.toggle_expansion();
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

