use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{App, CommandAction, FocusArea, PendingInputAction};
use crate::definitions::{IndentKind, MenuAction};

impl App {
    /// The main entry point for handling keyboard events.
    ///
    /// This function acts as a router, dispatching the key event to the appropriate
    /// handler based on the application's current state.
    pub async fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Overlays capture all input.
        if self.overlay.is_some() {
            self.handle_overlay_key(key).await;
            return;
        }

        // The menu bar captures input if it's open.
        if self.menu_bar.open && self.handle_menu_bar_key(key).await {
            return;
        }

        // Global shortcuts are handled next.
        if self.handle_global_shortcuts(key).await {
            return;
        }

        // If no global shortcuts were matched, pass the key to the focused pane.
        match self.focus {
            FocusArea::Editor => self.handle_editor_key(key),
            FocusArea::FileTree => self.handle_file_tree_key(key).await,
            FocusArea::Terminal => self.handle_terminal_key(key),
            FocusArea::Agent => self.handle_agent_key(key).await,
        }
    }

    /// Handles key events when the menu bar is open.
    /// Returns `true` if the key was handled, `false` otherwise.
    async fn handle_menu_bar_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => self.menu_bar.close(),
            KeyCode::Left => {
                self.menu_bar.move_active(-1);
                self.update_menu_hover_message();
            }
            KeyCode::Right => {
                self.menu_bar.move_active(1);
                self.update_menu_hover_message();
            }
            KeyCode::Up => {
                self.menu_bar.move_highlight(-1);
                self.update_menu_hover_message();
            }
            KeyCode::Down => {
                self.menu_bar.move_highlight(1);
                self.update_menu_hover_message();
            }
            KeyCode::Enter => {
                if let Some(action) = self.menu_bar.highlighted_action() {
                    let command = match action {
                        MenuAction::New => Some(CommandAction::NewDocument),
                        MenuAction::CreateFile => Some(CommandAction::CreateFile),
                        MenuAction::Open => Some(CommandAction::OpenFile),
                        MenuAction::Save => Some(CommandAction::SaveFile),
                        MenuAction::SaveAs => Some(CommandAction::SaveFileAs),
                        MenuAction::ToggleHiddenFiles => Some(CommandAction::ToggleHiddenFiles),
                        MenuAction::Delete => Some(CommandAction::DeleteFile),
                        MenuAction::ToggleFileTree => Some(CommandAction::ToggleFileTree),
                        MenuAction::ToggleEditor => Some(CommandAction::ToggleEditor),
                        MenuAction::ToggleTerminal => Some(CommandAction::ToggleTerminal),
                        MenuAction::ToggleAgent => Some(CommandAction::ToggleAgent),
                        MenuAction::ManageAgentPanel => Some(CommandAction::ManageAgentPanel),
                        MenuAction::SwitchAgent => Some(CommandAction::SwitchAgent),
                        MenuAction::CommandPalette => {
                            self.open_command_palette();
                            None
                        }
                        MenuAction::Exit => {
                            self.should_quit = true;
                            None
                        }
                        MenuAction::None => None,
                    };
                    if let Some(command) = command {
                        self.execute_menu_action(command).await;
                    }
                }
                self.menu_bar.close();
            }
            _ => return false,
        }
        true
    }

    /// Handles global keyboard shortcuts.
    /// Returns `true` if a shortcut was handled, `false` otherwise.
    async fn handle_global_shortcuts(&mut self, key: KeyEvent) -> bool {
        match (key.code, key.modifiers) {
            // Ctrl+S: Save
            (KeyCode::Char('s'), m) if m.contains(KeyModifiers::CONTROL) => {
                if m.contains(KeyModifiers::ALT) {
                    // Ctrl+Alt+S: Save As
                    self.prompt_input(
                        PendingInputAction::SaveAs,
                        "Save As",
                        self.suggest_current_path(),
                    );
                } else {
                    let _ = self.perform_save_current().await;
                }
            }
            // Ctrl+O: Open File
            (KeyCode::Char('o'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.prompt_input(
                    PendingInputAction::OpenFile,
                    "Open File",
                    self.suggest_current_path(),
                );
            }
            // Ctrl+N: New Document / Ctrl+Shift+N: New File
            (KeyCode::Char('n'), m) if m.contains(KeyModifiers::CONTROL) => {
                if m.contains(KeyModifiers::SHIFT) {
                    self.prompt_input(
                        PendingInputAction::CreateFile,
                        "Create New File",
                        self.suggest_current_path(),
                    );
                } else {
                    self.perform_new_document();
                }
            }
            // Ctrl+P: Open Menu / Ctrl+Shift+P: Open Command Palette
            (KeyCode::Char('p'), m) if m.contains(KeyModifiers::CONTROL) => {
                if m.contains(KeyModifiers::SHIFT) {
                    self.open_command_palette();
                }
                else {
                    self.menu_bar.open(0);
                    self.update_menu_hover_message();
                }
            }
            // Ctrl+Q: Quit
            (KeyCode::Char('q'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            // F10: Toggle Menu Bar
            (KeyCode::F(10), _) => {
                if self.menu_bar.open {
                    self.menu_bar.close();
                } else {
                    self.menu_bar.open(0);
                    self.update_menu_hover_message();
                }
            }
            // F6: Cycle Focus
            (KeyCode::F(6), m) if m.contains(KeyModifiers::SHIFT) => {
                self.cycle_focus(-1); // Shift+F6: Cycle backwards
            }
            (KeyCode::F(6), _) => {
                self.cycle_focus(1); // F6: Cycle forwards
            }
            _ => return false,
        }
        true
    }

    /// Handles key events when the Editor pane is focused.
    fn handle_editor_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => match self.preferences.indent {
                IndentKind::Spaces(n) => {
                    for _ in 0..n {
                        self.editor.insert_char(' ');
                    }
                }
                IndentKind::Tabs => self.editor.insert_char('\t'),
            },
            // 忽略其他 Ctrl+char 组合。这是一个功能限制，未来可以考虑实现剪切、复制、粘贴、撤销、重做等标准编辑器快捷键。
            KeyCode::Char(_) if key.modifiers.contains(KeyModifiers::CONTROL) => {}
            // Basic text input
            KeyCode::Char(ch) => {
                self.editor.insert_char(ch);
            }
            KeyCode::Enter => self.editor.insert_newline(),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete_forward(),
            // Cursor movement
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

    /// Handles key events when the FileTree pane is focused.
    async fn handle_file_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.file_tree.move_selection(-1),
            KeyCode::Down => self.file_tree.move_selection(1),
            KeyCode::Enter => self.activate_file_tree_selection().await,
            KeyCode::Delete => self.delete_selected_from_tree(),
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.file_tree.refresh();
                self.status_message = String::from("File tree refreshed");
            }
            _ => {}
        }
    }

    /// Handles key events when the Terminal pane is focused.
    ///
    /// 目前，终端窗格的功能非常有限，主要只支持滚动。未来可以考虑实现完整的终端交互，
    /// 包括发送用户输入到终端进程和处理其输出。
    fn handle_terminal_key(&mut self, key: KeyEvent) {
        // Currently, only scrolling is implemented.
        match key.code {
            KeyCode::Up => self.terminal.scroll(-1),
            KeyCode::Down => self.terminal.scroll(1),
            KeyCode::PageUp => self.terminal.scroll(-5),
            KeyCode::PageDown => self.terminal.scroll(5),
            _ => {}
        }
    }

    /// Handles key events when the Agent pane is focused.
    async fn handle_agent_key(&mut self, key: KeyEvent) {
        let modifiers = key.modifiers;
        match key.code {
            KeyCode::Enter => {
                if modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Enter: Insert a newline in the agent input.
                    self.agent_input.insert_newline();
                } else {
                    // Enter: Submit the prompt to the agent.
                    self.submit_agent_prompt().await;
                }
            }
            // Basic text editing in the agent input composer.
            KeyCode::Backspace => self.agent_input.backspace(),
            KeyCode::Delete => self.agent_input.delete(),
            KeyCode::Left => self.agent_input.move_left(),
            KeyCode::Right => self.agent_input.move_right(),
            KeyCode::Home => self.agent_input.move_to_line_start(),
            KeyCode::End => self.agent_input.move_to_line_end(),
            KeyCode::Esc => {
                self.agent_input.clear();
                self.status_message = String::from("Agent input cleared");
            }
            // History navigation and selection movement.
            KeyCode::Up => self.handle_agent_history_navigation(key, -1),
            KeyCode::Down => self.handle_agent_history_navigation(key, 1),
            // Scrolling the agent conversation.
            KeyCode::PageUp => self.agent.move_selection(-5),
            KeyCode::PageDown => self.agent.move_selection(5),
            KeyCode::Tab => self.agent_input.insert_char('\t'),
            // Character input for the agent prompt.
            KeyCode::Char(ch) => {
                if modifiers.contains(KeyModifiers::CONTROL)
                    || modifiers.contains(KeyModifiers::ALT)
                {
                    // Ignore other control shortcuts for now.
                } else {
                    self.agent_input.insert_char(ch);
                }
            }
            _ => {}
        }
    }

    /// Handles agent history navigation and selection movement.
    fn handle_agent_history_navigation(&mut self, key: KeyEvent, delta: isize) {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            // Ctrl+Up/Down: Move selection in the agent conversation.
            self.agent.move_selection(delta);
            return;
        }

        if !self.agent_input.is_empty() {
            // If there's text, don't navigate history.
            // In the future, this could move the cursor, but for now, we do nothing.
            return;
        }

        // Navigate history if input is empty.
        let navigated = if delta < 0 {
            self.agent_input.history_previous()
        } else {
            self.agent_input.history_next()
        };

        if navigated {
            self.status_message = if delta < 0 {
                "Previous agent prompt loaded".into()
            } else {
                "Next agent prompt loaded".into()
            };
        } else {
            // If history navigation fails (e.g., at the end of the list), move selection.
            self.agent.move_selection(delta);
        }
    }
}