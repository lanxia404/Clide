use crate::editor::Editor;
use crate::event::Event;
use crate::file_tree::FileTree;
use crate::git::GitState;
use crate::i18n::{English, SimplifiedChinese, TraditionalChinese, Language};
use crate::lsp::{LspClient, LspMessage};
use crate::plugin::PluginManager;
use crate::syntax::SyntaxHighlighter;
use crate::terminal::TerminalState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use lsp_types::Diagnostic;
use ratatui::layout::Rect;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use url::Url;

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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ActivePanel {
    Terminal,
    Git,
}

pub struct App {
    pub running: bool,
    lang_state: CurrentLanguage,
    pub lang: Box<dyn Language>,
    pub file_tree: FileTree,
    pub editor: Editor,
    pub syntax_highlighter: SyntaxHighlighter,
    pub lsp_client: LspClient,
    pub lsp_receiver: UnboundedReceiver<LspMessage>,
    pub diagnostics: HashMap<Url, Vec<Diagnostic>>,
    pub icon_set: IconSet,
    pub focus: Focus,
    pub file_tree_area: Rect,
    pub editor_area: Rect,
    last_click: Option<(Instant, MouseEvent)>,
    drag_start_row: Option<u16>,
    last_cursor_move: Instant,
    pub active_panel: Option<ActivePanel>,
    pub terminal: TerminalState,
    pub git: GitState,
    pub plugin_manager: PluginManager,
    pub completion_list: Option<Vec<lsp_types::CompletionItem>>,
    pub hover_info: Option<lsp_types::Hover>,
    git_state_receiver: mpsc::Receiver<GitState>,
    git_state_sender: mpsc::Sender<GitState>,
}

impl App {
    pub fn new() -> Result<Self> {
        let initial_path = env::current_dir().unwrap_or_else(|_| {
            env::home_dir().unwrap_or_else(|| PathBuf::from("."))
        });

        let (lsp_client, lsp_receiver) = LspClient::new()?;
        lsp_client.initialize(initial_path.as_path())?;

        let icon_set = match env::var("CLIDE_ICONS") {
            Ok(val) if val.to_lowercase() == "nerd" => IconSet::NerdFont,
            _ => IconSet::Unicode,
        };

        let (git_state_sender, git_state_receiver) = mpsc::channel(1);

        let mut app = Self {
            running: true,
            lang_state: CurrentLanguage::TraditionalChinese,
            lang: Box::new(TraditionalChinese),
            file_tree: FileTree::new(&initial_path)?,
            editor: Editor::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            lsp_client,
            lsp_receiver,
            diagnostics: HashMap::new(),
            icon_set,
            focus: Focus::FileTree,
            file_tree_area: Rect::default(),
            editor_area: Rect::default(),
            last_click: None,
            drag_start_row: None,
            last_cursor_move: Instant::now(),
            active_panel: None,
            terminal: TerminalState::new(),
            git: GitState::new(),
            plugin_manager: PluginManager::new(),
            completion_list: None,
            hover_info: None,
            git_state_receiver,
            git_state_sender,
        };

        app.plugin_manager.load_plugins();
        Ok(app)
    }

    pub fn tick(&mut self) {
        self.plugin_manager.tick_plugins();

        if self.focus == Focus::Editor && self.hover_info.is_none() && self.last_cursor_move.elapsed() > Duration::from_millis(500) {
            if let Some(path) = &self.editor.path {
                self.lsp_client.hover(path, self.editor.cursor_row as u32, self.editor.cursor_col as u32).unwrap_or(());
                self.last_cursor_move = Instant::now();
            }
        }

        if let Ok(new_git_state) = self.git_state_receiver.try_recv() {
            self.git = new_git_state;
        }
        self.terminal.poll_output();
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::Key(key_event) => self.handle_key_event(key_event),
            Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        // ... mouse event handling
    }

    fn handle_file_tree_action(&mut self, is_enter_press: bool) {
        // ... file tree action handling
    }

    fn is_mouse_over_area(&self, event: &MouseEvent, area: Rect) -> bool {
        area.intersects(Rect {
            x: event.column,
            y: event.row,
            width: 1,
            height: 1,
        })
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let global_handled = match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => { self.running = false; true },
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => { self.toggle_language(); true },
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => { self.save_file(); true },
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => { self.toggle_focus(); true },
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => { self.toggle_panel(ActivePanel::Terminal); true },
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => { self.toggle_panel(ActivePanel::Git); true },
            _ => false,
        };

        if global_handled { return; }

        if let Some(active_panel) = self.active_panel {
            match active_panel {
                ActivePanel::Terminal => { self.terminal.handle_key_event(key_event); return; },
                ActivePanel::Git => {}
            }
        }

        match self.focus {
            Focus::FileTree => self.handle_file_tree_keys(key_event),
            Focus::Editor => self.handle_editor_keys(key_event),
        }
    }

    fn handle_file_tree_keys(&mut self, key_event: KeyEvent) {
        // ... file tree key handling
    }

    fn handle_editor_keys(&mut self, key_event: KeyEvent) {
        // ... editor key handling
    }

    pub fn open_file(&mut self, path: PathBuf) {
        // ... open file logic
    }

    fn save_file(&mut self) {
        // ... save file logic
    }

    fn toggle_focus(&mut self) {
        // ... toggle focus logic
    }

    fn toggle_panel(&mut self, panel: ActivePanel) {
        let is_opening = self.active_panel != Some(panel);
        if is_opening {
            self.active_panel = Some(panel);
            if panel == ActivePanel::Git {
                let tx = self.git_state_sender.clone();
                let mut git_state = self.git.clone();
                tokio::spawn(async move {
                    git_state.update().await;
                    let _ = tx.send(git_state).await;
                });
            }
        } else {
            self.active_panel = None;
        }
    }

    fn toggle_language(&mut self) {
        // ... toggle language logic
    }

    pub fn clear_editor_cache(&mut self) {
        // ... clear editor cache logic
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}