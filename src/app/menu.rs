use log::debug;

use super::App;
use crate::app::CommandAction;
use crate::definitions::StatusControlKind;

// Implementation block for menu-related logic in the App.
impl App {
    /// Updates the status message based on the currently hovered menu item.
    pub(crate) fn update_menu_hover_message(&mut self) {
        if let Some(menu_idx) = self.menu_bar.active_index {
            if let Some(entry_idx) = self.menu_bar.highlighted_entry
                && let Some(menu) = self.menu_bar.items.get(menu_idx)
                    && let Some(entry) = menu.entries.get(entry_idx) {
                        self.status_message = format!("{} > {}", menu.title, entry.label);
                        return;
                    }
            // If no entry is highlighted, just show the active menu's title.
            if let Some(menu) = self.menu_bar.items.get(menu_idx) {
                self.status_message = format!("Menu: {}", menu.title);
            }
        }
    }

    /// Executes a specific menu action.
    ///
    /// 注意：此函数中的 `match` 块可能会随着 `CommandAction` 变体的增加而变得庞大。
    /// 如果动作数量变得难以管理，可以考虑将其重构为更模块化的命令模式或策略模式。
    pub(crate) async fn execute_menu_action(&mut self, action: CommandAction) {
        self.execute_action(action).await;
    }

    /// Handles a click event on a status bar control.
    pub(crate) fn handle_status_control_click(&mut self, kind: StatusControlKind) -> bool {
        debug!("Handling status control click: {:?}", kind);
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
                self.status_message = format!("Cursor: {}:{}", line + 1, col + 1);
                true
            }
            StatusControlKind::Dirty => {
                if self.editor.is_dirty() {
                    self.status_message = String::from("File has unsaved changes");
                } else {
                    self.status_message = String::from("File is saved");
                }
                true
            }
        }
    }
}