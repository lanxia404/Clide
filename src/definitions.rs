use ratatui::layout::Rect;
use std::collections::HashMap;

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
            FocusArea::FileTree => "檔案樹",
            FocusArea::Editor => "編輯器",
            FocusArea::Terminal => "終端機",
            FocusArea::Agent => "代理面板",
        }
    }
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
pub struct DragState {
    pub target: DividerKind,
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
    pub pane_geometries: HashMap<PaneKind, PaneGeometry>,
    pub divider_geometries: HashMap<DividerKind, Rect>,
    pub workspace: Rect,
    pub center_area: Option<Rect>,
    pub drag_state: Option<DragState>,
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

    pub fn start_drag(&mut self, kind: DividerKind, _column: u16, _row: u16) {
        self.drag_state = Some(DragState { target: kind });
    }

    pub fn drag_state(&self) -> Option<DividerKind> {
        self.drag_state.map(|state| state.target)
    }

    pub fn clear_drag(&mut self) {
        self.drag_state = None;
    }
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
    HiddenFiles,
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
    New,
    CreateFile,
    Open,
    Save,
    SaveAs,
    ToggleHiddenFiles,
    Delete,
    CommandPalette,
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
    pub item_areas: Vec<Rect>,
    pub entry_areas: HashMap<(usize, usize), Rect>,
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
                        label: "新增空白檔案",
                        action: MenuAction::New,
                    },
                    MenuEntry {
                        label: "建立檔案…",
                        action: MenuAction::CreateFile,
                    },
                    MenuEntry {
                        label: "開啟…",
                        action: MenuAction::Open,
                    },
                    MenuEntry {
                        label: "儲存",
                        action: MenuAction::Save,
                    },
                    MenuEntry {
                        label: "另存新檔…",
                        action: MenuAction::SaveAs,
                    },
                    MenuEntry {
                        label: "切換隱藏檔案",
                        action: MenuAction::ToggleHiddenFiles,
                    },
                    MenuEntry {
                        label: "刪除檔案…",
                        action: MenuAction::Delete,
                    },
                    MenuEntry {
                        label: "指令面板…",
                        action: MenuAction::CommandPalette,
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

pub fn rect_contains(rect: &Rect, column: u16, row: u16) -> bool {
    column >= rect.x
        && column < rect.x.saturating_add(rect.width)
        && row >= rect.y
        && row < rect.y.saturating_add(rect.height)
}
