use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crate::definitions::{
    DividerKind, EditorPreferences, FocusArea, IndentKind, LayoutState, MenuAction, MenuBar,
    PaneKind, StatusControlKind, StatusControlRegistry, rect_contains,
};
use crate::editor::Editor;
use crate::file_tree::{FileEntryKind, FileTree, FileTreeAction};
use crate::panels::{agent::AgentPanel, terminal::TerminalPane};



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
    last_tick: Instant,
    tick_rate: Duration,
    last_click: Option<(Instant, u16, u16)>,
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
            last_tick: Instant::now(),
            tick_rate: Duration::from_millis(250),
            last_click: None,
        })
    }


    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
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
                's' => self.status_message = "儲存功能尚未實作".into(),
                'a' => self.status_message = "代理請求尚未實作".into(),
                _ => {}
            },
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
            KeyCode::Enter => self.activate_file_tree_selection(),
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
            FileTreeAction::OpenFile(path) => match self.editor.open_file(&path) {
                Ok(_) => {
                    let display = self.format_workspace_path(&path);
                    self.status_message = format!("已開啟檔案：{}", display);
                }
                Err(err) => self.status_message = format!("開啟失敗：{err}"),
            },
            FileTreeAction::ChangedDir(path) => {
                let display = self.format_workspace_path(&path);
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
            MenuAction::Open => {
                self.status_message = String::from("開啟功能尚未實作");
            }
            MenuAction::Save => {
                self.status_message = String::from("儲存功能尚未實作");
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

    fn format_workspace_path(&self, path: &Path) -> String {
        if let Ok(relative) = path.strip_prefix(&self.workspace_root) {
            if relative.as_os_str().is_empty() {
                String::from("./")
            } else {
                format!("./{}", relative.display())
            }
        } else {
            path.display().to_string()
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
                    _ => format!("選取：{}", self.format_workspace_path(&entry.path)),
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
                if let Some(pane) = self.layout.hit_test_body(column, row) {
                    self.focus = match pane {
                        PaneKind::FileTree => FocusArea::FileTree,
                        PaneKind::Editor => FocusArea::Editor,
                        PaneKind::Terminal => FocusArea::Terminal,
                        PaneKind::Agent => FocusArea::Agent,
                    };
                    match pane {
                        PaneKind::FileTree => self.mouse_select_file_tree(column, row),
                        PaneKind::Agent => self.mouse_select_agent(column, row),
                        PaneKind::Terminal => {}
                        PaneKind::Editor => {}
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(drag) = self.layout.drag_state() {
                    self.update_drag(drag, column, row);
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
                                        format!("選取：{}", self.format_workspace_path(&entry.path))
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
                                        format!("選取：{}", self.format_workspace_path(&entry.path))
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

                self.editor_hover_line = None;
                if let Some(geom) = self.layout.pane_geometry(PaneKind::Editor) {
                    if rect_contains(&geom.area, column, row)
                        && row > geom.area.y
                        && row < geom.area.y.saturating_add(geom.area.height).saturating_sub(1)
                    {
                        let line_in_viewport = (row - geom.area.y - 1) as usize;
                        let absolute_line =
                            self.editor.viewport_start().saturating_add(line_in_viewport);
                        if absolute_line < self.editor.total_lines() {
                            self.editor_hover_line = Some(absolute_line);
                        }
                    }
                }
            }
            _ => {}
        }
        self.ensure_focus_available();
    }
}




