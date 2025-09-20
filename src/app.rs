// src/app.rs

use crate::components::editor::Editor;
use crate::components::file_tree::FileTree;
use crate::core::i18n::{English, Language, SimplifiedChinese, TraditionalChinese};
use crate::core::lsp::{LspClient, LspMessage, LspNotification, LspResponse};
use crate::core::syntax::SyntaxHighlighter;
use crate::event::Event;
use crate::features::git::GitState;
use crate::features::plugin::PluginManager;
use crate::features::terminal::TerminalState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use lsp_types::{Diagnostic, InitializeParams, WorkspaceFolder};
use ratatui::layout::Rect;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::
{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use url::Url;

// ... (Enums remain the same)
pub enum IconSet {
    Unicode,
    NerdFont,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
enum CurrentLanguage {
    English,
    SimplifiedChinese,
    TraditionalChinese,
}

impl CurrentLanguage {
    #[allow(dead_code)]
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

pub enum LspStatus {
    Starting,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Vertical,
    // Horizontal, // For future implementation
}

#[derive(Debug, Clone, Copy)]
pub struct DraggingState {
    pub kind: DragKind,
    pub start_mouse_col: u16,
    pub start_percent: u16,
}

#[allow(dead_code)]
pub struct App {
    pub running: bool,
    lang_state: CurrentLanguage,
    pub lang: Box<dyn Language>,
    pub file_tree: FileTree,
    pub editor: Editor,
    pub syntax_highlighter: SyntaxHighlighter,
    pub lsp_client: LspClient,
    pub lsp_receiver: UnboundedReceiver<LspMessage>,
    lsp_server: Option<Child>,
    pub lsp_status: LspStatus,
    pub diagnostics: HashMap<Url, Vec<Diagnostic>>,
    pub icon_set: IconSet,
    pub focus: Focus,
    pub file_tree_area: Rect,
    pub editor_area: Rect,
    last_click: Option<(Instant, MouseEvent)>,
    last_cursor_move: Instant,
    pub active_panel: Option<ActivePanel>,
    pub terminal: TerminalState,
    pub git: GitState,
    pub plugin_manager: PluginManager,
    pub completion_list: Option<Vec<lsp_types::CompletionItem>>,
    pub completion_selection: Option<usize>,
    pub hover_info: Option<lsp_types::Hover>,
    git_state_receiver: mpsc::Receiver<GitState>,
    git_state_sender: mpsc::Sender<GitState>,
    pub lsp_message: Option<String>,
    pub file_tree_width_percent: u16,
    pub dragging: Option<DraggingState>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (lsp_writer_tx, lsp_writer_rx) = mpsc::unbounded_channel();
        let (lsp_event_tx, lsp_event_rx) = mpsc::unbounded_channel();

        let lsp_client = LspClient::new(lsp_writer_tx.clone());

        let (git_state_sender, git_state_receiver) = mpsc::channel(1);

        let mut app = Self {
            running: true,
            lang_state: CurrentLanguage::TraditionalChinese,
            lang: Box::new(TraditionalChinese),
            file_tree: FileTree::new(&env::current_dir()?)?,
            editor: Editor::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            lsp_client,
            lsp_receiver: lsp_event_rx,
            lsp_server: None,
            lsp_status: LspStatus::Starting,
            diagnostics: HashMap::new(),
            icon_set: IconSet::Unicode,
            focus: Focus::FileTree,
            file_tree_area: Rect::default(),
            editor_area: Rect::default(),
            last_click: None,
            last_cursor_move: Instant::now(),
            active_panel: None,
            terminal: TerminalState::new(),
            git: GitState::new(),
            plugin_manager: PluginManager::new(),
            completion_list: None,
            completion_selection: None,
            hover_info: None,
            git_state_receiver,
            git_state_sender,
            lsp_message: None,
            file_tree_width_percent: 25,
            dragging: None,
        };

        app.start_lsp_server(lsp_writer_tx, lsp_writer_rx, lsp_event_tx);
        app.plugin_manager.load_plugins();
        Ok(app)
    }

    fn start_lsp_server(
        &mut self,
        lsp_writer_tx: UnboundedSender<Value>,
        lsp_writer_rx: UnboundedReceiver<Value>,
        lsp_event_tx: UnboundedSender<LspMessage>,
    ) {
        let root_path = env::current_dir().unwrap();

        tokio::spawn(async move {
            let server_process = Command::new("rust-analyzer")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            if let Ok(mut server) = server_process {
                let stdin = server.stdin.take().unwrap();
                let stdout = server.stdout.take().unwrap();
                let stderr = server.stderr.take().unwrap();

                tokio::spawn(writer_task(stdin, lsp_writer_rx));
                tokio::spawn(reader_task(stdout, lsp_event_tx.clone()));
                tokio::spawn(stderr_task(stderr, lsp_event_tx.clone()));

                let root_uri_url = Url::from_directory_path(root_path).unwrap();
                let root_uri: lsp_types::Uri = root_uri_url.as_str().parse().unwrap();
                let params = InitializeParams {
                    process_id: Some(std::process::id()),
                    workspace_folders: Some(vec![WorkspaceFolder {
                        uri: root_uri,
                        name: "Clide".to_string(),
                    }]),
                    capabilities: lsp_types::ClientCapabilities::default(),
                    ..Default::default()
                };
                let request = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "initialize",
                    "params": params,
                });
                let _ = lsp_writer_tx.send(request);

            // self.lsp_server = Some(server); // Cannot borrow self in async move block
            } else {
                let _ = lsp_event_tx.send(LspMessage::Error(
                    1,
                    serde_json::json!({"message": "Failed to start rust-analyzer"}),
                ));
            }
        });
    }

    pub fn tick(&mut self) {
        self.plugin_manager.tick_plugins();

        if self.focus == Focus::Editor
            && self.hover_info.is_none()
            && self.last_cursor_move.elapsed() > Duration::from_millis(500)
            && let Some(path) = &self.editor.path
        {
            self.lsp_client
                .hover(
                    path,
                    self.editor.cursor_row as u32,
                    self.editor.cursor_col as u32,
                )
                .unwrap_or(());
            self.last_cursor_move = Instant::now();
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
        // --- DRAGGING LOGIC ---
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                // Check if the click is on the vertical border between file tree and editor
                let border_x = self.file_tree_area.right();
                if self.file_tree_area.width > 0 && event.column == border_x {
                    self.dragging = Some(DraggingState {
                        kind: DragKind::Vertical,
                        start_mouse_col: event.column,
                        start_percent: self.file_tree_width_percent,
                    });
                    return; // Consume the event
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(drag_state) = self.dragging {
                    let total_width = self.file_tree_area.width + self.editor_area.width;
                    if total_width > 0 {
                        let delta_x = event.column as i16 - drag_state.start_mouse_col as i16;
                        let delta_percent = (delta_x as f32 * 100.0 / total_width as f32).round() as i16;
                        let new_percent = (drag_state.start_percent as i16 + delta_percent) as u16;
                        self.file_tree_width_percent = new_percent.clamp(10, 90);
                    }
                    return; // Consume the event
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.dragging.is_some() {
                    self.dragging = None;
                    return; // Consume the event
                }
            }
            _ => {}
        }

        // --- REGULAR MOUSE LOGIC ---
        let is_double_click = if let Some((last_time, last_event)) = self.last_click {
            let now = Instant::now();
            now.duration_since(last_time) < Duration::from_millis(300)
                && event.column == last_event.column
                && event.row == last_event.row
        } else {
            false
        };

        if self.is_mouse_over_area(&event, self.file_tree_area) {
            self.focus = Focus::FileTree;
            let relative_row = event.row.saturating_sub(self.file_tree_area.y).saturating_sub(1);
            self.file_tree.select_by_index(relative_row as usize);

            if event.kind == MouseEventKind::Down(MouseButton::Left) {
                if is_double_click {
                    let selected_path = self.file_tree.get_selected_path();
                    if selected_path.is_dir() {
                        self.file_tree.toggle_expansion();
                    } else {
                        let _ = self.open_file(selected_path);
                    }
                    self.last_click = None;
                } else {
                    self.last_click = Some((Instant::now(), event));
                }
            }
        } else if self.is_mouse_over_area(&event, self.editor_area) {
            self.focus = Focus::Editor;
            if event.kind == MouseEventKind::Down(MouseButton::Left) {
                let line_number_width = self.editor.content.len().to_string().len();
                let col = event
                    .column
                    .saturating_sub(self.editor_area.x + line_number_width as u16 + 3);
                let row = event.row.saturating_sub(self.editor_area.y + 1)
                    + self.editor.vertical_scroll as u16;
                self.editor.move_cursor_to(row as usize, col as usize);
            }
        }

        match self.focus {
            Focus::FileTree => {
                if event.kind == MouseEventKind::ScrollUp {
                    self.file_tree.select_previous();
                } else if event.kind == MouseEventKind::ScrollDown {
                    self.file_tree.select_next();
                }
            }
            Focus::Editor => {
                if event.kind == MouseEventKind::ScrollUp {
                    self.editor.move_cursor_up();
                } else if event.kind == MouseEventKind::ScrollDown {
                    self.editor.move_cursor_down();
                }
            }
        }
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
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => {
                self.running = false;
                true
            }
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                self.toggle_language();
                true
            }
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => {
                self.save_file();
                true
            }
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
                self.toggle_focus();
                true
            }
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => {
                self.toggle_panel(ActivePanel::Terminal);
                true
            }
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.toggle_panel(ActivePanel::Git);
                true
            }
            _ => false,
        };

        if global_handled {
            return;
        }

        if let Some(active_panel) = self.active_panel {
            match active_panel {
                ActivePanel::Terminal => {
                    self.terminal.handle_key_event(key_event);
                    return;
                }
                ActivePanel::Git => {}
            }
        }

        match self.focus {
            Focus::FileTree => self.handle_file_tree_keys(key_event),
            Focus::Editor => self.handle_editor_keys(key_event),
        }
    }

    fn handle_file_tree_keys(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => self.file_tree.select_previous(),
            KeyCode::Down => self.file_tree.select_next(),
            KeyCode::Left => self.file_tree.collapse_selected(),
            KeyCode::Right => self.file_tree.expand_selected(),
            KeyCode::Enter => {
                let selected_path = self.file_tree.get_selected_path();
                if selected_path.is_dir() {
                    self.file_tree.toggle_expansion();
                } else {
                    let _ = self.open_file(selected_path);
                }
            }
            _ => {}
        }
    }

    fn handle_editor_keys(&mut self, key_event: KeyEvent) {
        if let Some(completion_list) = &self.completion_list {
            match key_event.code {
                KeyCode::Up => {
                    let selection = self.completion_selection.unwrap_or(0);
                    if selection > 0 {
                        self.completion_selection = Some(selection - 1);
                    }
                }
                KeyCode::Down => {
                    let selection = self.completion_selection.unwrap_or(0);
                    if selection < completion_list.len() - 1 {
                        self.completion_selection = Some(selection + 1);
                    }
                }
                KeyCode::Enter | KeyCode::Tab => {
                    if let Some(selection) = self.completion_selection {
                        let item = &completion_list[selection];
                        self.editor.insert_text(&item.label);
                    }
                    self.completion_list = None;
                    self.completion_selection = None;
                }
                KeyCode::Esc => {
                    self.completion_list = None;
                    self.completion_selection = None;
                }
                _ => self.handle_standard_editor_keys(key_event),
            }
        } else {
            self.handle_standard_editor_keys(key_event);
        }
    }

    fn handle_standard_editor_keys(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => self.editor.move_cursor_up(),
            KeyCode::Down => self.editor.move_cursor_down(),
            KeyCode::Left => self.editor.move_cursor_left(),
            KeyCode::Right => self.editor.move_cursor_right(),
            KeyCode::Char(c) => {
                self.editor.insert_text(&c.to_string());
                if c == '.' || c == ':' {
                    if let Some(path) = &self.editor.path {
                        self.lsp_client
                            .completion(
                                path,
                                self.editor.cursor_row as u32,
                                self.editor.cursor_col as u32,
                            )
                            .unwrap_or(());
                    }
                }
            }
            KeyCode::Backspace => self.editor.delete_char(),
            KeyCode::Enter => self.editor.insert_newline(),
            KeyCode::Tab => self.editor.insert_tab(),
            KeyCode::Delete => self.editor.delete_forward_char(),
            KeyCode::Home => self.editor.move_cursor_home(),
            KeyCode::End => self.editor.move_cursor_end(),
            KeyCode::PageUp => self.editor.move_cursor_page_up(self.editor_area.height as usize),
            KeyCode::PageDown => self.editor.move_cursor_page_down(self.editor_area.height as usize),
            _ => {}
        }
        self.last_cursor_move = Instant::now();
        self.hover_info = None;
    }

    pub fn open_file(&mut self, path: PathBuf) -> anyhow::Result<()> {
        let content = fs::read_to_string(&path)?;
        let file_tree = FileTree::new(&self.file_tree.root.path)?;
        // The selection logic will need to be smarter if we want to preserve selection
        // For now, creating a new tree is enough to show the context.
        self.file_tree = file_tree;
        self.editor.set_content(path, content);
        self.focus = Focus::Editor;
        Ok(())
    }

    fn save_file(&mut self) {
        if let Some(path) = &self.editor.path {
            let content = self.editor.content.join("\n");
            if std::fs::write(path, content).is_ok() {
                self.editor.dirty = false;
            } else {
                // In a real app, this should show a message to the user in the UI
                eprintln!("Failed to save file!");
            }
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = match &self.focus {
            Focus::FileTree => Focus::Editor,
            Focus::Editor => Focus::FileTree,
        };
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
        self.lang_state = self.lang_state.next();
        self.lang = match self.lang_state {
            CurrentLanguage::English => Box::new(English),
            CurrentLanguage::SimplifiedChinese => Box::new(SimplifiedChinese),
            CurrentLanguage::TraditionalChinese => Box::new(TraditionalChinese),
        };
    }

    pub fn clear_editor_cache(&mut self) {
        self.editor.layout_cache.clear();
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}

async fn writer_task(mut stdin: tokio::process::ChildStdin, mut rx: UnboundedReceiver<Value>) {
    while let Some(msg) = rx.recv().await {
        let msg_str = msg.to_string();
        let content = format!("Content-Length: {}\r\n\r\n{}", msg_str.len(), msg_str);
        if stdin.write_all(content.as_bytes()).await.is_err() {
            break;
        }
    }
}

async fn reader_task<T: tokio::io::AsyncRead + Unpin>(
    stream: T,
    msg_tx: UnboundedSender<LspMessage>,
) {
    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();
    let mut content_length: Option<usize> = None;

    loop {
        buffer.clear();
        if let Ok(bytes_read) = reader.read_line(&mut buffer).await {
            if bytes_read == 0 {
                break;
            }
            if buffer.starts_with("Content-Length:") {
                if let Some(len_str) = buffer.trim().split(':').nth(1) {
                    content_length = len_str.trim().parse::<usize>().ok();
                }
            }
            if buffer.trim().is_empty() {
                if let Some(length) = content_length {
                    let mut body_buffer = vec![0; length];
                    if reader.read_exact(&mut body_buffer).await.is_ok() {
                        if let Ok(notification) =
                            serde_json::from_slice::<LspNotification>(&body_buffer)
                        {
                            let _ = msg_tx.send(LspMessage::Notification(
                                notification.method,
                                notification.params,
                            ));
                        } else if let Ok(response) =
                            serde_json::from_slice::<LspResponse>(&body_buffer)
                        {
                            match response {
                                LspResponse::Success { id, result } => {
                                    let _ = msg_tx.send(LspMessage::Response(id, result));
                                }
                                LspResponse::Error { id, error } => {
                                    let _ = msg_tx.send(LspMessage::Error(id, error));
                                }
                            }
                        }
                    }
                    content_length = None;
                }
            }
        } else {
            break;
        }
    }
}

async fn stderr_task(stderr: tokio::process::ChildStderr, msg_tx: UnboundedSender<LspMessage>) {
    let mut reader = BufReader::new(stderr);
    let mut buffer = String::new();
    while let Ok(bytes_read) = reader.read_line(&mut buffer).await {
        if bytes_read == 0 {
            break;
        }
        let _ = msg_tx.send(LspMessage::Stderr(buffer.trim_end().to_string()));
        buffer.clear();
    }
}
