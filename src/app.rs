// src/app.rs

use crate::components::editor::Editor;
use crate::components::file_tree::{scan_directory_async, FileTree, TreeNode};
use crate::core::i18n::{English, Language, SimplifiedChinese, TraditionalChinese};
use crate::core::lsp::{LspClient, LspMessage, LspNotification, LspResponse};
use crate::core::syntax::SyntaxHighlighter;
use crate::event::Event;
use crate::features::git::GitState;
use crate::features::plugin::PluginManager;
use crate::features::terminal::TerminalState;
use anyhow::{anyhow, Result};
use arboard::Clipboard;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use lsp_types::{
    notification::Notification,
    Diagnostic,
    InitializeParams,
    Uri,
    WorkspaceFolder,
};
use ratatui::layout::Rect;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::{
    fs as async_fs,
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use url::Url;

const HOVER_DELAY_MS: u64 = 500;
const DOUBLE_CLICK_INTERVAL_MS: u128 = 300;
const STATUS_MESSAGE_DURATION_SECS: u64 = 5;

/// Represents the result of a background task.
pub enum TaskResult {
    FileOpened { path: PathBuf, content: String },
    FileSaved { path: PathBuf, success: bool },
    DirectoryChanged(Result<FileTree>),
    DirectoryExpanded { path: Vec<usize>, children: Result<Vec<TreeNode>> },
    LspServerExited,
    Error(String),
}

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

pub enum LspStatus {
    Starting,
    Ready,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    Vertical,
}

#[derive(Debug, Clone, Copy)]
pub struct DraggingState {
    pub kind: DragKind,
    pub start_mouse_col: u16,
    pub start_percent: u16,
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
    _lsp_server: Option<Child>,
    pub lsp_status: LspStatus,
    pub diagnostics: HashMap<Uri, Vec<Diagnostic>>,
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
    pub status_message: Option<(String, Instant)>,
    pub file_tree_width_percent: u16,
    pub dragging: Option<DraggingState>,
    task_sender: UnboundedSender<TaskResult>,
    pub task_receiver: UnboundedReceiver<TaskResult>,
    clipboard: Option<Clipboard>,
}

impl App {
    pub fn new() -> Result<Self> {
        let (task_sender, task_receiver) = mpsc::unbounded_channel();
        let (git_state_sender, git_state_receiver) = mpsc::channel(1);

        Ok(Self {
            running: true,
            lang_state: CurrentLanguage::TraditionalChinese,
            lang: Box::new(TraditionalChinese),
            file_tree: FileTree::new(&env::current_dir()?)?,
            editor: Editor::new(),
            syntax_highlighter: SyntaxHighlighter::new(),
            lsp_client: LspClient::new(None),
            lsp_receiver: mpsc::unbounded_channel().1,
            _lsp_server: None,
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
            status_message: None,
            file_tree_width_percent: 25,
            dragging: None,
            task_sender,
            task_receiver,
            clipboard: Clipboard::new().ok(),
        })
    }

    pub fn start_lsp_server(
        &mut self,
        lsp_writer_tx: UnboundedSender<Value>,
        lsp_writer_rx: UnboundedReceiver<Value>,
        lsp_event_tx: UnboundedSender<LspMessage>,
    ) {
        let task_sender = self.task_sender.clone();
        tokio::spawn(async move {
            let lsp_event_tx_clone = lsp_event_tx.clone();
            let result: Result<Child> = async {
                let root_path = env::current_dir()?;
                let mut server = Command::new("rust-analyzer")
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;

                let stdin = server.stdin.take().ok_or_else(|| anyhow!("Failed to take LSP stdin"))?;
                let stdout = server.stdout.take().ok_or_else(|| anyhow!("Failed to take LSP stdout"))?;
                let stderr = server.stderr.take().ok_or_else(|| anyhow!("Failed to take LSP stderr"))?;

                tokio::spawn(writer_task(stdin, lsp_writer_rx));
                tokio::spawn(reader_task(stdout, lsp_event_tx.clone()));
                tokio::spawn(stderr_task(stderr, lsp_event_tx));

                let root_uri_str = Url::from_directory_path(root_path)
                    .map_err(|_| anyhow!("Failed to create root URI from path"))?
                    .to_string();
                let root_uri: Uri = root_uri_str.parse()
                    .map_err(|_| anyhow!("Failed to parse root URI"))?;

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
                    "id": crate::core::lsp::INITIALIZE_ID,
                    "method": "initialize",
                    "params": params,
                });
                lsp_writer_tx.send(request)?;
                Ok(server)
            }
            .await;

            match result {
                Ok(mut child) => {
                    // Spawn a monitor task to watch if the LSP server process exits
                    let monitor_task_sender = task_sender.clone();
                    tokio::spawn(async move {
                        let _ = child.wait().await;
                        if monitor_task_sender.send(TaskResult::LspServerExited).is_err() {
                            eprintln!("[ERROR] Failed to send LspServerExited task result");
                        }
                    });
                }
                Err(e) => {
                    let error_message = format!("Failed to start LSP server: {}", e);
                    if lsp_event_tx_clone.send(LspMessage::Error(
                        crate::core::lsp::INITIALIZE_ID,
                        serde_json::json!({ "message": error_message }),
                    )).is_err() {
                        eprintln!("Failed to send LSP server start error");
                    }
                }
            }
        });
    }

    pub fn tick(&mut self) {
        self.plugin_manager.tick_plugins();

        if let Some((_, time)) = self.status_message {
            if time.elapsed().as_secs() >= STATUS_MESSAGE_DURATION_SECS {
                self.status_message = None;
            }
        }

        if self.focus == Focus::Editor
            && self.hover_info.is_none()
            && self.last_cursor_move.elapsed() > Duration::from_millis(HOVER_DELAY_MS)
            && let Some(path) = &self.editor.path
        {
            if let Err(e) = self.lsp_client.hover(
                path,
                self.editor.cursor_row as u32,
                self.editor.cursor_col as u32,
            ) {
                eprintln!("[Clide DEBUG] Hover request failed: {}", e);
            }
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
            _ => {} // Ignore other event types for now
        }
    }

    fn handle_mouse_event(&mut self, event: MouseEvent) {
        if let MouseEventKind::Down(MouseButton::Left) = event.kind {
            let border_x = self.file_tree_area.right();
            if self.file_tree_area.width > 0 && event.column == border_x {
                self.dragging = Some(DraggingState {
                    kind: DragKind::Vertical,
                    start_mouse_col: event.column,
                    start_percent: self.file_tree_width_percent,
                });
                return;
            }
        }

        if let MouseEventKind::Drag(MouseButton::Left) = event.kind {
            if let Some(drag_state) = self.dragging {
                let total_width = self.file_tree_area.width + self.editor_area.width;
                if total_width > 0 {
                    let delta_x = event.column as i16 - drag_state.start_mouse_col as i16;
                    let delta_percent = (delta_x as f32 * 100.0 / total_width as f32).round() as i16;
                    let new_percent = (drag_state.start_percent as i16 + delta_percent) as u16;
                    self.file_tree_width_percent = new_percent.clamp(10, 90);
                }
                return;
            }
        }

        if let MouseEventKind::Up(MouseButton::Left) = event.kind {
            if self.dragging.is_some() {
                self.dragging = None;
                return;
            }
        }

        let is_double_click = self.last_click.as_ref().map_or(false, |(time, last_event)| {
            Instant::now().duration_since(*time).as_millis() < DOUBLE_CLICK_INTERVAL_MS
                && event.column == last_event.column
                && event.row == last_event.row
        });

        if self.is_mouse_over_area(&event, self.file_tree_area) {
            self.handle_file_tree_mouse(event, is_double_click);
        } else if self.is_mouse_over_area(&event, self.editor_area) {
            self.handle_editor_mouse(event);
        }
    }

    fn handle_file_tree_mouse(&mut self, event: MouseEvent, is_double_click: bool) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = Focus::FileTree;
                let relative_row = event.row.saturating_sub(self.file_tree_area.y + 2);
                self.file_tree.select_by_index(relative_row as usize);

                if is_double_click {
                    self.handle_file_tree_item_activation();
                    self.last_click = None;
                } else {
                    self.last_click = Some((Instant::now(), event));
                }
            }
            MouseEventKind::ScrollUp => self.file_tree.select_previous(),
            MouseEventKind::ScrollDown => self.file_tree.select_next(),
            _ => {} // Ignore other mouse events for file tree
        }
    }

    fn handle_file_tree_item_activation(&mut self) {
        let node_info = self.file_tree.get_selected_node_info();
        if node_info.is_parent_directory {
            self.change_directory(&node_info.path);
        } else if node_info.is_directory {
            self.toggle_directory_expansion();
        } else {
            self.open_file(node_info.path);
        }
    }

    fn handle_editor_mouse(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = Focus::Editor;
                let line_number_width = self.editor.content.len().to_string().len();
                let col = event.column.saturating_sub(self.editor_area.x + line_number_width as u16 + 3);
                let row = event.row.saturating_sub(self.editor_area.y + 1) + self.editor.vertical_scroll as u16;
                self.editor.move_cursor_to(row as usize, col as usize);
            }
            MouseEventKind::ScrollUp => self.editor.move_cursor_up(),
            MouseEventKind::ScrollDown => self.editor.move_cursor_down(),
            _ => {} // Ignore other mouse events for editor
        }
    }

    fn is_mouse_over_area(&self, event: &MouseEvent, area: Rect) -> bool {
        area.intersects(Rect::new(event.column, event.row, 1, 1))
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        let global_handled = match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('q'), KeyModifiers::CONTROL) => { self.running = false; true },
            (KeyCode::Char('l'), KeyModifiers::CONTROL) => { self.toggle_language(); true },
            (KeyCode::Char('s'), KeyModifiers::CONTROL) => { self.save_file(); true },
            (KeyCode::Char('w'), KeyModifiers::CONTROL) => { self.toggle_focus(); true },
            (KeyCode::Char('t'), KeyModifiers::CONTROL) => { self.toggle_panel(ActivePanel::Terminal); true },
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => { self.toggle_panel(ActivePanel::Git); true },
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => { self.copy_to_clipboard(); true },
            (KeyCode::Char('x'), KeyModifiers::CONTROL) => { self.cut_to_clipboard(); true },
            (KeyCode::Char('v'), KeyModifiers::CONTROL) => { self.paste_from_clipboard(); true },
            (KeyCode::Char('e'), KeyModifiers::CONTROL) => { self.open_under_cursor(); true },
            _ => false,
        };

        if global_handled { return; }

        if self.active_panel == Some(ActivePanel::Terminal) {
            self.terminal.handle_key_event(key_event);
            return;
        }

        match self.focus {
            Focus::FileTree => self.handle_file_tree_keys(key_event),
            Focus::Editor => self.handle_editor_keys(key_event),
        }
    }

    fn copy_to_clipboard(&mut self) {
        if self.focus == Focus::Editor {
            if let Some(clipboard) = &mut self.clipboard {
                let line_content = self.editor.content.get(self.editor.cursor_row).cloned().unwrap_or_default();
                if clipboard.set_text(line_content).is_ok() {
                    self.status_message = Some(("Copied to clipboard.".to_string(), Instant::now()));
                }
            }
        }
    }

    fn cut_to_clipboard(&mut self) {
        if self.focus == Focus::Editor {
            if let Some(clipboard) = &mut self.clipboard {
                if self.editor.cursor_row < self.editor.content.len() {
                    let line_content = if self.editor.content.len() == 1 {
                        std::mem::take(&mut self.editor.content[0])
                    } else {
                        self.editor.content.remove(self.editor.cursor_row)
                    };

                    if clipboard.set_text(line_content).is_ok() {
                        self.status_message = Some(("Cut to clipboard.".to_string(), Instant::now()));
                        self.editor.dirty = true;
                        if self.editor.cursor_row >= self.editor.content.len() {
                            self.editor.cursor_row = self.editor.content.len().saturating_sub(1);
                        }
                    }
                }
            }
        }
    }

    fn paste_from_clipboard(&mut self) {
        if self.focus == Focus::Editor {
            if let Some(clipboard) = &mut self.clipboard {
                if let Ok(text) = clipboard.get_text() {
                    self.editor.paste_text(&text);
                }
            }
        }
    }

    fn open_under_cursor(&mut self) {
        if self.focus == Focus::Editor {
            let word = self.editor.get_word_under_cursor();
            if Path::new(&word).exists() {
                if let Err(e) = open::that(&word) {
                    self.status_message = Some((format!("Failed to open path: {}", e), Instant::now()));
                }
            } else if Url::parse(&word).is_ok() {
                if let Err(e) = open::that(&word) {
                    self.status_message = Some((format!("Failed to open URL: {}", e), Instant::now()));
                }
            } else {
                self.status_message = Some(("Not a valid path or URL.".to_string(), Instant::now()));
            }
        }
    }

    fn handle_file_tree_keys(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => self.file_tree.select_previous(),
            KeyCode::Down => self.file_tree.select_next(),
            KeyCode::Left => self.file_tree.collapse_selected(),
            KeyCode::Right => self.toggle_directory_expansion(),
            KeyCode::Enter => self.handle_file_tree_item_activation(),
            _ => {} // Ignore other key events for file tree
        }
    }

    fn toggle_directory_expansion(&mut self) {
        let node = self.file_tree.get_selected_node_mut();
        if !node.is_directory { return; }

        if node.is_expanded {
            node.is_expanded = false;
        } else if !node.children.is_empty() {
            node.is_expanded = true;
        } else {
            let tx = self.task_sender.clone();
            let path = node.path.clone();
            let selection_path = self.file_tree.selected.clone();
            tokio::spawn(async move {
                let children_result = scan_directory_async(&path).await;
                if tx.send(TaskResult::DirectoryExpanded { path: selection_path, children: children_result }).is_err() {
                    eprintln!("[ERROR] Failed to send DirectoryExpanded task result");
                }
            });
        }
    }

    fn handle_editor_keys(&mut self, key_event: KeyEvent) {
        if self.completion_list.is_some() {
            self.handle_completion_keys(key_event);
        } else {
            self.handle_standard_editor_keys(key_event);
        }
    }

    fn handle_completion_keys(&mut self, key_event: KeyEvent) {
        if let Some(completion_list) = &self.completion_list {
            match key_event.code {
                KeyCode::Up => self.completion_selection = Some(self.completion_selection.unwrap_or(0).saturating_sub(1)),
                KeyCode::Down => {
                    let selection = self.completion_selection.unwrap_or(0);
                    if selection < completion_list.len() - 1 {
                        self.completion_selection = Some(selection + 1);
                    }
                }
                KeyCode::Enter | KeyCode::Tab => {
                    if let Some(selection) = self.completion_selection {
                        if let Some(item) = completion_list.get(selection) {
                            self.editor.insert_text(&item.label);
                        }
                    }
                    self.completion_list = None;
                    self.completion_selection = None;
                }
                KeyCode::Esc => {
                    self.completion_list = None;
                    self.completion_selection = None;
                }
                _ => {
                    self.completion_list = None;
                    self.completion_selection = None;
                    self.handle_standard_editor_keys(key_event);
                }
            }
        }
    }

    fn handle_standard_editor_keys(&mut self, key_event: KeyEvent) {
        match (key_event.code, key_event.modifiers) {
            (KeyCode::Char('z'), KeyModifiers::CONTROL) => self.editor.undo(),
            (KeyCode::Char('y'), KeyModifiers::CONTROL) => self.editor.redo(),
            _ => match key_event.code {
                KeyCode::Up => self.editor.move_cursor_up(),
                KeyCode::Down => self.editor.move_cursor_down(),
                KeyCode::Left => self.editor.move_cursor_left(),
                KeyCode::Right => self.editor.move_cursor_right(),
                KeyCode::Char(c) => {
                    self.editor.insert_text(&c.to_string());
                    if (c == '.' || c == ':') && self.editor.path.is_some() {
                        let _ = self.lsp_client.completion(self.editor.path.as_ref().unwrap(), self.editor.cursor_row as u32, self.editor.cursor_col as u32);
                    }
                }
                KeyCode::Backspace => self.editor.delete_char(),
                KeyCode::Enter => self.editor.insert_newline(),
                KeyCode::Tab => self.editor.insert_text("    "),
                KeyCode::Delete => self.editor.delete_forward_char(),
                KeyCode::Home => self.editor.move_cursor_home(),
                KeyCode::End => self.editor.move_cursor_end(),
                KeyCode::PageUp => self.editor.move_cursor_page_up(self.editor_area.height as usize),
                KeyCode::PageDown => self.editor.move_cursor_page_down(self.editor_area.height as usize),
                KeyCode::Insert => self.editor.toggle_overwrite_mode(),
                _ => {} // Ignore other key events for editor
            },
        }
        self.last_cursor_move = Instant::now();
        self.hover_info = None;
    }

    pub fn open_file(&mut self, path: PathBuf) {
        let tx = self.task_sender.clone();
        tokio::spawn(async move {
            let result = match async_fs::read_to_string(&path).await {
                Ok(content) => TaskResult::FileOpened { path, content },
                Err(e) => TaskResult::Error(format!("Failed to open file: {}", e)),
            };
            if tx.send(result).is_err() {
                eprintln!("[ERROR] Failed to send FileOpened task result");
            }
        });
    }

    fn change_directory(&mut self, path: &PathBuf) {
        let tx = self.task_sender.clone();
        let path_clone = path.clone();
        tokio::spawn(async move {
            let result = FileTree::new_async(&path_clone).await;
            if tx.send(TaskResult::DirectoryChanged(result)).is_err() {
                eprintln!("[ERROR] Failed to send DirectoryChanged task result");
            }
        });
    }

    fn save_file(&mut self) {
        if let Some(path) = self.editor.path.clone() {
            let content = self.editor.content.join("\n");
            let tx = self.task_sender.clone();
            tokio::spawn(async move {
                let result = match async_fs::write(&path, content).await {
                    Ok(_) => TaskResult::FileSaved { path, success: true },
                    Err(e) => TaskResult::Error(format!("Failed to save file: {}", e)),
                };
                if tx.send(result).is_err() {
                    eprintln!("[ERROR] Failed to send FileSaved task result");
                }
            });
        }
    }

    fn toggle_focus(&mut self) {
        self.focus = if self.focus == Focus::FileTree { Focus::Editor } else { Focus::FileTree };
    }

    fn toggle_panel(&mut self, panel: ActivePanel) {
        self.active_panel = if self.active_panel == Some(panel) {
            None
        } else {
            if panel == ActivePanel::Git {
                let tx = self.git_state_sender.clone();
                let mut git_state = self.git.clone();
                tokio::spawn(async move {
                    git_state.update().await;
                    let _ = tx.send(git_state).await;
                });
            }
            Some(panel)
        };
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

    pub fn handle_task_result(&mut self, result: TaskResult) {
        match result {
            TaskResult::FileOpened { path, content } => {
                let _ = self.lsp_client.did_open(&path, &content);
                self.file_tree.select_path(&path);
                self.editor.set_content(path, content);
                self.focus = Focus::Editor;
            }
            TaskResult::FileSaved { path, success } => {
                if success {
                    if self.editor.path.as_ref() == Some(&path) {
                        self.editor.dirty = false;
                    }
                    self.status_message = Some((format!("Saved {}", path.to_string_lossy()), Instant::now()));
                } else {
                    self.status_message = Some((format!("Failed to save {}", path.to_string_lossy()), Instant::now()));
                }
            }
            TaskResult::DirectoryChanged(Ok(new_tree)) => {
                self.file_tree = new_tree;
            }
            TaskResult::DirectoryExpanded { path, children: Ok(children) } => {
                let mut node = &mut self.file_tree.root;
                for &index in &path {
                    if index >= node.children.len() { return; }
                    node = &mut node.children[index];
                }
                node.children = children;
                node.is_expanded = true;
                self.file_tree.mark_dirty();
            }
            TaskResult::LspServerExited => {
                self.lsp_status = LspStatus::Failed;
                self.status_message = Some(("LSP server has stopped.".to_string(), Instant::now()));
            }
            TaskResult::DirectoryChanged(Err(e)) | TaskResult::DirectoryExpanded { children: Err(e), .. } => {
                self.status_message = Some((format!("FileTree Error: {}", e), Instant::now()));
            }
            TaskResult::Error(e) => {
                self.status_message = Some((format!("Error: {}", e), Instant::now()));
            }
        }
    }

    pub fn handle_lsp_message(&mut self, lsp_message: LspMessage) {
        match lsp_message {
            LspMessage::Notification(method, params) => {
                if method == lsp_types::notification::PublishDiagnostics::METHOD {
                    if let Ok(diagnostics) = serde_json::from_value::<lsp_types::PublishDiagnosticsParams>(params) {
                        self.diagnostics.insert(diagnostics.uri, diagnostics.diagnostics);
                    }
                }
            }
            LspMessage::Response(id, result) => self.handle_lsp_response(id, result),
            LspMessage::Error(id, error) => {
                eprintln!("[Clide LSP ERROR] ID: {}, Body: {:?}", id, error);
                if id == crate::core::lsp::INITIALIZE_ID {
                    self.lsp_status = LspStatus::Failed;
                }
            }
            LspMessage::Stderr(msg) => {
                self.status_message = Some((msg, Instant::now()));
            }
        }
    }

    fn handle_lsp_response(&mut self, id: u64, result: Value) {
        match id {
            crate::core::lsp::INITIALIZE_ID => { self.lsp_status = LspStatus::Ready; } // LSP server initialized successfully
            crate::core::lsp::COMPLETION_ID => {
                if let Ok(Some(lsp_types::CompletionResponse::Array(items))) = serde_json::from_value(result) {
                    if !items.is_empty() { self.completion_selection = Some(0); }
                    self.completion_list = Some(items);
                }
            }
            crate::core::lsp::HOVER_ID => {
                if let Ok(Some(hover)) = serde_json::from_value(result) { self.hover_info = Some(hover); }
            }
            crate::core::lsp::GOTO_DEFINITION_ID => {
                if let Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(location))) = serde_json::from_value(result) {
                    if let Some(path_str) = location.uri.as_str().strip_prefix("file://") {
                        let path = std::path::PathBuf::from(path_str);
                        self.open_file(path);
                        self.editor.move_cursor_to(location.range.start.line as usize, location.range.start.character as usize);
                    }
                }
            }
            _ => {} // Ignore unknown LSP response IDs
        }
    }
}

async fn writer_task(mut stdin: tokio::process::ChildStdin, mut rx: UnboundedReceiver<Value>) {
    while let Some(msg) = rx.recv().await {
        let msg_str = msg.to_string();
        let content = format!("Content-Length: {}\r\n\r\nTell me more about this file: {}", msg_str.len(), msg_str);
        if stdin.write_all(content.as_bytes()).await.is_err() { break; }
    }
}

async fn reader_task<T: tokio::io::AsyncRead + Unpin>(stream: T, msg_tx: UnboundedSender<LspMessage>) {
    let mut reader = BufReader::new(stream);
    let mut buffer = String::new();
    let mut content_length: Option<usize> = None;

    loop {
        buffer.clear();
        if let Ok(bytes_read) = reader.read_line(&mut buffer).await {
            if bytes_read == 0 { break; } // EOF
            if buffer.starts_with("Content-Length:") {
                if let Some(len_str) = buffer.trim().split(':').nth(1) {
                    content_length = len_str.trim().parse().ok();
                }
            } else if buffer.trim().is_empty() {
                // End of headers, process the body if content length is known
                if let Some(length) = content_length.take() {
                    let mut body_buffer = vec![0; length];
                    if reader.read_exact(&mut body_buffer).await.is_ok() {
                        // Try to deserialize as Notification first, then Response
                        if let Ok(notification) = serde_json::from_slice::<LspNotification>(&body_buffer) {
                            let _ = msg_tx.send(LspMessage::Notification(notification.method, notification.params));
                        } else if let Ok(response) = serde_json::from_slice::<LspResponse>(&body_buffer) {
                            let lsp_message = match response {
                                LspResponse::Success { id, result } => LspMessage::Response(id, result),
                                LspResponse::Error { id, error } => LspMessage::Error(id, error),
                            };
                            let _ = msg_tx.send(lsp_message);
                        }
                    }
                }
            }
        } else { break; } // Error reading line
    }
}

async fn stderr_task(stderr: tokio::process::ChildStderr, msg_tx: UnboundedSender<LspMessage>) {
    let mut reader = BufReader::new(stderr);
    let mut buffer = String::new();
    while let Ok(bytes_read) = reader.read_line(&mut buffer).await {
        if bytes_read == 0 { break; } // EOF
        if msg_tx.send(LspMessage::Stderr(buffer.trim_end().to_string())).is_err() {
            break; // Channel closed
        }
        buffer.clear();
    }
}
