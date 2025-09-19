use log::debug;

use super::{App, CommandAction, PaneKind, PendingInputAction};

impl App {
    /// The single source of truth for executing a `CommandAction`.
    pub(crate) async fn execute_action(&mut self, action: CommandAction) {
        debug!("Executing action {:?}", action);
        match action {
            CommandAction::NewDocument => self.perform_new_document(),
            CommandAction::CreateFile => {
                self.prompt_input(
                    PendingInputAction::CreateFile,
                    "Create New File",
                    self.suggest_current_path(),
                );
            }
            CommandAction::OpenFile => {
                self.prompt_input(
                    PendingInputAction::OpenFile,
                    "Open File",
                    self.suggest_current_path(),
                );
            }
            CommandAction::SaveFile => {
                let _ = self.perform_save_current().await;
            }
            CommandAction::SaveFileAs => {
                self.prompt_input(
                    PendingInputAction::SaveAs,
                    "Save As",
                    self.suggest_current_path(),
                );
            }
            CommandAction::ToggleHiddenFiles => self.toggle_hidden_files(),
            CommandAction::DeleteFile => self.delete_via_prompt(),
            CommandAction::ToggleWrap => self.toggle_wrap_mode(),
            CommandAction::ToggleLineEnding => self.toggle_line_ending(),
            CommandAction::ToggleEncoding => self.toggle_encoding(),
            CommandAction::CycleIndent => self.cycle_indent_kind(),
            CommandAction::ToggleFileTree => self.toggle_pane(PaneKind::FileTree),
            CommandAction::ToggleEditor => self.toggle_pane(PaneKind::Editor),
            CommandAction::ToggleTerminal => self.toggle_pane(PaneKind::Terminal),
            CommandAction::ToggleAgent => self.toggle_pane(PaneKind::Agent),
            CommandAction::ManageAgentPanel => self.manage_agent_panel(),
            CommandAction::SwitchAgent => self.open_agent_switcher(),
        }
    }
}
