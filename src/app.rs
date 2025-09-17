use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::definitions::{
    DividerKind, EditorPreferences, FocusArea, IndentKind, LayoutState, MenuAction, MenuBar,
    PaneKind, StatusControlKind, StatusControlRegistry,
};
use crate::editor::Editor;
use crate::file_tree::{FileEntryKind, FileTree, FileTreeAction};
use crate::panels::{agent::AgentPanel, terminal::TerminalPane};
use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;

pub struct App {
    pub should_quit: bool,
    pub focus: FocusArea,
    pub editor: Editor,
    pub file_tree: FileTree,
    pub terminal: TerminalPane,
    pub agent: AgentPanel,
    pub status_message: String,
    pub workspace_root: PathBuf,
    pub layout: LayoutState,
    pub preferences: EditorPreferences,
    pub menu_bar: MenuBar,
    pub status_controls: StatusControlRegistry,
    pub editor_hover_line: Option<usize>,
    pub overlay: Option<OverlayState>,
    pub suppress_delete_confirm: bool,
    editor_drag_selecting: bool,
    last_tick: Instant,
    tick_rate: Duration,
    last_click: Option<(Instant, u16, u16)>,
}

#[derive(Debug, Clone)]
pub enum OverlayState {
    CommandPalette(CommandPaletteState),
    InputPrompt(InputPromptState),
    ConfirmDelete(ConfirmDeleteState),
}

#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    pub entries: Vec<CommandPaletteEntry>,
    pub filter: String,
    pub visible: Vec<usize>,
    pub selected: usize,
}

impl CommandPaletteState {
    pub fn new(entries: Vec<CommandPaletteEntry>) -> Self {
        let visible = (0..entries.len()).collect();
        Self {
            entries,
            filter: String::new(),
            visible,
            selected: 0,
        }
    }

    pub fn selected_entry(&self) -> Option<&CommandPaletteEntry> {
        self.visible
            .get(self.selected)
            .and_then(|idx| self.entries.get(*idx))
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.visible.len() as isize;
        let mut index = self.selected as isize + delta;
        if index < 0 {
            index += len * ((-index) / len + 1);
        }
        index %= len;
        self.selected = index as usize;
    }

    pub fn update_filter(&mut self) {
        if self.filter.trim().is_empty() {
            self.visible = (0..self.entries.len()).collect();
            self.selected = 0.min(self.visible.len().saturating_sub(1));
            return;
        }
        let needle = self.filter.to_lowercase();
        self.visible = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(idx, entry)| {
                if entry.search_text.to_lowercase().contains(&needle) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();
        if self.selected >= self.visible.len() && !self.visible.is_empty() {
            self.selected = self.visible.len() - 1;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandPaletteEntry {
    pub label: String,
    pub detail: Option<String>,
    pub action: CommandAction,
    search_text: String,
}

impl CommandPaletteEntry {
    pub fn new(label: impl Into<String>, detail: Option<String>, action: CommandAction) -> Self {
        let label_str = label.into();
        let mut search = label_str.clone();
        if let Some(detail_str) = detail.as_ref() {
            search.push(' ');
            search.push_str(detail_str);
        }
        Self {
            label: label_str,
            detail,
            action,
            search_text: search,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAction {
    NewDocument,
    CreateFile,
    OpenFile,
    SaveFile,
    SaveFileAs,
    ToggleHiddenFiles,
    ToggleWrap,
    ToggleLineEnding,
    ToggleEncoding,
    CycleIndent,
    ToggleFileTree,
    ToggleEditor,
    ToggleTerminal,
    ToggleAgent,
    DeleteFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingInputAction {
    OpenFile,
    SaveAs,
    CreateFile,
}

#[derive(Debug, Clone)]
pub struct InputPromptState {
    pub title: String,
    pub value: String,
    pub placeholder: String,
    pub action: PendingInputAction,
    pub error: Option<String>,
}

impl InputPromptState {
    pub fn new(
        title: impl Into<String>,
        placeholder: impl Into<String>,
        action: PendingInputAction,
        initial: Option<String>,
    ) -> Self {
        Self {
            title: title.into(),
            value: initial.unwrap_or_default(),
            placeholder: placeholder.into(),
            action,
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfirmDeleteState {
    pub target: PathBuf,
    pub display: String,
    pub selected_index: usize,
    pub suppress_future: bool,
}

impl ConfirmDeleteState {
    pub fn new(target: PathBuf, display: String) -> Self {
        Self {
            target,
            display,
            selected_index: 0,
            suppress_future: false,
        }
    }

    pub fn toggle_selection(&mut self) {
        self.selected_index = (self.selected_index + 1) % 2;
    }

    pub fn select(&mut self, index: usize) {
        self.selected_index = index.min(1);
    }

    pub fn confirm_selected(&self) -> bool {
        self.selected_index == 0
    }
}

impl App {
    pub fn new(workspace_root: PathBuf) -> Result<Self> {
        let canonical_root = workspace_root.canonicalize().unwrap_or(workspace_root);
        let mut file_tree = FileTree::from_root(canonical_root.clone());
        if file_tree.is_empty() {
            file_tree.populate_with_placeholder();
        }

        let editor = Editor::with_placeholder(
            "// 歡迎使用 Clide 命令列 IDE 原型\n\
             // 左：檔案樹 | 中：編輯器 + 終端機 | 右：代理面板\n\
             fn main() {\n    println!(\"開始創作吧！\");\n}\n",
        );

        let mut terminal = TerminalPane::default();
        terminal.append_line("> cargo run    // 整合終端機示意");
        terminal.append_line("建置未啟動：敬請期待");

        let agent = AgentPanel::with_placeholder();

        Ok(Self {
            should_quit: false,
            focus: FocusArea::Editor,
            editor,
            file_tree,
            terminal,
            agent,
            status_message: String::from("F6 切換窗格、輸入即編輯、Ctrl+Q 離開"),
            workspace_root: canonical_root,
            layout: LayoutState::new(),
            preferences: EditorPreferences::new(),
            menu_bar: MenuBar::new(),
            status_controls: StatusControlRegistry::default(),
            editor_hover_line: None,
            overlay: None,
            suppress_delete_confirm: false,
            editor_drag_selecting: false,
            last_tick: Instant::now(),
            tick_rate: Duration::from_millis(250),
            last_click: None,
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        if self.overlay.is_some() {
            self.handle_overlay_key(key);
            return;
        }

        if self.menu_bar.open {
            match key.code {
                KeyCode::Esc => {
                    self.menu_bar.close();
                    return;
                }
                KeyCode::Left => {
                    self.menu_bar.move_active(-1);
                    if let Some(idx) = self.menu_bar.active_index {
                        if let Some(item) = self.menu_bar.items.get(idx) {
                            self.status_message = format!("選單：{}", item.title);
                        }
                    }
                    self.update_menu_hover_message();
                    return;
                }
                KeyCode::Right => {
                    self.menu_bar.move_active(1);
                    if let Some(idx) = self.menu_bar.active_index {
                        if let Some(item) = self.menu_bar.items.get(idx) {
                            self.status_message = format!("選單：{}", item.title);
                        }
                    }
                    self.update_menu_hover_message();
                    return;
                }
                KeyCode::Up => {
                    self.menu_bar.move_highlight(-1);
                    self.update_menu_hover_message();
                    return;
                }
                KeyCode::Down => {
                    self.menu_bar.move_highlight(1);
                    self.update_menu_hover_message();
                    return;
                }
                KeyCode::Enter => {
                    if let Some(action) = self.menu_bar.highlighted_action() {
                        self.execute_menu_action(action);
                    }
                    self.menu_bar.close();
                    return;
                }
                _ => {}
            }
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char(ch), m)
                if (ch == 's' || ch == 'S') && m.contains(KeyModifiers::CONTROL) =>
            {
                if m.contains(KeyModifiers::ALT) {
                    self.prompt_input(
                        PendingInputAction::SaveAs,
                        "另存新檔",
                        self.suggest_current_path(),
                    );
                } else {
                    let _ = self.perform_save_current();
                }
                return;
            }
            (KeyCode::Char(ch), m)
                if (ch == 'o' || ch == 'O') && m.contains(KeyModifiers::CONTROL) =>
            {
                self.prompt_input(
                    PendingInputAction::OpenFile,
                    "開啟檔案",
                    self.suggest_current_path(),
                );
                return;
            }
            (KeyCode::Char(ch), m)
                if (ch == 'n' || ch == 'N') && m.contains(KeyModifiers::CONTROL) =>
            {
                if m.contains(KeyModifiers::SHIFT) || ch.is_uppercase() {
                    self.prompt_input(
                        PendingInputAction::CreateFile,
                        "建立新檔案",
                        self.suggest_current_path(),
                    );
                } else {
                    self.perform_new_document();
                }
                return;
            }
            (KeyCode::Char(ch), m)
                if (ch == 'p' || ch == 'P') && m.contains(KeyModifiers::CONTROL) =>
            {
                if m.contains(KeyModifiers::SHIFT) || ch.is_uppercase() {
                    self.open_command_palette();
                    return;
                }
            }
            (KeyCode::Char('q'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
                return;
            }
            (KeyCode::F(10), _) => {
                if self.menu_bar.open {
                    self.menu_bar.close();
                } else {
                    self.menu_bar.open(0);
                    if let Some(item) = self.menu_bar.items.first() {
                        self.status_message = format!("選單：{}", item.title);
                    }
                    self.update_menu_hover_message();
                }
                return;
            }
            (KeyCode::F(6), m) if m.contains(KeyModifiers::SHIFT) => {
                self.cycle_focus(-1);
                return;
            }
            (KeyCode::F(6), _) => {
                self.cycle_focus(1);
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
            KeyCode::Tab => match self.preferences.indent {
                crate::app::IndentKind::Spaces(n) => {
                    for _ in 0..n {
                        self.editor.insert_char(' ');
                    }
                }
                crate::app::IndentKind::Tabs => self.editor.insert_char('\t'),
            },
            KeyCode::Char(ch) if key.modifiers.contains(KeyModifiers::CONTROL) => match ch {
                'a' | 'A' => self.status_message = "代理請求尚未實作".into(),
                _ => {}
            },
            KeyCode::Char(ch) => {
                self.editor.insert_char(ch);
            }
            KeyCode::Enter => self.editor.insert_newline(),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete_forward(),
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
            KeyCode::Enter => self.activate_file_tree_selection(),
            KeyCode::Delete => self.delete_selected_from_tree(),
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.file_tree.refresh();
                self.status_message = String::from("檔案樹已更新");
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
                    self.status_message = format!("代理建議待套用：{}", selected.title);
                }
            }
            _ => {}
        }
    }

    fn activate_file_tree_selection(&mut self) {
        match self.file_tree.activate_selected() {
            FileTreeAction::OpenFile(path) => {
                let _ = self.perform_open_file(path);
            }
            FileTreeAction::ChangedDir(path) => {
                let display = self.format_display_path(&path);
                self.status_message = format!("切換目錄：{}", display);
            }
            FileTreeAction::None => {}
        }
    }

    fn cycle_focus(&mut self, direction: isize) {
        let order = self.visible_focus_order();
        if order.is_empty() {
            return;
        }
        let current_index = order
            .iter()
            .position(|area| *area == self.focus)
            .unwrap_or(0);
        let len = order.len() as isize;
        let mut new_index = current_index as isize + direction;
        if new_index < 0 {
            new_index += len;
        }
        new_index %= len;
        self.focus = order[new_index as usize];
        self.status_message = format!("焦點：{}", self.focus.label());
    }

    fn visible_focus_order(&self) -> Vec<FocusArea> {
        let mut order = Vec::new();
        if self.layout.tree_visible {
            order.push(FocusArea::FileTree);
        }
        if self.layout.editor_visible {
            order.push(FocusArea::Editor);
        }
        if self.layout.terminal_visible {
            order.push(FocusArea::Terminal);
        }
        if self.layout.agent_visible {
            order.push(FocusArea::Agent);
        }
        if order.is_empty() {
            order.push(FocusArea::Editor);
        }
        order
    }

    fn toggle_wrap_mode(&mut self) {
        self.preferences.wrap_mode = self.preferences.wrap_mode.toggle();
        self.status_message = format!("換行方式：{}", self.preferences.wrap_mode.label());
    }

    fn toggle_line_ending(&mut self) {
        self.preferences.line_ending = self.preferences.line_ending.toggle();
        self.status_message = format!("換行符號：{}", self.preferences.line_ending.label());
    }

    fn toggle_encoding(&mut self) {
        self.preferences.encoding = self.preferences.encoding.toggle();
        self.status_message = format!("編碼方式：{}", self.preferences.encoding.label());
    }

    fn cycle_indent_kind(&mut self) {
        self.preferences.indent = self.preferences.indent.next();
        self.status_message = format!("縮排方式：{}", self.preferences.indent.label());
    }

    fn handle_status_control_click(&mut self, kind: StatusControlKind) -> bool {
        match kind {
            StatusControlKind::Wrap => {
                self.toggle_wrap_mode();
                true
            }
            StatusControlKind::LineEnding => {
                self.toggle_line_ending();
                true
            }
            StatusControlKind::Encoding => {
                self.toggle_encoding();
                true
            }
            StatusControlKind::Indent => {
                self.cycle_indent_kind();
                true
            }
            StatusControlKind::HiddenFiles => {
                self.toggle_hidden_files();
                true
            }
            StatusControlKind::Cursor => {
                let (line, col) = self.editor.cursor();
                self.status_message = format!("游標位置：{}:{}", line + 1, col + 1);
                true
            }
            StatusControlKind::Dirty => {
                if self.editor.is_dirty() {
                    self.status_message = String::from("檔案尚未儲存");
                } else {
                    self.status_message = String::from("檔案已儲存");
                }
                true
            }
        }
    }

    fn toggle_hidden_files(&mut self) {
        let enabled = self.file_tree.toggle_show_hidden();
        self.status_message = if enabled {
            String::from("已顯示隱藏檔案")
        } else {
            String::from("已隱藏隱藏檔案")
        };
    }

    fn execute_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::ToggleWrap => {
                self.toggle_wrap_mode();
            }
            MenuAction::ToggleLineEnding => {
                self.toggle_line_ending();
            }
            MenuAction::ToggleEncoding => {
                self.toggle_encoding();
            }
            MenuAction::CycleIndent => {
                self.cycle_indent_kind();
            }
            MenuAction::ToggleFileTree => {
                self.toggle_pane(PaneKind::FileTree);
            }
            MenuAction::ToggleEditor => {
                self.toggle_pane(PaneKind::Editor);
            }
            MenuAction::ToggleTerminal => {
                self.toggle_pane(PaneKind::Terminal);
            }
            MenuAction::ToggleAgent => {
                self.toggle_pane(PaneKind::Agent);
            }
            MenuAction::New => {
                self.perform_new_document();
            }
            MenuAction::CreateFile => {
                self.prompt_input(
                    PendingInputAction::CreateFile,
                    "建立新檔案",
                    self.suggest_current_path(),
                );
            }
            MenuAction::Open => {
                self.prompt_input(
                    PendingInputAction::OpenFile,
                    "開啟檔案",
                    self.suggest_current_path(),
                );
            }
            MenuAction::Save => {
                let _ = self.perform_save_current();
            }
            MenuAction::SaveAs => {
                self.prompt_input(
                    PendingInputAction::SaveAs,
                    "另存新檔",
                    self.suggest_current_path(),
                );
            }
            MenuAction::ToggleHiddenFiles => {
                self.toggle_hidden_files();
            }
            MenuAction::Delete => {
                self.delete_via_prompt();
            }
            MenuAction::CommandPalette => {
                self.open_command_palette();
            }
            MenuAction::Exit => {
                self.should_quit = true;
                self.status_message = String::from("離開應用程式");
            }
            MenuAction::None => {}
        }
    }

    fn update_menu_hover_message(&mut self) {
        if let Some(menu_idx) = self.menu_bar.active_index {
            if let Some(entry_idx) = self.menu_bar.highlighted_entry {
                if let Some(menu) = self.menu_bar.items.get(menu_idx) {
                    if let Some(entry) = menu.entries.get(entry_idx) {
                        self.status_message = format!("{} > {}", menu.title, entry.label);
                        return;
                    }
                }
            }
            if let Some(menu) = self.menu_bar.items.get(menu_idx) {
                self.status_message = format!("選單：{}", menu.title);
            }
        }
    }

    fn format_display_path(&self, path: &Path) -> String {
        if let Ok(relative) = path.strip_prefix(&self.workspace_root) {
            if relative.as_os_str().is_empty() {
                String::from("./")
            } else {
                format!("./{}", relative.display())
            }
        } else if let Some(home) = Self::home_directory() {
            if path.starts_with(&home) {
                let suffix = path.strip_prefix(&home).unwrap();
                if suffix.as_os_str().is_empty() {
                    String::from("~")
                } else {
                    format!("~/{}", suffix.display())
                }
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        }
    }

    fn format_input_path(&self, path: &Path) -> String {
        if let Ok(relative) = path.strip_prefix(&self.workspace_root) {
            if relative.as_os_str().is_empty() {
                String::from(".")
            } else {
                relative.display().to_string()
            }
        } else if let Some(home) = Self::home_directory() {
            if path.starts_with(&home) {
                let suffix = path.strip_prefix(&home).unwrap();
                if suffix.as_os_str().is_empty() {
                    String::from("~")
                } else {
                    format!("~/{}", suffix.display())
                }
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        }
    }

    fn home_directory() -> Option<PathBuf> {
        if cfg!(windows) {
            std::env::var("USERPROFILE").map(PathBuf::from).ok()
        } else {
            std::env::var("HOME").map(PathBuf::from).ok()
        }
    }

    fn canonicalize_path(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn ensure_file_tree_highlights(&mut self, path: &Path) {
        let target = self.canonicalize_path(path);
        if let Some(idx) = self
            .file_tree
            .entries()
            .iter()
            .position(|entry| entry.path == target)
        {
            self.file_tree.set_selection(idx);
        }
    }

    fn perform_open_file(&mut self, path: PathBuf) -> Result<(), String> {
        match self.editor.open_file(&path) {
            Ok(_) => {
                let display = self.format_display_path(&path);
                self.status_message = format!("已開啟檔案：{}", display);
                self.focus = FocusArea::Editor;
                self.ensure_file_tree_highlights(&path);
                Ok(())
            }
            Err(err) => {
                let message = format!("開啟失敗：{err}");
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    fn perform_save_current(&mut self) -> Result<(), String> {
        if self.editor.file_path().is_none() {
            self.prompt_input(
                PendingInputAction::SaveAs,
                "儲存檔案為",
                self.suggest_current_path(),
            );
            return Err(String::from("尚未指定檔案路徑"));
        }
        match self.editor.save() {
            Ok(path) => {
                let display = self.format_display_path(&path);
                self.status_message = format!("已儲存檔案：{}", display);
                Ok(())
            }
            Err(err) => {
                let message = format!("儲存失敗：{err}");
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    fn perform_save_as(&mut self, path: PathBuf) -> Result<(), String> {
        match self.editor.save_as(&path) {
            Ok(saved) => {
                let display = self.format_display_path(&saved);
                self.status_message = format!("已另存檔案：{}", display);
                self.file_tree.refresh();
                if self.is_in_workspace(&saved) {
                    self.ensure_file_tree_highlights(&saved);
                }
                Ok(())
            }
            Err(err) => {
                let message = format!("另存失敗：{err}");
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    fn perform_create_file(&mut self, path: PathBuf) -> Result<(), String> {
        if path.exists() {
            let display = self.format_display_path(&path);
            let message = format!("建立失敗：檔案已存在 ({display})");
            self.status_message = message.clone();
            return Err(message);
        }
        self.editor.new_document();
        match self.editor.save_as(&path) {
            Ok(saved) => {
                let display = self.format_display_path(&saved);
                self.status_message = format!("已建立檔案：{}", display);
                self.file_tree.refresh();
                if self.is_in_workspace(&saved) {
                    self.ensure_file_tree_highlights(&saved);
                }
                Ok(())
            }
            Err(err) => {
                let message = format!("建立失敗：{err}");
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    fn perform_new_document(&mut self) {
        self.editor.new_document();
        self.status_message = String::from("已建立新的未命名檔案");
        self.focus = FocusArea::Editor;
    }

    fn suggest_current_path(&self) -> Option<String> {
        if let Some(path) = self.editor.file_path() {
            return Some(self.format_input_path(path));
        }
        if let Some(entry) = self.file_tree.selected_entry() {
            match entry.kind {
                FileEntryKind::File | FileEntryKind::Directory => {
                    return Some(self.format_input_path(&entry.path));
                }
                _ => {}
            }
        }
        None
    }

    fn resolve_input_path(&self, raw: &str) -> Result<PathBuf, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(String::from("請輸入路徑"));
        }
        let candidate = Self::expand_user_path(trimmed)?;
        let path = if candidate.is_absolute() {
            candidate
        } else {
            self.workspace_root.join(candidate)
        };
        Ok(path)
    }

    fn expand_user_path(raw: &str) -> Result<PathBuf, String> {
        if let Some(rest) = raw.strip_prefix('~') {
            let home = Self::home_directory().ok_or_else(|| String::from("找不到家目錄路徑"))?;
            if rest.is_empty() {
                return Ok(home);
            }
            let mut chars = rest.chars();
            match chars.next() {
                Some('/') | Some('\\') => Ok(home.join(chars.as_str())),
                _ => Err(String::from("不支援的家目錄縮寫格式")),
            }
        } else {
            Ok(PathBuf::from(raw))
        }
    }

    fn is_in_workspace(&self, path: &Path) -> bool {
        path.starts_with(&self.workspace_root)
    }

    fn editor_inner_rect(&self) -> Option<Rect> {
        let geom = self.layout.pane_geometry(PaneKind::Editor)?;
        let area = geom.area;
        let width = area.width.saturating_sub(2);
        let height = area.height.saturating_sub(2);
        if width == 0 || height == 0 {
            return None;
        }
        Some(Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width,
            height,
        })
    }

    fn editor_text_metrics(&self) -> Option<(Rect, usize)> {
        let inner = self.editor_inner_rect()?;
        let text_width = inner.width.saturating_sub(EDITOR_GUTTER_WIDTH).max(1) as usize;
        Some((inner, text_width))
    }

    fn move_editor_cursor_with_mouse(&mut self, column: u16, row: u16, extend: bool) {
        let Some((inner, text_width)) = self.editor_text_metrics() else {
            return;
        };
        if row < inner.y || row >= inner.y + inner.height {
            return;
        }

        let visual_row = (row - inner.y) as usize;
        let text_start_x = inner.x.saturating_add(EDITOR_GUTTER_WIDTH);
        let visual_col = if column <= text_start_x {
            0
        } else if column >= inner.x.saturating_add(inner.width) {
            text_width
        } else {
            (column - text_start_x) as usize
        };

        let viewport_line = self.editor.viewport_start();
        let viewport_offset = self.editor.viewport_line_offset();
        self.editor.move_cursor_visual(
            viewport_line,
            viewport_offset,
            text_width,
            visual_row,
            visual_col,
            extend,
        );
        let (line, col) = self.editor.cursor();
        self.status_message = format!("游標位置：{}:{}", line + 1, col + 1);
    }

    fn editor_line_at_visual_row(&self, row: u16) -> Option<usize> {
        let (inner, text_width) = self.editor_text_metrics()?;
        if row < inner.y || row >= inner.y + inner.height {
            return None;
        }
        let visual_row = (row - inner.y) as usize;
        let viewport_line = self.editor.viewport_start();
        let viewport_offset = self.editor.viewport_line_offset();
        self.editor
            .position_for_visual(viewport_line, viewport_offset, text_width, visual_row, 0)
            .map(|(line, _)| line)
            .or_else(|| {
                if self.editor.total_lines() == 0 {
                    Some(0)
                } else {
                    Some(self.editor.total_lines() - 1)
                }
            })
    }

    fn prompt_input(
        &mut self,
        action: PendingInputAction,
        title: &str,
        suggestion: Option<String>,
    ) {
        let placeholder = match action {
            PendingInputAction::OpenFile => "輸入要開啟的檔案路徑（相對於工作區）",
            PendingInputAction::SaveAs => "輸入要儲存的檔案路徑（相對於工作區）",
            PendingInputAction::CreateFile => "輸入要建立的檔案路徑（相對於工作區）",
        };
        self.menu_bar.close();
        self.overlay = Some(OverlayState::InputPrompt(InputPromptState::new(
            title,
            placeholder,
            action,
            suggestion,
        )));
        self.status_message = format!("{}：等待輸入", title);
    }

    fn command_palette_entries(&self) -> Vec<CommandPaletteEntry> {
        let mut entries = Vec::new();
        entries.push(CommandPaletteEntry::new(
            "新增未命名檔案",
            Some(String::from("Ctrl+N")),
            CommandAction::NewDocument,
        ));
        entries.push(CommandPaletteEntry::new(
            "新增檔案（指定路徑）",
            None,
            CommandAction::CreateFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "開啟檔案…",
            Some(String::from("Ctrl+O")),
            CommandAction::OpenFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "儲存檔案",
            Some(String::from("Ctrl+S")),
            CommandAction::SaveFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "另存新檔…",
            Some(String::from("Ctrl+Alt+S")),
            CommandAction::SaveFileAs,
        ));
        entries.push(CommandPaletteEntry::new(
            "刪除檔案…",
            Some(String::from("Delete")),
            CommandAction::DeleteFile,
        ));
        entries.push(CommandPaletteEntry::new(
            if self.file_tree.show_hidden() {
                "隱藏隱藏檔案"
            } else {
                "顯示隱藏檔案"
            },
            None,
            CommandAction::ToggleHiddenFiles,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("切換換行（目前：{}）", self.preferences.wrap_mode.label()),
            None,
            CommandAction::ToggleWrap,
        ));
        entries.push(CommandPaletteEntry::new(
            format!(
                "切換換行符（目前：{}）",
                self.preferences.line_ending.label()
            ),
            None,
            CommandAction::ToggleLineEnding,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("切換編碼（目前：{}）", self.preferences.encoding.label()),
            None,
            CommandAction::ToggleEncoding,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("切換縮排（目前：{}）", self.preferences.indent.label()),
            None,
            CommandAction::CycleIndent,
        ));
        entries.push(CommandPaletteEntry::new(
            "切換檔案樹",
            None,
            CommandAction::ToggleFileTree,
        ));
        entries.push(CommandPaletteEntry::new(
            "切換編輯器",
            None,
            CommandAction::ToggleEditor,
        ));
        entries.push(CommandPaletteEntry::new(
            "切換終端機",
            None,
            CommandAction::ToggleTerminal,
        ));
        entries.push(CommandPaletteEntry::new(
            "切換代理面板",
            None,
            CommandAction::ToggleAgent,
        ));
        entries
    }

    fn open_command_palette(&mut self) {
        self.menu_bar.close();
        let entries = self.command_palette_entries();
        self.overlay = Some(OverlayState::CommandPalette(CommandPaletteState::new(
            entries,
        )));
        self.status_message = String::from("指令面板：輸入關鍵字以篩選");
    }

    fn close_overlay(&mut self) {
        self.overlay = None;
    }

    fn delete_via_prompt(&mut self) {
        self.menu_bar.close();
        if let Some(path) = self.selected_file_for_actions() {
            self.request_delete(path);
        } else {
            self.status_message = String::from("目前沒有可刪除的檔案");
        }
    }

    fn selected_file_for_actions(&self) -> Option<PathBuf> {
        if let Some(entry) = self.file_tree.selected_entry() {
            if entry.kind == FileEntryKind::File {
                return Some(entry.path.clone());
            }
        }
        self.editor
            .file_path()
            .map(|path| self.canonicalize_path(path))
    }

    fn delete_selected_from_tree(&mut self) {
        if let Some(entry) = self.file_tree.selected_entry() {
            match entry.kind {
                FileEntryKind::File => self.request_delete(entry.path.clone()),
                FileEntryKind::Directory => {
                    let display = self.format_display_path(&entry.path);
                    self.status_message = format!("暫不支援刪除資料夾：{}", display);
                }
                _ => {
                    self.status_message = String::from("此項目無法刪除");
                }
            }
        } else {
            self.status_message = String::from("目前沒有選取項目");
        }
    }

    fn request_delete(&mut self, path: PathBuf) {
        let target = self.canonicalize_path(&path);
        match fs::metadata(&target) {
            Ok(meta) => {
                if !meta.is_file() {
                    let display = self.format_display_path(&target);
                    self.status_message = format!("暫不支援刪除資料夾：{}", display);
                    return;
                }
            }
            Err(err) => {
                self.status_message = format!("無法讀取檔案資訊：{err}");
                return;
            }
        }

        if self.suppress_delete_confirm {
            self.finalize_delete(target, false);
            return;
        }

        let display = self.format_display_path(&target);
        self.overlay = Some(OverlayState::ConfirmDelete(ConfirmDeleteState::new(
            target,
            display.clone(),
        )));
        self.status_message = format!("確認刪除：{}", display);
    }

    fn finalize_delete(&mut self, path: PathBuf, suppress_future: bool) {
        if suppress_future {
            self.suppress_delete_confirm = true;
        }

        let display = self.format_display_path(&path);
        match fs::metadata(&path) {
            Ok(meta) => {
                if !meta.is_file() {
                    self.status_message = format!("刪除失敗：{} 不是檔案", display);
                    return;
                }
            }
            Err(err) => {
                self.status_message = format!("刪除失敗（讀取資訊失敗）：{err}");
                return;
            }
        }

        match fs::remove_file(&path) {
            Ok(_) => {
                if self.editor.file_path().map(|p| self.canonicalize_path(p))
                    == Some(self.canonicalize_path(&path))
                {
                    self.editor.new_document();
                }
                self.file_tree.refresh();
                if self.is_in_workspace(&path) {
                    self.file_tree.set_selection(0);
                }
                self.status_message = format!("已刪除檔案：{}", display);
            }
            Err(err) => {
                self.status_message = format!("刪除失敗：{err}");
            }
        }
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) {
        if let Some(OverlayState::ConfirmDelete(state)) = self.overlay.as_mut() {
            enum Decision {
                None,
                Confirm,
                Cancel,
            }
            let decision = {
                let mut result = Decision::None;
                match key.code {
                    KeyCode::Esc => result = Decision::Cancel,
                    KeyCode::Left | KeyCode::Up => state.select(0),
                    KeyCode::Right | KeyCode::Down => state.select(1),
                    KeyCode::Tab | KeyCode::BackTab => state.toggle_selection(),
                    KeyCode::Char(' ') => {
                        state.suppress_future = !state.suppress_future;
                    }
                    KeyCode::Enter => {
                        result = if state.confirm_selected() {
                            Decision::Confirm
                        } else {
                            Decision::Cancel
                        };
                    }
                    _ => {}
                }
                result
            };
            let suppress = state.suppress_future;
            let path = state.target.clone();
            let display = state.display.clone();
            let _ = state;
            match decision {
                Decision::Confirm => {
                    self.close_overlay();
                    self.finalize_delete(path, suppress);
                }
                Decision::Cancel => {
                    if suppress {
                        self.suppress_delete_confirm = true;
                    }
                    self.close_overlay();
                    self.status_message = format!("已取消刪除：{}", display);
                }
                Decision::None => {}
            }
            return;
        }

        if let Some(OverlayState::CommandPalette(state)) = self.overlay.as_mut() {
            let (close, action) = {
                let mut close = false;
                let mut action = None;
                match key.code {
                    KeyCode::Esc => close = true,
                    KeyCode::Up => {
                        state.move_selection(-1);
                    }
                    KeyCode::Down => {
                        state.move_selection(1);
                    }
                    KeyCode::PageUp => {
                        state.move_selection(-5);
                    }
                    KeyCode::PageDown => {
                        state.move_selection(5);
                    }
                    KeyCode::Backspace => {
                        state.filter.pop();
                        state.update_filter();
                    }
                    KeyCode::Char(ch) => {
                        if !(key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT))
                        {
                            state.filter.push(ch);
                            state.update_filter();
                        }
                    }
                    KeyCode::Enter => {
                        action = state.selected_entry().map(|entry| entry.action);
                    }
                    KeyCode::Tab => {
                        state.move_selection(1);
                    }
                    KeyCode::BackTab => {
                        state.move_selection(-1);
                    }
                    _ => {}
                }
                if let Some(entry) = state.selected_entry() {
                    self.status_message = format!("指令：{}", entry.label);
                } else if state.visible.is_empty() {
                    self.status_message = String::from("指令：無符合項目");
                }
                (close, action)
            };
            if let Some(action) = action {
                self.close_overlay();
                self.execute_command_action(action);
            } else if close {
                self.close_overlay();
            }
            return;
        }

        if let Some(OverlayState::InputPrompt(state)) = self.overlay.as_mut() {
            let (close, submit) = {
                let mut close = false;
                let mut submit = false;
                match key.code {
                    KeyCode::Esc => close = true,
                    KeyCode::Enter => submit = true,
                    KeyCode::Backspace => {
                        state.value.pop();
                        state.error = None;
                    }
                    KeyCode::Char(ch) => {
                        if !(key.modifiers.contains(KeyModifiers::CONTROL)
                            || key.modifiers.contains(KeyModifiers::ALT))
                        {
                            state.value.push(ch);
                            state.error = None;
                        }
                    }
                    KeyCode::Tab => {}
                    _ => {}
                }
                (close, submit)
            };
            if submit {
                self.complete_input_prompt();
            } else if close {
                self.close_overlay();
            }
        }
    }

    fn execute_pending_input(
        &mut self,
        action: PendingInputAction,
        path: PathBuf,
    ) -> Result<(), String> {
        match action {
            PendingInputAction::OpenFile => {
                if !path.exists() {
                    return Err(format!("檔案不存在：{}", path.display()));
                }
                if path.is_dir() {
                    return Err(String::from("指定路徑為資料夾"));
                }
                let canonical = self.canonicalize_path(&path);
                self.perform_open_file(canonical)
            }
            PendingInputAction::SaveAs => self.perform_save_as(path),
            PendingInputAction::CreateFile => self.perform_create_file(path),
        }
    }

    fn complete_input_prompt(&mut self) {
        let (action, input) = match self.overlay.as_mut() {
            Some(OverlayState::InputPrompt(state)) => {
                let action = state.action;
                let input = state.value.clone();
                state.error = None;
                (action, input)
            }
            _ => return,
        };

        let result = match self.resolve_input_path(&input) {
            Ok(path) => self.execute_pending_input(action, path),
            Err(msg) => Err(msg),
        };

        match result {
            Ok(_) => {
                self.close_overlay();
            }
            Err(message) => {
                if let Some(OverlayState::InputPrompt(state)) = self.overlay.as_mut() {
                    state.error = Some(message.clone());
                }
                self.status_message = message;
            }
        }
    }

    fn execute_command_action(&mut self, action: CommandAction) {
        match action {
            CommandAction::NewDocument => self.perform_new_document(),
            CommandAction::CreateFile => {
                self.prompt_input(
                    PendingInputAction::CreateFile,
                    "建立新檔案",
                    self.suggest_current_path(),
                );
            }
            CommandAction::OpenFile => {
                self.prompt_input(
                    PendingInputAction::OpenFile,
                    "開啟檔案",
                    self.suggest_current_path(),
                );
            }
            CommandAction::SaveFile => {
                let _ = self.perform_save_current();
            }
            CommandAction::SaveFileAs => {
                self.prompt_input(
                    PendingInputAction::SaveAs,
                    "另存新檔",
                    self.suggest_current_path(),
                );
            }
            CommandAction::ToggleHiddenFiles => {
                self.toggle_hidden_files();
            }
            CommandAction::DeleteFile => {
                self.delete_via_prompt();
            }
            CommandAction::ToggleWrap => self.toggle_wrap_mode(),
            CommandAction::ToggleLineEnding => self.toggle_line_ending(),
            CommandAction::ToggleEncoding => self.toggle_encoding(),
            CommandAction::CycleIndent => self.cycle_indent_kind(),
            CommandAction::ToggleFileTree => self.toggle_pane(PaneKind::FileTree),
            CommandAction::ToggleEditor => self.toggle_pane(PaneKind::Editor),
            CommandAction::ToggleTerminal => self.toggle_pane(PaneKind::Terminal),
            CommandAction::ToggleAgent => self.toggle_pane(PaneKind::Agent),
        }
    }

    fn ensure_focus_available(&mut self) {
        let order = self.visible_focus_order();
        if !order.contains(&self.focus) {
            if let Some(first) = order.first() {
                self.focus = *first;
            }
        }
    }

    fn toggle_pane(&mut self, pane: PaneKind) {
        match pane {
            PaneKind::FileTree => {
                self.layout.tree_visible = !self.layout.tree_visible;
                let state = if self.layout.tree_visible {
                    "顯示"
                } else {
                    "隱藏"
                };
                self.status_message = format!("檔案樹已{}", state);
            }
            PaneKind::Agent => {
                self.layout.agent_visible = !self.layout.agent_visible;
                let state = if self.layout.agent_visible {
                    "顯示"
                } else {
                    "隱藏"
                };
                self.status_message = format!("代理面板已{}", state);
            }
            PaneKind::Editor => {
                if self.layout.editor_visible && !self.layout.terminal_visible {
                    self.status_message = String::from("終端機已隱藏，至少保留一個中央視窗");
                    return;
                }
                self.layout.editor_visible = !self.layout.editor_visible;
                let state = if self.layout.editor_visible {
                    "顯示"
                } else {
                    "隱藏"
                };
                self.status_message = format!("編輯器已{}", state);
            }
            PaneKind::Terminal => {
                if self.layout.terminal_visible && !self.layout.editor_visible {
                    self.status_message = String::from("編輯器已隱藏，至少保留一個中央視窗");
                    return;
                }
                self.layout.terminal_visible = !self.layout.terminal_visible;
                let state = if self.layout.terminal_visible {
                    "顯示"
                } else {
                    "隱藏"
                };
                self.status_message = format!("終端機已{}", state);
            }
        }
        self.ensure_focus_available();
    }

    fn mouse_select_file_tree(&mut self, column: u16, row: u16) {
        if let Some(geom) = self.layout.pane_geometry(PaneKind::FileTree) {
            if row <= geom.area.y || row >= geom.area.y.saturating_add(geom.area.height) - 1 {
                return;
            }
            if column < geom.area.x || column >= geom.area.x + geom.area.width {
                return;
            }
            let anchor = geom.area.y.saturating_add(1);
            let list_row = row.saturating_sub(anchor) as usize;
            self.file_tree.set_selection(list_row);
            if let Some(entry) = self.file_tree.selected_entry() {
                self.status_message = match entry.kind {
                    FileEntryKind::ParentLink => String::from("選取：../"),
                    FileEntryKind::WorkspaceRoot => String::from("選取：./"),
                    _ => format!("選取：{}", self.format_display_path(&entry.path)),
                };
            }
        }
    }

    fn mouse_select_agent(&mut self, column: u16, row: u16) {
        if let Some(geom) = self.layout.pane_geometry(PaneKind::Agent) {
            if row <= geom.area.y || row >= geom.area.y.saturating_add(geom.area.height) - 1 {
                return;
            }
            if column < geom.area.x || column >= geom.area.x + geom.area.width {
                return;
            }
            let anchor = geom.area.y.saturating_add(1);
            let list_row = row.saturating_sub(anchor) as usize;
            self.agent.set_selection(list_row);
            if let Some(msg) = self.agent.selected_message() {
                self.status_message = format!("選取代理建議：{}", msg.title);
            }
        }
    }

    fn update_drag(&mut self, target: DividerKind, column: u16, row: u16) {
        match target {
            DividerKind::TreeCenter => {
                if !self.layout.tree_visible {
                    return;
                }
                let workspace = self.layout.workspace;
                let total_width = workspace.width.max(1) as f32;
                let relative = column.saturating_sub(workspace.x) as f32 / total_width;
                let ratio = relative.clamp(0.1, 0.6);
                self.layout.tree_ratio = ratio;
                self.status_message = format!("檔案樹寬度：{:.0}%", ratio * 100.0);
            }
            DividerKind::CenterAgent => {
                if !self.layout.agent_visible {
                    return;
                }
                let workspace = self.layout.workspace;
                let total_width = workspace.width.max(1) as f32;
                let right_edge = workspace.x + workspace.width;
                let relative = right_edge.saturating_sub(column) as f32 / total_width;
                let ratio = relative.clamp(0.1, 0.5);
                self.layout.agent_ratio = ratio;
                self.status_message = format!("代理面板寬度：{:.0}%", ratio * 100.0);
            }
            DividerKind::EditorTerminal => {
                if !(self.layout.editor_visible && self.layout.terminal_visible) {
                    return;
                }
                if let Some(center) = self.layout.center_area() {
                    let total_height = center.height.max(1) as f32;
                    let relative = row.saturating_sub(center.y) as f32 / total_height;
                    let ratio = relative.clamp(0.2, 0.9);
                    self.layout.editor_ratio = ratio;
                    self.status_message = format!("編輯器高度：{:.0}%", ratio * 100.0);
                }
            }
        }
    }

    pub fn handle_mouse(&mut self, event: MouseEvent) {
        if self.overlay.is_some() {
            return;
        }

        const DOUBLE_CLICK_DURATION: Duration = Duration::from_millis(500);
        let column = event.column;
        let row = event.row;

        if let MouseEventKind::Down(MouseButton::Left) = event.kind {
            let now = Instant::now();
            let mut is_double_click = false;
            if let Some((last_time, last_col, last_row)) = self.last_click {
                if now.duration_since(last_time) < DOUBLE_CLICK_DURATION
                    && last_col == column
                    && last_row == row
                {
                    is_double_click = true;
                }
            }

            if is_double_click {
                if self.focus == FocusArea::FileTree {
                    if let Some(entry) = self.file_tree.selected_entry() {
                        match entry.kind {
                            FileEntryKind::Directory => self.file_tree.toggle_selected_directory(),
                            FileEntryKind::File => self.activate_file_tree_selection(),
                            _ => {}
                        }
                    }
                }
                self.last_click = None; // Reset after double click
            } else {
                self.last_click = Some((now, column, row));
            }

            if is_double_click {
                return; // Consume the event
            }
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.editor_drag_selecting = false;
                if let Some(kind) = self.status_controls.hit_test(column, row) {
                    if self.handle_status_control_click(kind) {
                        self.menu_bar.close();
                        return;
                    }
                }
                let mut menu_interacted = false;
                if let Some(menu_idx) = self.menu_bar.layout.hit_item(column, row) {
                    menu_interacted = true;
                    if self.menu_bar.open && self.menu_bar.active_index == Some(menu_idx) {
                        self.menu_bar.close();
                    } else {
                        self.menu_bar.open(menu_idx);
                        if let Some(item) = self.menu_bar.items.get(menu_idx) {
                            self.status_message = format!("選單：{}", item.title);
                        }
                        self.update_menu_hover_message();
                    }
                } else if self.menu_bar.open {
                    if let Some(active) = self.menu_bar.active_index {
                        if let Some(entry_idx) = self.menu_bar.layout.hit_entry(active, column, row)
                        {
                            self.menu_bar.highlighted_entry = Some(entry_idx);
                            self.update_menu_hover_message();
                            menu_interacted = true;
                        } else {
                            self.menu_bar.highlighted_entry = None;
                            self.update_menu_hover_message();
                        }
                    }
                }
                if menu_interacted {
                    return;
                } else if self.menu_bar.open {
                    self.menu_bar.close();
                }
                if let Some(divider) = self.layout.hit_test_divider(column, row) {
                    self.layout.start_drag(divider, column, row);
                    return;
                }
                if let Some(pane) = self.layout.hit_test_header(column, row) {
                    self.toggle_pane(pane);
                    return;
                }
                self.editor_drag_selecting = false;
                if let Some(pane) = self.layout.hit_test_body(column, row) {
                    self.focus = match pane {
                        PaneKind::FileTree => FocusArea::FileTree,
                        PaneKind::Editor => FocusArea::Editor,
                        PaneKind::Terminal => FocusArea::Terminal,
                        PaneKind::Agent => FocusArea::Agent,
                    };
                    match pane {
                        PaneKind::FileTree => {
                            self.editor_drag_selecting = false;
                            self.mouse_select_file_tree(column, row)
                        }
                        PaneKind::Agent => {
                            self.editor_drag_selecting = false;
                            self.mouse_select_agent(column, row)
                        }
                        PaneKind::Terminal => {
                            self.editor_drag_selecting = false;
                        }
                        PaneKind::Editor => {
                            self.move_editor_cursor_with_mouse(column, row, false);
                            self.editor.start_selection_anchor();
                            self.editor_hover_line = Some(self.editor.cursor().0);
                            self.editor_drag_selecting = true;
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(drag) = self.layout.drag_state() {
                    self.update_drag(drag, column, row);
                } else if self.editor_drag_selecting {
                    self.move_editor_cursor_with_mouse(column, row, true);
                    self.editor_hover_line = self.editor_line_at_visual_row(row);
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.menu_bar.open {
                    if let Some(active) = self.menu_bar.active_index {
                        if let Some(entry_idx) = self.menu_bar.layout.hit_entry(active, column, row)
                        {
                            self.menu_bar.highlighted_entry = Some(entry_idx);
                            if let Some(action) = self.menu_bar.highlighted_action() {
                                self.execute_menu_action(action);
                            }
                            self.menu_bar.close();
                        } else {
                            self.menu_bar.highlighted_entry = None;
                            self.update_menu_hover_message();
                        }
                    }
                }
                if self.editor_drag_selecting {
                    if !self.editor.has_selection() {
                        self.editor.clear_selection();
                    }
                    self.editor_drag_selecting = false;
                }
                self.layout.clear_drag();
            }
            MouseEventKind::ScrollUp => {
                if let Some(pane) = self.layout.hit_test_body(column, row) {
                    match pane {
                        PaneKind::FileTree => {
                            self.file_tree.move_selection(-1);
                            if let Some(entry) = self.file_tree.selected_entry() {
                                self.status_message = match entry.kind {
                                    FileEntryKind::ParentLink => String::from("選取：../"),
                                    FileEntryKind::WorkspaceRoot => String::from("選取：./"),
                                    _ => {
                                        format!("選取：{}", self.format_display_path(&entry.path))
                                    }
                                };
                            }
                        }
                        PaneKind::Terminal => self.terminal.scroll(-3),
                        PaneKind::Agent => {
                            self.agent.move_selection(-1);
                            if let Some(msg) = self.agent.selected_message() {
                                self.status_message = format!("選取代理建議：{}", msg.title);
                            }
                        }
                        PaneKind::Editor => {}
                    }
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(pane) = self.layout.hit_test_body(column, row) {
                    match pane {
                        PaneKind::FileTree => {
                            self.file_tree.move_selection(1);
                            if let Some(entry) = self.file_tree.selected_entry() {
                                self.status_message = match entry.kind {
                                    FileEntryKind::ParentLink => String::from("選取：../"),
                                    FileEntryKind::WorkspaceRoot => String::from("選取：./"),
                                    _ => {
                                        format!("選取：{}", self.format_display_path(&entry.path))
                                    }
                                };
                            }
                        }
                        PaneKind::Terminal => self.terminal.scroll(3),
                        PaneKind::Agent => {
                            self.agent.move_selection(1);
                            if let Some(msg) = self.agent.selected_message() {
                                self.status_message = format!("選取代理建議：{}", msg.title);
                            }
                        }
                        PaneKind::Editor => {}
                    }
                }
            }
            MouseEventKind::Moved => {
                if self.menu_bar.open {
                    if let Some(menu_idx) = self.menu_bar.layout.hit_item(column, row) {
                        if Some(menu_idx) != self.menu_bar.active_index {
                            self.menu_bar.open(menu_idx);
                            if let Some(item) = self.menu_bar.items.get(menu_idx) {
                                self.status_message = format!("選單：{}", item.title);
                            }
                            self.update_menu_hover_message();
                        }
                    }
                    if let Some(active) = self.menu_bar.active_index {
                        let hover = self.menu_bar.layout.hit_entry(active, column, row);
                        self.menu_bar.highlighted_entry = hover;
                        if hover.is_some() {
                            self.update_menu_hover_message();
                        } else if let Some(item) = self.menu_bar.items.get(active) {
                            self.status_message = format!("選單：{}", item.title);
                        }
                    }
                }

                self.editor_hover_line = self.editor_line_at_visual_row(row);
            }
            _ => {}
        }
        self.ensure_focus_available();
    }
}
const EDITOR_GUTTER_WIDTH: u16 = 7;
