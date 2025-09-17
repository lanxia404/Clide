use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crate::editor::Editor;
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusArea {
    FileTree,
    Editor,
    Terminal,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneKind {
    FileTree,
    Editor,
    Terminal,
    Agent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DividerKind {
    TreeCenter,
    CenterAgent,
    EditorTerminal,
}

#[derive(Clone, Copy, Default)]
pub struct PaneGeometry {
    pub area: Rect,
    pub header: Rect,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    target: DividerKind,
}

#[derive(Default)]
pub struct LayoutState {
    pub tree_visible: bool,
    pub agent_visible: bool,
    pub editor_visible: bool,
    pub terminal_visible: bool,
    pub tree_ratio: f32,
    pub agent_ratio: f32,
    pub editor_ratio: f32,
    pane_geometries: HashMap<PaneKind, PaneGeometry>,
    divider_geometries: HashMap<DividerKind, Rect>,
    workspace: Rect,
    center_area: Option<Rect>,
    drag_state: Option<DragState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WrapMode {
    NoWrap,
    WordWrap,
}

impl WrapMode {
    pub fn label(&self) -> &'static str {
        match self {
            WrapMode::NoWrap => "單行",
            WrapMode::WordWrap => "自動換行",
        }
    }

    pub fn abbr(&self) -> &'static str {
        match self {
            WrapMode::NoWrap => "OFF",
            WrapMode::WordWrap => "ON",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            WrapMode::NoWrap => WrapMode::WordWrap,
            WrapMode::WordWrap => WrapMode::NoWrap,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
}

impl LineEnding {
    pub fn label(&self) -> &'static str {
        match self {
            LineEnding::Lf => "LF",
            LineEnding::CrLf => "CRLF",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            LineEnding::Lf => LineEnding::CrLf,
            LineEnding::CrLf => LineEnding::Lf,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodingKind {
    Utf8,
    Utf16,
}

impl EncodingKind {
    pub fn label(&self) -> &'static str {
        match self {
            EncodingKind::Utf8 => "UTF-8",
            EncodingKind::Utf16 => "UTF-16",
        }
    }

    pub fn abbr(&self) -> &'static str {
        match self {
            EncodingKind::Utf8 => "UTF8",
            EncodingKind::Utf16 => "UTF16",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            EncodingKind::Utf8 => EncodingKind::Utf16,
            EncodingKind::Utf16 => EncodingKind::Utf8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndentKind {
    Spaces(usize),
    Tabs,
}

impl IndentKind {
    pub fn label(&self) -> String {
        match self {
            IndentKind::Spaces(width) => format!("空白×{}", width),
            IndentKind::Tabs => String::from("Tab"),
        }
    }

    pub fn abbr(&self) -> String {
        match self {
            IndentKind::Spaces(width) => format!("SP{}", width),
            IndentKind::Tabs => String::from("TAB"),
        }
    }

    pub fn next(self) -> Self {
        match self {
            IndentKind::Spaces(2) => IndentKind::Spaces(4),
            IndentKind::Spaces(4) => IndentKind::Tabs,
            IndentKind::Tabs => IndentKind::Spaces(2),
            IndentKind::Spaces(other) => {
                if other <= 2 {
                    IndentKind::Spaces(4)
                } else {
                    IndentKind::Tabs
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct EditorPreferences {
    pub wrap_mode: WrapMode,
    pub line_ending: LineEnding,
    pub encoding: EncodingKind,
    pub indent: IndentKind,
}

impl EditorPreferences {
    pub fn new() -> Self {
        Self {
            wrap_mode: WrapMode::NoWrap,
            line_ending: LineEnding::Lf,
            encoding: EncodingKind::Utf8,
            indent: IndentKind::Spaces(4),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusControlKind {
    Wrap,
    LineEnding,
    Encoding,
    Indent,
    Cursor,
    Dirty,
}

#[derive(Default, Debug, Clone)]
pub struct StatusControlRegistry {
    entries: HashMap<StatusControlKind, Rect>,
}

impl StatusControlRegistry {
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn register(&mut self, kind: StatusControlKind, area: Rect) {
        self.entries.insert(kind, area);
    }

    pub fn hit_test(&self, column: u16, row: u16) -> Option<StatusControlKind> {
        self.entries.iter().find_map(|(kind, rect)| {
            if rect_contains(rect, column, row) {
                Some(*kind)
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    ToggleWrap,
    ToggleLineEnding,
    ToggleEncoding,
    CycleIndent,
    ToggleFileTree,
    ToggleEditor,
    ToggleTerminal,
    ToggleAgent,
    Open,
    Save,
    Exit,
    None,
}

#[derive(Debug, Clone)]
pub struct MenuEntry {
    pub label: &'static str,
    pub action: MenuAction,
}

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub title: &'static str,
    pub entries: Vec<MenuEntry>,
}

#[derive(Debug, Default, Clone)]
pub struct MenuLayout {
    item_areas: Vec<Rect>,
    entry_areas: HashMap<(usize, usize), Rect>,
}

impl MenuLayout {
    pub fn reset(&mut self, item_count: usize) {
        self.item_areas.clear();
        self.entry_areas.clear();
        self.item_areas.resize(item_count, Rect::default());
    }

    pub fn register_item(&mut self, index: usize, rect: Rect) {
        if let Some(slot) = self.item_areas.get_mut(index) {
            *slot = rect;
        }
    }

    pub fn item_area(&self, index: usize) -> Option<Rect> {
        self.item_areas.get(index).copied()
    }

    pub fn register_entry(&mut self, item_index: usize, entry_index: usize, rect: Rect) {
        self.entry_areas.insert((item_index, entry_index), rect);
    }

    pub fn hit_item(&self, column: u16, row: u16) -> Option<usize> {
        self.item_areas
            .iter()
            .enumerate()
            .find(|(_, rect)| rect_contains(rect, column, row))
            .map(|(idx, _)| idx)
    }

    pub fn hit_entry(&self, item_index: usize, column: u16, row: u16) -> Option<usize> {
        self.entry_areas
            .iter()
            .filter_map(|((item, entry), rect)| {
                if *item == item_index && rect_contains(rect, column, row) {
                    Some(*entry)
                } else {
                    None
                }
            })
            .next()
    }
}

#[derive(Debug, Clone)]
pub struct MenuBar {
    pub items: Vec<MenuItem>,
    pub active_index: Option<usize>,
    pub highlighted_entry: Option<usize>,
    pub open: bool,
    pub layout: MenuLayout,
}

impl MenuBar {
    pub fn new() -> Self {
        let items = vec![
            MenuItem {
                title: "檔案",
                entries: vec![
                    MenuEntry {
                        label: "開啟…",
                        action: MenuAction::Open,
                    },
                    MenuEntry {
                        label: "儲存",
                        action: MenuAction::Save,
                    },
                    MenuEntry {
                        label: "離開",
                        action: MenuAction::Exit,
                    },
                ],
            },
            MenuItem {
                title: "編輯",
                entries: vec![
                    MenuEntry {
                        label: "復原",
                        action: MenuAction::None,
                    },
                    MenuEntry {
                        label: "重做",
                        action: MenuAction::None,
                    },
                ],
            },
            MenuItem {
                title: "搜尋",
                entries: vec![MenuEntry {
                    label: "尋找",
                    action: MenuAction::None,
                }],
            },
            MenuItem {
                title: "格式",
                entries: vec![
                    MenuEntry {
                        label: "切換換行",
                        action: MenuAction::ToggleWrap,
                    },
                    MenuEntry {
                        label: "切換換行符",
                        action: MenuAction::ToggleLineEnding,
                    },
                    MenuEntry {
                        label: "切換編碼",
                        action: MenuAction::ToggleEncoding,
                    },
                    MenuEntry {
                        label: "切換縮排",
                        action: MenuAction::CycleIndent,
                    },
                ],
            },
            MenuItem {
                title: "視窗",
                entries: vec![
                    MenuEntry {
                        label: "切換檔案樹",
                        action: MenuAction::ToggleFileTree,
                    },
                    MenuEntry {
                        label: "切換編輯器",
                        action: MenuAction::ToggleEditor,
                    },
                    MenuEntry {
                        label: "切換終端機",
                        action: MenuAction::ToggleTerminal,
                    },
                    MenuEntry {
                        label: "切換代理面板",
                        action: MenuAction::ToggleAgent,
                    },
                ],
            },
            MenuItem {
                title: "說明",
                entries: vec![MenuEntry {
                    label: "關於",
                    action: MenuAction::None,
                }],
            },
        ];
        let mut layout = MenuLayout::default();
        layout.reset(items.len());
        Self {
            items,
            active_index: None,
            highlighted_entry: None,
            open: false,
            layout,
        }
    }

    pub fn open(&mut self, index: usize) {
        self.active_index = Some(index);
        let highlight = self.items.get(index).and_then(|item| {
            if item.entries.is_empty() {
                None
            } else {
                Some(0)
            }
        });
        self.highlighted_entry = highlight;
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.highlighted_entry = None;
        self.active_index = None;
    }

    pub fn move_active(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let len = self.items.len() as isize;
        let current = self.active_index.unwrap_or(0) as isize;
        let mut next = current + delta;
        if next < 0 {
            next += len;
        }
        next %= len;
        self.open(next as usize);
    }

    pub fn move_highlight(&mut self, delta: isize) {
        if let Some(active) = self.active_index {
            let entries = &self.items[active].entries;
            if entries.is_empty() {
                self.highlighted_entry = None;
                return;
            }
            let len = entries.len() as isize;
            let current = self.highlighted_entry.unwrap_or(0) as isize;
            let mut next = current + delta;
            if next < 0 {
                next += len;
            }
            next %= len;
            self.highlighted_entry = Some(next as usize);
        }
    }

    pub fn highlighted_action(&self) -> Option<MenuAction> {
        if let (Some(menu_idx), Some(entry_idx)) = (self.active_index, self.highlighted_entry) {
            self.items
                .get(menu_idx)
                .and_then(|menu| menu.entries.get(entry_idx))
                .map(|entry| entry.action)
        } else {
            None
        }
    }
}

impl LayoutState {
    pub fn new() -> Self {
        Self {
            tree_visible: true,
            agent_visible: true,
            editor_visible: true,
            terminal_visible: true,
            tree_ratio: 0.22,
            agent_ratio: 0.26,
            editor_ratio: 0.7,
            ..Default::default()
        }
    }

    pub fn begin_frame(&mut self, workspace: Rect) {
        self.workspace = workspace;
        self.pane_geometries.clear();
        self.divider_geometries.clear();
        self.center_area = None;
    }

    pub fn register_center_area(&mut self, area: Rect) {
        self.center_area = Some(area);
    }

    pub fn register_pane(&mut self, kind: PaneKind, area: Rect) {
        let header = Rect {
            x: area.x.saturating_add(1),
            y: area.y,
            width: area.width.saturating_sub(2),
            height: 1,
        };
        self.pane_geometries
            .insert(kind, PaneGeometry { area, header });
    }

    pub fn register_divider(&mut self, kind: DividerKind, rect: Rect) {
        self.divider_geometries.insert(kind, rect);
    }

    pub fn pane_geometry(&self, kind: PaneKind) -> Option<&PaneGeometry> {
        self.pane_geometries.get(&kind)
    }

    pub fn center_area(&self) -> Option<Rect> {
        self.center_area
    }

    pub fn hit_test_header(&self, column: u16, row: u16) -> Option<PaneKind> {
        self.pane_geometries
            .iter()
            .find(|(_, geometry)| rect_contains(&geometry.header, column, row))
            .map(|(kind, _)| *kind)
    }

    pub fn hit_test_body(&self, column: u16, row: u16) -> Option<PaneKind> {
        self.pane_geometries
            .iter()
            .find(|(_, geometry)| rect_contains(&geometry.area, column, row))
            .map(|(kind, _)| *kind)
    }

    pub fn hit_test_divider(&self, column: u16, row: u16) -> Option<DividerKind> {
        self.divider_geometries
            .iter()
            .find(|(_, rect)| rect_contains(rect, column, row))
            .map(|(kind, _)| *kind)
    }

    fn start_drag(&mut self, kind: DividerKind, _column: u16, _row: u16, _ratio: f32) {
        self.drag_state = Some(DragState { target: kind });
    }

    pub fn drag_state(&self) -> Option<DividerKind> {
        self.drag_state.map(|state| state.target)
    }

    pub fn clear_drag(&mut self) {
        self.drag_state = None;
    }
}

fn rect_contains(rect: &Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}

impl FocusArea {
    pub fn label(&self) -> &'static str {
        match self {
            FocusArea::FileTree => "檔案樹",
            FocusArea::Editor => "編輯器",
            FocusArea::Terminal => "終端機",
            FocusArea::Agent => "代理面板",
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
                    let ratio = match divider {
                        DividerKind::TreeCenter => self.layout.tree_ratio,
                        DividerKind::CenterAgent => self.layout.agent_ratio,
                        DividerKind::EditorTerminal => self.layout.editor_ratio,
                    };
                    self.layout.start_drag(divider, column, row, ratio);
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileEntryKind {
    Directory,
    File,
    WorkspaceRoot,
    ParentLink,
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub depth: usize,
    pub kind: FileEntryKind,
    pub expanded: bool,
    pub has_children: bool,
}

pub struct FileTree {
    entries: Vec<FileEntry>,
    selected: usize,
    current_dir: PathBuf,
    expanded_dirs: HashSet<PathBuf>,
}

impl FileTree {
    pub fn from_root(root: PathBuf) -> Self {
        let mut tree = Self {
            entries: Vec::new(),
            selected: 0,
            current_dir: root,
            expanded_dirs: HashSet::new(),
        };
        tree.refresh();
        tree
    }

    fn canonicalize(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn ensure_expanded_root(&mut self) {
        let root = self.canonicalize(&self.current_dir);
        self.expanded_dirs.insert(root);
    }

    fn append_directory_entries(
        &mut self,
        dir: &Path,
        depth: usize,
        entries: &mut Vec<FileEntry>,
        has_content: &mut bool,
    ) {
        let mut children = match fs::read_dir(dir) {
            Ok(read_dir) => read_dir.filter_map(|res| res.ok()).collect::<Vec<_>>(),
            Err(_) => return,
        };

        children.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            match b_is_dir.cmp(&a_is_dir) {
                std::cmp::Ordering::Equal => a.file_name().cmp(&b.file_name()),
                other => other,
            }
        });

        for child in children {
            let name = match child.file_name().to_str() {
                Some(name) if !name.starts_with('.') => name.to_string(),
                Some(_) => continue,
                None => continue,
            };

            let path = self.canonicalize(&child.path());
            let is_dir = child.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let display_depth = depth + 1;
            if is_dir {
                let expanded = self.expanded_dirs.contains(&path);
                entries.push(FileEntry {
                    name,
                    path: path.clone(),
                    depth: display_depth,
                    kind: FileEntryKind::Directory,
                    expanded,
                    has_children: true, // Always expandable
                });
                *has_content = true;
                if expanded {
                    self.append_directory_entries(&path, display_depth, entries, has_content);
                }
            } else {
                entries.push(FileEntry {
                    name,
                    path: path.clone(),
                    depth: display_depth,
                    kind: FileEntryKind::File,
                    expanded: false,
                    has_children: false,
                });
                *has_content = true;
            }
        }
    }

    pub fn refresh(&mut self) {
        let previously_selected = self.entries.get(self.selected).map(|entry| entry.path.clone());

        let current_canon = self.canonicalize(&self.current_dir);
        self.current_dir = current_canon.clone();
        let root_has_children = current_canon.is_dir();

        self.ensure_expanded_root();

        let mut entries = Vec::new();
        entries.push(FileEntry {
            name: String::from("./"),
            path: current_canon.clone(),
            depth: 0,
            kind: FileEntryKind::WorkspaceRoot,
            expanded: true,
            has_children: root_has_children,
        });

        let parent_target = self
            .current_dir
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| self.current_dir.clone());
        entries.push(FileEntry {
            name: String::from("../"),
            path: parent_target,
            depth: 0,
            kind: FileEntryKind::ParentLink,
            expanded: false,
            has_children: true,
        });

        let mut has_content = false;
        let dir_to_scan = self.current_dir.clone();
        self.append_directory_entries(&dir_to_scan, 0, &mut entries, &mut has_content);

        if !has_content {
            entries.push(FileEntry {
                name: String::from("(空目錄)"),
                path: self.current_dir.clone(),
                depth: 1,
                kind: FileEntryKind::File,
                expanded: false,
                has_children: false,
            });
        }

        if let Some(path) = previously_selected {
            if let Some(idx) = entries.iter().position(|entry| entry.path == path) {
                self.selected = idx;
            }
        }

        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn populate_with_placeholder(&mut self) {
        self.expanded_dirs.clear();
        let canon = self.canonicalize(&self.current_dir);
        self.expanded_dirs.insert(canon.clone());
        self.entries = vec![
            FileEntry {
                name: String::from("./"),
                path: canon.clone(),
                depth: 0,
                kind: FileEntryKind::WorkspaceRoot,
                expanded: true,
                has_children: true,
            },
            FileEntry {
                name: String::from("../"),
                path: canon.parent().map(|p| p.to_path_buf()).unwrap_or(canon.clone()),
                depth: 0,
                kind: FileEntryKind::ParentLink,
                expanded: false,
                has_children: true,
            },
            FileEntry {
                name: String::from("src/main.rs"),
                path: canon.join("src").join("main.rs"),
                depth: 1,
                kind: FileEntryKind::File,
                expanded: false,
                has_children: false,
            },
        ];
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

    pub fn set_selection(&mut self, index: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = index.min(self.entries.len().saturating_sub(1));
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

    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn toggle_selected_directory(&mut self) {
        if let Some(entry) = self.selected_entry().cloned() {
            if entry.kind == FileEntryKind::Directory && entry.has_children {
                if entry.expanded {
                    self.expanded_dirs.remove(&entry.path);
                } else {
                    self.expanded_dirs.insert(entry.path.clone());
                }
                self.refresh();
            }
        }
    }

    pub fn activate_selected(&mut self) -> FileTreeAction {
        if let Some(entry) = self.selected_entry().cloned() {
            match entry.kind {
                FileEntryKind::File => {
                    return FileTreeAction::OpenFile(entry.path);
                }
                FileEntryKind::Directory | FileEntryKind::ParentLink => {
                    if self.navigate_to(&entry.path) {
                        return FileTreeAction::ChangedDir(entry.path);
                    }
                }
                FileEntryKind::WorkspaceRoot => {
                    // no-op for current directory entry
                }
            }
        }
        FileTreeAction::None
    }

    fn navigate_to(&mut self, path: &PathBuf) -> bool {
        let target = self.canonicalize(path);
        if target == self.current_dir {
            return false;
        }
        if !target.is_dir() {
            return false;
        }
        self.current_dir = target.clone();
        self.expanded_dirs.insert(target);
        self.selected = 0;
        self.refresh();
        true
    }
}

pub enum FileTreeAction {
    OpenFile(PathBuf),
    ChangedDir(PathBuf),
    None,
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
                title: String::from("代理狀態"),
                detail: String::from("等待 AI 編輯建議"),
            },
            AgentMessage {
                title: String::from("變更預覽"),
                detail: String::from("main.rs 第 10 行新增 println!"),
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

    pub fn set_selection(&mut self, index: usize) {
        if self.messages.is_empty() {
            return;
        }
        self.selected = index.min(self.messages.len().saturating_sub(1));
    }
}
