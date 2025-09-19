//! Defines the core state structures for the application.
//!
//! This module contains the central `App` struct that holds the entire state
//! of the TUI application. It also defines various sub-states for managing
//! different UI components like overlays (command palette, prompts), the agent
//! composer, and more.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::agent::AgentManager;
use crate::agent::config::{AgentCapabilities, AgentProfile, HttpProvider};
use crate::definitions::{
    EditorPreferences, FocusArea, LayoutState, MenuBar, StatusControlRegistry,
};
use crate::editor::Editor;
use crate::file_tree::FileTree;
use crate::panels::{agent::AgentPanel, terminal::TerminalPane};

/// The main application state.
///
/// This struct holds all the data required to render the UI and manage user
/// interactions. It is the single source of truth for the application's state.
pub struct App {
    // --- Core State ---
    /// Flag to indicate if the application should quit.
    pub should_quit: bool,
    /// The currently focused UI area.
    pub focus: FocusArea,
    /// The state of the text editor.
    pub editor: Editor,
    /// The state of the file tree panel.
    pub file_tree: FileTree,
    /// The state of the terminal panel.
    pub terminal: TerminalPane,
    /// The state of the agent chat panel.
    pub agent: AgentPanel,
    /// The state of the agent input composer/textbox.
    pub agent_input: AgentComposer,

    // --- UI & Layout ---
    /// The message currently displayed in the status bar.
    pub status_message: String,
    /// The root directory of the current workspace.
    pub workspace_root: PathBuf,
    /// The state of the overall UI layout (pane sizes, etc.).
    pub layout: LayoutState,
    /// User preferences for the editor.
    pub preferences: EditorPreferences,
    /// The state of the top menu bar.
    pub menu_bar: MenuBar,
    /// Registry for status bar controls.
    pub status_controls: StatusControlRegistry,
    /// The line number currently being hovered over in the editor.
    pub editor_hover_line: Option<usize>,
    /// The currently active overlay, if any.
    pub overlay: Option<OverlayState>,

    // --- Internal State & Flags ---
    /// If true, suppresses the confirmation prompt when deleting files.
    pub suppress_delete_confirm: bool,
    /// True if the user is currently drag-selecting text in the editor.
    pub(crate) editor_drag_selecting: bool,
    /// The timestamp of the last application tick.
    pub(crate) last_tick: Instant,
    /// The configured tick rate for the application.
    pub(crate) tick_rate: Duration,

    // --- Agent-related State ---
    /// The capabilities of the currently active agent.
    pub agent_capabilities: Option<AgentCapabilities>,
    /// The manager for handling agent processes and communication.
    pub(crate) agent_manager: Option<AgentManager>,
    /// A pending request to set up a new agent.
    pub(crate) pending_agent_setup: Option<AgentSetupRequest>,
}

/// Represents a request to set up a new HTTP-based agent.
#[derive(Clone)]
pub(crate) struct AgentSetupRequest {
    pub provider: HttpProvider,
    pub _instructions: String,
}

/// State for the text input composer, used for the agent chat input.
///
/// Manages the text buffer, cursor position, and command history.
#[derive(Clone, Default)]
pub struct AgentComposer {
    buffer: String,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
}

impl AgentComposer {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Inserts a character at the current cursor position.
    pub fn insert_char(&mut self, ch: char) {
        self.buffer.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
        self.reset_history_navigation();
    }

    /// Inserts a newline character.
    pub fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    /// Deletes the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some((idx, _)) = self.buffer[..self.cursor].char_indices().next_back() {
            self.buffer.drain(idx..self.cursor);
            self.cursor = idx;
            self.reset_history_navigation();
        }
    }

    /// Deletes the character at the cursor (delete).
    pub fn delete(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        if let Some((_, ch)) = self.buffer[self.cursor..].char_indices().next() {
            let end = self.cursor + ch.len_utf8();
            self.buffer.drain(self.cursor..end);
            self.reset_history_navigation();
        }
    }

    /// Moves the cursor one character to the left.
    pub fn move_left(&mut self) {
        if self.cursor == 0 {
            return;
        }
        if let Some((idx, _)) = self.buffer[..self.cursor].char_indices().next_back() {
            self.cursor = idx;
        } else {
            self.cursor = 0;
        }
        self.reset_history_navigation();
    }

    /// Moves the cursor one character to the right.
    pub fn move_right(&mut self) {
        if self.cursor >= self.buffer.len() {
            return;
        }
        if let Some((offset, ch)) = self.buffer[self.cursor..].char_indices().next() {
            self.cursor += offset + ch.len_utf8();
        } else {
            self.cursor = self.buffer.len();
        }
        self.reset_history_navigation();
    }

    /// Moves the cursor to the start of the current line.
    pub fn move_to_line_start(&mut self) {
        if let Some(pos) = self.buffer[..self.cursor].rfind('\n') {
            self.cursor = pos + 1;
        } else {
            self.cursor = 0;
        }
        self.reset_history_navigation();
    }

    /// Moves the cursor to the end of the current line.
    pub fn move_to_line_end(&mut self) {
        if let Some(pos) = self.buffer[self.cursor..].find('\n') {
            self.cursor += pos;
        } else {
            self.cursor = self.buffer.len();
        }
        self.reset_history_navigation();
    }

    /// Clears the entire input buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.reset_history_navigation();
    }

    /// Takes the content of the buffer, adds it to history, and clears the buffer.
    pub fn take(&mut self) -> String {
        let content = std::mem::take(&mut self.buffer);
        if !content.trim().is_empty() {
            self.history.push(content.clone());
        }
        self.cursor = 0;
        self.reset_history_navigation();
        content
    }

    /// Navigates to the previous entry in the command history.
    pub fn history_previous(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        let target = match self.history_index {
            Some(0) => 0,
            Some(idx) => idx.saturating_sub(1),
            None => self.history.len().saturating_sub(1),
        };
        self.load_history(target)
    }

    /// Navigates to the next entry in the command history.
    pub fn history_next(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }
        match self.history_index {
            Some(idx) if idx + 1 < self.history.len() => self.load_history(idx + 1),
            _ => {
                self.history_index = None;
                self.buffer.clear();
                self.cursor = 0;
                true
            }
        }
    }

    /// Loads a specific history entry into the buffer.
    fn load_history(&mut self, index: usize) -> bool {
        if let Some(entry) = self.history.get(index).cloned() {
            self.buffer = entry;
            self.cursor = self.buffer.len();
            self.history_index = Some(index);
            true
        } else {
            false
        }
    }

    /// Resets the history navigation state.
    fn reset_history_navigation(&mut self) {
        self.history_index = None;
    }

    /// Calculates the (col, row) position of the cursor for rendering.
    ///
    /// 注意：此方法通过逐字符迭代来计算光标的显示位置。对于非常长的行，
    /// 这可能会在性能上造成轻微的低效率，因为它每次都会从行首重新计算。
    /// 如果在实际使用中发现性能瓶颈，可以考虑通过缓存行中断位置或使用
    /// 更高级的文本布局算法进行优化。
    pub fn cursor_display_position(&self, width: usize) -> (u16, u16) {
        if width == 0 {
            return (0, 0);
        }
        let mut col = 0usize;
        let mut row = 0usize;
        for ch in self.buffer[..self.cursor].chars() {
            if ch == '\n' {
                row += 1;
                col = 0;
                continue;
            }
            let char_width = unicode_width::UnicodeWidthChar::width(ch)
                .unwrap_or(1)
                .max(1);
            if col + char_width > width {
                row += 1;
                col = 0;
            }
            col += char_width;
            if col >= width {
                row += 1;
                col = 0;
            }
        }
        (col as u16, row as u16)
    }
}

/// State for the agent switcher overlay.
#[derive(Debug, Clone)]
pub struct AgentSwitcherState {
    /// The list of available agent profiles.
    pub profiles: Vec<AgentProfile>,
    /// The index of the currently selected profile.
    pub selected: usize,
}

impl AgentSwitcherState {
    pub fn new(profiles: Vec<AgentProfile>, selected: usize) -> Self {
        let mut state = Self {
            profiles,
            selected: 0,
        };
        if !state.profiles.is_empty() {
            state.selected = selected.min(state.profiles.len() - 1);
        }
        state
    }

    /// Moves the selection up or down in the list.
    pub fn move_selection(&mut self, delta: isize) {
        if self.profiles.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.profiles.len() as isize;
        let mut next = self.selected as isize + delta;
        if next < 0 {
            next = 0;
        }
        if next >= len {
            next = len - 1;
        }
        self.selected = next as usize;
    }

    /// Returns the currently selected agent profile.
    pub fn selected_profile(&self) -> Option<&AgentProfile> {
        self.profiles.get(self.selected)
    }
}

/// Represents the state of any active overlay panel.
///
/// Overlays are temporary UI panels that appear on top of the main interface,
/// such as command palettes or confirmation dialogs.
#[derive(Debug, Clone)]
pub enum OverlayState {
    CommandPalette(CommandPaletteState),
    InputPrompt(InputPromptState),
    ConfirmDelete(ConfirmDeleteState),
    AgentSwitcher(AgentSwitcherState),
}

/// State for the command palette overlay.
#[derive(Debug, Clone)]
pub struct CommandPaletteState {
    /// All possible entries in the command palette.
    pub entries: Vec<CommandPaletteEntry>,
    /// The current user-entered filter string.
    pub filter: String,
    /// The indices of entries that are currently visible after filtering.
    pub visible: Vec<usize>,
    /// The index of the currently selected item within the `visible` list.
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

    /// Returns the currently selected command entry.
    pub fn selected_entry(&self) -> Option<&CommandPaletteEntry> {
        self.visible
            .get(self.selected)
            .and_then(|idx| self.entries.get(*idx))
    }

    /// Moves the selection up or down, wrapping around the list.
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

    /// Updates the list of visible entries based on the current filter.
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

/// A single entry in the command palette.
#[derive(Debug, Clone)]
pub struct CommandPaletteEntry {
    /// The main text displayed for the entry.
    pub label: String,
    /// Optional additional detail text.
    pub detail: Option<String>,
    /// The action to perform when the entry is selected.
    pub action: CommandAction,
    /// The combined text used for searching/filtering.
    pub(crate) search_text: String,
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

/// An enumeration of all possible actions that can be triggered from the command palette.
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
    ManageAgentPanel,
    SwitchAgent,
    DeleteFile,
}

/// An enumeration of actions that require further user input via a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingInputAction {
    OpenFile,
    SaveAs,
    CreateFile,
    SetAgentApiKey,
}

/// State for the input prompt overlay.
#[derive(Debug, Clone)]
pub struct InputPromptState {
    /// The title displayed at the top of the prompt.
    pub title: String,
    /// The current value entered by the user.
    pub value: String,
    /// Placeholder text to display when the input is empty.
    pub placeholder: String,
    /// The action that will be performed upon confirmation.
    pub action: PendingInputAction,
    /// An optional error message to display.
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

/// State for the "confirm delete" overlay.
#[derive(Debug, Clone)]
pub struct ConfirmDeleteState {
    /// The path to the file or directory to be deleted.
    pub target: PathBuf,
    /// The string representation of the target to display to the user.
    pub display: String,
    /// The index of the selected button (0 for Confirm, 1 for Cancel).
    pub selected_index: usize,
    /// Whether to suppress this confirmation for future deletions in this session.
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

    /// Toggles the selection between the "Confirm" and "Cancel" buttons.
    pub fn toggle_selection(&mut self) {
        self.selected_index = (self.selected_index + 1) % 2;
    }

    /// Sets the selection to a specific index.
    pub fn select(&mut self, index: usize) {
        self.selected_index = index.min(1);
    }

    /// Returns true if the "Confirm" button is currently selected.
    pub fn confirm_selected(&self) -> bool {
        self.selected_index == 0
    }
}
