use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use log::debug;

use super::{AgentComposer, App};
use crate::definitions::{
    EditorPreferences, FocusArea, LayoutState, MenuBar, StatusControlRegistry,
};
use crate::editor::Editor;
use crate::file_tree::FileTree;
use crate::panels::{agent::AgentPanel, terminal::TerminalPane};

impl App {
    /// Creates a new instance of the `App` state.
    ///
    /// This function initializes all the components of the application, including
    /// the file tree, editor, terminal, and agent panels.
    ///
    /// # Arguments
    ///
    /// * `workspace_root` - The root path of the workspace to be opened.
    pub fn new(workspace_root: PathBuf) -> Result<Self> {
        // Attempt to resolve the canonical path of the workspace root.
        let canonical_root = workspace_root.canonicalize().unwrap_or(workspace_root);
        debug!("Initializing App with workspace: {}", canonical_root.display());

        // Initialize the file tree, populating it with a placeholder if empty.
        let mut file_tree = FileTree::from_root(canonical_root.clone());
        if file_tree.is_empty() {
            file_tree.populate_with_placeholder();
        }

        let preferences = EditorPreferences::new();

        // Initialize the editor.
        let editor = Editor::new(preferences.clone());

        // Initialize the terminal pane with some placeholder content.
        let mut terminal = TerminalPane::default();
        terminal.append_line("> cargo run    // Integrated terminal placeholder");
        terminal.append_line("Build not started: coming soon");

        // Initialize the agent panel with placeholder content.
        let agent = AgentPanel::with_placeholder();

        // Construct the main App struct with all its initial state.
        let mut app = Self {
            should_quit: false,
            focus: FocusArea::Editor, // Start with focus on the editor.
            editor,
            file_tree,
            terminal,
            agent,
            agent_input: AgentComposer::new(),
            status_message: String::from("F6 to switch panes, type to edit, Ctrl+Q to quit"),
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
            tick_rate: Duration::from_millis(250), // Set a default tick rate.
            agent_capabilities: None,
            agent_manager: None,
            pending_agent_setup: None,
        };

        // Initialize the agent runtime environment.
        app.initialize_agent_runtime();

        Ok(app)
    }
}