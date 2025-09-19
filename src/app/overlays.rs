use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::{debug, info, warn};

use super::{
    AgentSwitcherState, App, CommandAction, CommandPaletteEntry, CommandPaletteState, ConfirmDeleteState,
    InputPromptState, OverlayState, PendingInputAction,
};

// Implementation block for overlay-related logic in the App.
impl App {
    /// Opens an input prompt overlay.
    ///
    /// This function creates and displays an `InputPrompt` overlay, which is used to
    /// get text input from the user for a specific action.
    pub(crate) fn prompt_input(
        &mut self,
        action: PendingInputAction,
        title: &str,
        suggestion: Option<String>,
    ) {
        let placeholder = match action {
            PendingInputAction::OpenFile => "Enter file path to open (relative to workspace)",
            PendingInputAction::SaveAs => "Enter file path to save as (relative to workspace)",
            PendingInputAction::CreateFile => "Enter file path to create (relative to workspace)",
            PendingInputAction::SetAgentApiKey => "Enter the agent API key",
        };
        debug!("Showing input prompt: {}", title);
        self.menu_bar.close();
        self.overlay = Some(OverlayState::InputPrompt(InputPromptState::new(
            title,
            placeholder,
            action,
            suggestion,
        )));
        self.status_message = format!("{}: waiting for input", title);
    }

    /// Generates the list of entries for the command palette.
    ///
    /// This function dynamically creates a list of all available commands based on
    /// the current application state (e.g., showing different labels for toggleable
    /// options).
    pub(crate) fn command_palette_entries(&self) -> Vec<CommandPaletteEntry> {
        let mut entries = Vec::new();
        entries.push(CommandPaletteEntry::new(
            "New Untitled File",
            Some(String::from("Ctrl+N")),
            CommandAction::NewDocument,
        ));
        entries.push(CommandPaletteEntry::new(
            "New File (from path)",
            None,
            CommandAction::CreateFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "Open File...",
            Some(String::from("Ctrl+O")),
            CommandAction::OpenFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "Save File",
            Some(String::from("Ctrl+S")),
            CommandAction::SaveFile,
        ));
        entries.push(CommandPaletteEntry::new(
            "Save File As...",
            Some(String::from("Ctrl+Alt+S")),
            CommandAction::SaveFileAs,
        ));
        entries.push(CommandPaletteEntry::new(
            "Delete File...",
            Some(String::from("Delete")),
            CommandAction::DeleteFile,
        ));
        entries.push(CommandPaletteEntry::new(
            if self.file_tree.show_hidden() {
                "Hide Hidden Files"
            } else {
                "Show Hidden Files"
            },
            None,
            CommandAction::ToggleHiddenFiles,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("Toggle Wrap (Current: {})", self.preferences.wrap_mode.label()),
            None,
            CommandAction::ToggleWrap,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("Toggle Line Ending (Current: {})", self.preferences.line_ending.label()),
            None,
            CommandAction::ToggleLineEnding,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("Toggle Encoding (Current: {})", self.preferences.encoding.label()),
            None,
            CommandAction::ToggleEncoding,
        ));
        entries.push(CommandPaletteEntry::new(
            format!("Cycle Indent (Current: {})", self.preferences.indent.label()),
            None,
            CommandAction::CycleIndent,
        ));
        entries.push(CommandPaletteEntry::new(
            "Toggle File Tree",
            None,
            CommandAction::ToggleFileTree,
        ));
        entries.push(CommandPaletteEntry::new(
            "Toggle Editor",
            None,
            CommandAction::ToggleEditor,
        ));
        entries.push(CommandPaletteEntry::new(
            "Toggle Terminal",
            None,
            CommandAction::ToggleTerminal,
        ));
        entries.push(CommandPaletteEntry::new(
            "Toggle Agent Panel",
            None,
            CommandAction::ToggleAgent,
        ));
        entries.push(CommandPaletteEntry::new(
            "Manage Agent Panel",
            None,
            CommandAction::ManageAgentPanel,
        ));
        entries.push(CommandPaletteEntry::new(
            "Switch Agent...",
            None,
            CommandAction::SwitchAgent,
        ));
        entries
    }

    /// Opens the command palette overlay.
    pub(crate) fn open_command_palette(&mut self) {
        self.menu_bar.close();
        let entries = self.command_palette_entries();
        self.overlay = Some(OverlayState::CommandPalette(CommandPaletteState::new(
            entries,
        )));
        self.status_message = String::from("Command Palette: type to filter");
        info!("Opened command palette");
    }

    /// Closes any active overlay.
    pub(crate) fn close_overlay(&mut self) {
        self.overlay = None;
        debug!("Closed overlay");
    }

    /// Executes an action that requires a file path from an input prompt.
    async fn execute_pending_input(
        &mut self,
        action: PendingInputAction,
        path: PathBuf,
    ) -> Result<(), String> {
        debug!("Executing input action {:?} for path {}", action, path.display());
        match action {
            PendingInputAction::OpenFile => {
                if !path.exists() {
                    warn!("File does not exist: {}", path.display());
                    return Err(format!("File does not exist: {}", path.display()));
                }
                self.perform_open_file(path).await
            }
            PendingInputAction::SaveAs => self.perform_save_as(path),
            PendingInputAction::CreateFile => self.perform_create_file(path),
            PendingInputAction::SetAgentApiKey => Err(String::from("This action does not take a path"))
        }
    }

    /// Handles the completion of an input prompt overlay.
    ///
    /// This is called when the user presses Enter in an input prompt.
    pub(crate) async fn complete_input_prompt(&mut self) {
        let (action, input) = match self.overlay.as_mut() {
            Some(OverlayState::InputPrompt(state)) => {
                let action = state.action;
                let input = state.value.clone();
                state.error = None;
                (action, input)
            }
            _ => return,
        };

        // Special handling for setting the API key, which doesn't involve a file path.
        if action == PendingInputAction::SetAgentApiKey {
            let trimmed = input.trim();
            if trimmed.is_empty() {
                if let Some(OverlayState::InputPrompt(state)) = self.overlay.as_mut() {
                    state.error = Some(String::from("API key cannot be empty"));
                }
                return;
            }
            match self.apply_agent_api_key(trimmed) {
                Ok(_) => {
                    self.close_overlay();
                    self.status_message = String::from("Agent API key saved, reconnecting...");
                    info!("API key set, re-initializing agent");
                    self.initialize_agent_runtime();
                }
                Err(err) => {
                    if let Some(OverlayState::InputPrompt(state)) = self.overlay.as_mut() {
                        state.error = Some(err.clone());
                    }
                    warn!("Failed to set API key: {}", err);
                    self.status_message = format!("Agent setup failed: {}", err);
                }
            }
            return;
        }

        // For other actions, resolve the input as a file path.
        let result = match self.resolve_input_path(&input) {
            Ok(path) => self.execute_pending_input(action, path).await,
            Err(msg) => Err(msg),
        };

        // Handle the result of the action, closing the overlay on success or showing an error.
        match result {
            Ok(_) => {
                self.close_overlay();
                info!("Completed input action {:?}", action);
            }
            Err(message) => {
                if let Some(OverlayState::InputPrompt(state)) = self.overlay.as_mut() {
                    state.error = Some(message.clone());
                }
                warn!("Input prompt action failed: {}", message);
                self.status_message = message;
            }
        }
    }

    /// Executes an action selected from the command palette.
    pub(crate) async fn execute_command_action(&mut self, action: CommandAction) {
        self.close_overlay();
        self.execute_action(action).await;
    }


    /// Handles key events when any overlay is active.
    /// This function is the main router for all overlay-related key presses.
    pub(crate) async fn handle_overlay_key(&mut self, key: KeyEvent) {
        // Clone the overlay state to avoid borrowing issues while calling helper methods.
        if let Some(overlay) = self.overlay.clone() {
            match overlay {
                OverlayState::ConfirmDelete(state) => self.handle_confirm_delete_key(key, state).await,
                OverlayState::AgentSwitcher(state) => self.handle_agent_switcher_key(key, state).await,
                OverlayState::CommandPalette(state) => self.handle_command_palette_key(key, state).await,
                OverlayState::InputPrompt(state) => self.handle_input_prompt_key(key, state).await,
            }
        }
    }

    /// Handles key events for the ConfirmDelete overlay.
    async fn handle_confirm_delete_key(&mut self, key: KeyEvent, mut state: ConfirmDeleteState) {
        enum Decision {
            None,
            Confirm,
            Cancel,
        }
        let decision = {
            let mut result = Decision::None;
            match key.code {
                KeyCode::Esc => result = Decision::Cancel,
                KeyCode::Left | KeyCode::Up => state.select(0), // Select "Confirm"
                KeyCode::Right | KeyCode::Down => state.select(1), // Select "Cancel"
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
                self.status_message = format!("Deletion cancelled: {}", display);
            }
            Decision::None => {
                // If no decision was made, update the state in place.
                self.overlay = Some(OverlayState::ConfirmDelete(state));
            }
        }
    }

    /// Handles key events for the AgentSwitcher overlay.
    async fn handle_agent_switcher_key(&mut self, key: KeyEvent, mut state: AgentSwitcherState) {
        let mut selection_changed = false;
        match key.code {
            KeyCode::Esc => {
                self.close_overlay();
                return;
            }
            KeyCode::Up | KeyCode::Left => {
                state.move_selection(-1);
                selection_changed = true;
            }
            KeyCode::Down | KeyCode::Right => {
                state.move_selection(1);
                selection_changed = true;
            }
            KeyCode::PageUp => {
                state.move_selection(-5);
                selection_changed = true;
            }
            KeyCode::PageDown => {
                state.move_selection(5);
                selection_changed = true;
            }
            KeyCode::Tab => {
                state.move_selection(1);
                selection_changed = true;
            }
            KeyCode::BackTab => {
                state.move_selection(-1);
                selection_changed = true;
            }
            KeyCode::Enter => {
                let profile = state.selected_profile().cloned();
                self.close_overlay();
                if let Some(profile) = profile {
                    self.switch_agent_profile(profile);
                }
                return;
            }
            _ => {}
        }

        if selection_changed {
            if let Some(profile) = state.selected_profile() {
                self.status_message = format!("Select Agent: {}", profile.label);
            } else {
                self.status_message = String::from("Select Agent: No profiles available");
            }
            self.overlay = Some(OverlayState::AgentSwitcher(state));
        }
    }

    /// Handles key events for the CommandPalette overlay.
    async fn handle_command_palette_key(&mut self, key: KeyEvent, mut state: CommandPaletteState) {
        let (close, action) = {
            let mut close = false;
            let mut action = None;
            match key.code {
                KeyCode::Esc => close = true,
                KeyCode::Up => state.move_selection(-1),
                KeyCode::Down => state.move_selection(1),
                KeyCode::PageUp => state.move_selection(-5),
                KeyCode::PageDown => state.move_selection(5),
                KeyCode::Backspace => {
                    state.filter.pop();
                    state.update_filter();
                }
                KeyCode::Char(ch) => {
                    if !(key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT)) {
                        state.filter.push(ch);
                        state.update_filter();
                    }
                }
                KeyCode::Enter => action = state.selected_entry().map(|entry| entry.action),
                KeyCode::Tab => state.move_selection(1),
                KeyCode::BackTab => state.move_selection(-1),
                _ => {}
            }
            if let Some(entry) = state.selected_entry() {
                self.status_message = format!("Command: {}", entry.label);
            } else if state.visible.is_empty() {
                self.status_message = String::from("Command: No matches");
            }
            (close, action)
        };

        if let Some(action) = action {
            self.close_overlay();
            self.execute_command_action(action).await;
        } else if close {
            self.close_overlay();
        } else {
            self.overlay = Some(OverlayState::CommandPalette(state));
        }
    }

    /// Handles key events for the InputPrompt overlay.
    async fn handle_input_prompt_key(&mut self, key: KeyEvent, mut state: InputPromptState) {
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
                    if !(key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT)) {
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
            self.overlay = Some(OverlayState::InputPrompt(state)); // Put state back for completion logic
            self.complete_input_prompt().await;
        } else if close {
            self.close_overlay();
        } else {
            self.overlay = Some(OverlayState::InputPrompt(state));
        }
    }
}