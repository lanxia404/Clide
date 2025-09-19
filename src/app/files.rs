use std::fs;
use std::path::{Path, PathBuf};

use log::{debug, error, info, warn};

use super::{App, ConfirmDeleteState, FocusArea, OverlayState, PendingInputAction};
use crate::file_tree::{FileEntryKind, FileTreeAction};

// Implementation block for file-related logic in the App.
impl App {
    /// Toggles the visibility of hidden files (dotfiles) in the file tree.
    pub(crate) fn toggle_hidden_files(&mut self) {
        let enabled = self.file_tree.toggle_show_hidden();
        info!(
            "Toggled hidden files: {}",
            if enabled { "shown" } else { "hidden" }
        );
        self.status_message = if enabled {
            String::from("Showing hidden files")
        } else {
            String::from("Hiding hidden files")
        };
    }

    /// Formats a file path for display in the UI.
    pub(crate) fn format_display_path(&self, path: &Path) -> String {
        self.format_path(path, false)
    }

    /// Formats a file path for use in an input prompt.
    pub(crate) fn format_input_path(&self, path: &Path) -> String {
        self.format_path(path, true)
    }

    /// A generic helper to format a file path for display or input.
    fn format_path(&self, path: &Path, for_input: bool) -> String {
        if let Ok(relative) = path.strip_prefix(&self.workspace_root) {
            if relative.as_os_str().is_empty() {
                if for_input {
                    ".".to_string()
                } else {
                    "./".to_string()
                }
            } else if for_input {
                relative.display().to_string()
            } else {
                format!("./{}", relative.display())
            }
        } else if let Some(home) = Self::home_directory() {
            if let Ok(suffix) = path.strip_prefix(&home) {
                if suffix.as_os_str().is_empty() {
                    "~".to_string()
                } else {
                    format!("~/வுகளை {}", suffix.display())
                }
            } else {
                path.display().to_string()
            }
        } else {
            path.display().to_string()
        }
    }

    /// Attempts to canonicalize a path, returning the original path on failure.
    pub(crate) fn canonicalize_path(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    /// Ensures that the file tree selection highlights the specified path.
    pub(crate) fn ensure_file_tree_highlights(&mut self, path: &Path) {
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

    /// Handles the activation (e.g., pressing Enter) of the selected item in the file tree.
    pub(crate) async fn activate_file_tree_selection(&mut self) {
        match self.file_tree.activate_selected() {
            FileTreeAction::OpenFile(path) => {
                info!("Opening file from tree: {}", path.display());
                let _ = self.perform_open_file(path).await;
            }
            FileTreeAction::ChangedDir(path) => {
                let display = self.format_display_path(&path);
                info!("Changed file tree directory to: {}", display);
                self.status_message = format!("Directory changed to: {}", display);
            }
            FileTreeAction::None => {}        }
    }

    /// Opens a file in the editor.
    pub(crate) async fn perform_open_file(&mut self, path: PathBuf) -> Result<(), String> {
        match self.editor.open_file(&path) {
            Ok(_) => {
                let display = self.format_display_path(&path);
                self.status_message = format!("Opened file: {}", display);
                info!("Successfully opened file: {}", path.display());
                self.focus = FocusArea::Editor;
                self.ensure_file_tree_highlights(&path);
                if self.should_auto_send_agent() {
                    self.send_to_agent(None).await;
                }
                Ok(())
            }
            Err(err) => {
                let message = format!("Failed to open: {}", err);
                error!("Failed to open file {}: {}", path.display(), err);
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    /// Saves the currently active file in the editor.
    /// If the file has no path, it prompts for a path using "Save As".
    pub(crate) async fn perform_save_current(&mut self) -> Result<(), String> {
        if self.editor.file_path().is_none() {
            self.prompt_input(
                PendingInputAction::SaveAs,
                "Save As",
                self.suggest_current_path(),
            );
            return Err(String::from("File path is not specified"));
        }
        match self.editor.save() {
            Ok(path) => {
                let display = self.format_display_path(&path);
                self.status_message = format!("File saved: {}", display);
                info!("Successfully saved file: {}", path.display());
                Ok(())
            }
            Err(err) => {
                let message = format!("Failed to save: {}", err);
                error!("Failed to save file: {}", err);
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    /// Saves the current editor content to a new specified path.
    pub(crate) fn perform_save_as(&mut self, path: PathBuf) -> Result<(), String> {
        match self.editor.save_as(&path) {
            Ok(saved) => {
                let display = self.format_display_path(&saved);
                self.status_message = format!("File saved as: {}", display);
                info!("Successfully saved file as: {}", saved.display());
                self.file_tree.refresh();
                if self.is_in_workspace(&saved) {
                    self.ensure_file_tree_highlights(&saved);
                }
                Ok(())
            }
            Err(err) => {
                let message = format!("Failed to save as: {}", err);
                error!("Failed to save file as: {}", err);
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    /// Creates a new file at the specified path with the current editor content.
    pub(crate) fn perform_create_file(&mut self, path: PathBuf) -> Result<(), String> {
        if path.exists() {
            let display = self.format_display_path(&path);
            let message = format!("Creation failed: file already exists ({})", display);
            self.status_message = message.clone();
            warn!("Attempted to create a file that already exists: {}", path.display());
            return Err(message);
        }
        self.editor.new_document();
        match self.editor.save_as(&path) {
            Ok(saved) => {
                let display = self.format_display_path(&saved);
                self.status_message = format!("File created: {}", display);
                info!("Successfully created new file: {}", saved.display());
                self.file_tree.refresh();
                if self.is_in_workspace(&saved) {
                    self.ensure_file_tree_highlights(&saved);
                }
                Ok(())
            }
            Err(err) => {
                let message = format!("Creation failed: {}", err);
                error!("Failed to create file: {}", err);
                self.status_message = message.clone();
                Err(message)
            }
        }
    }

    /// Creates a new, empty, untitled document in the editor.
    pub(crate) fn perform_new_document(&mut self) {
        self.editor.new_document();
        self.status_message = String::from("New untitled document created");
        debug!("Created a new untitled document");
        self.focus = FocusArea::Editor;
    }

    /// Suggests a path for input prompts like "Open" or "Save As".
    /// It prioritizes the editor's current file path, then the file tree selection.
    pub(crate) fn suggest_current_path(&self) -> Option<String> {
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

    /// Resolves a raw string input into a full, absolute-like path.
    /// It expands `~` and joins relative paths with the workspace root.
    pub(crate) fn resolve_input_path(&self, raw: &str) -> Result<PathBuf, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(String::from("Path cannot be empty"));
        }
        let candidate = Self::expand_user_path(trimmed)?;
        let path = if candidate.is_absolute() {
            candidate
        } else {
            self.workspace_root.join(candidate)
        };
        Ok(path)
    }

    /// Initiates the file deletion process via a prompt.
    pub(crate) fn delete_via_prompt(&mut self) {
        self.menu_bar.close();
        if let Some(path) = self.selected_file_for_actions() {
            self.request_delete(path);
        } else {
            self.status_message = String::from("No file selected to delete");
            warn!("Delete request failed: no file selected");
        }
    }

    /// Determines which file is the target for an action (e.g., delete).
    /// Prioritizes the file tree selection, then falls back to the editor's current file.
    pub(crate) fn selected_file_for_actions(&self) -> Option<PathBuf> {
        if let Some(entry) = self.file_tree.selected_entry()
            && entry.kind == FileEntryKind::File {
                return Some(entry.path.clone());
            }
        self.editor
            .file_path()
            .map(|path| self.canonicalize_path(path))
    }

    /// Initiates a deletion request from the file tree.
    pub(crate) fn delete_selected_from_tree(&mut self) {
        if let Some(entry) = self.file_tree.selected_entry() {
            match entry.kind {
                FileEntryKind::File => self.request_delete(entry.path.clone()),
                FileEntryKind::Directory => {
                    let display = self.format_display_path(&entry.path);
                    // 警告：目前不支持删除目录。这是一个待实现的功能。
                    self.status_message = format!("Deleting directories is not yet supported: {}", display);
                }
                _ => {
                    self.status_message = String::from("This item cannot be deleted");
                }
            }
        } else {
            self.status_message = String::from("No item selected");
        }
    }

    /// Requests to delete a path, showing a confirmation prompt if necessary.
    pub(crate) fn request_delete(&mut self, path: PathBuf) {
        let target = self.canonicalize_path(&path);
        match fs::metadata(&target) {
            Ok(meta) => {
                if !meta.is_file() {
                    let display = self.format_display_path(&target);
                    // 警告：目前不支持删除目录。这是一个待实现的功能。
                    self.status_message = format!("Deleting directories is not yet supported: {}", display);
                    warn!("Ignoring request to delete directory: {}", target.display());
                    return;
                }
            }
            Err(err) => {
                self.status_message = format!("Could not read file metadata: {}", err);
                error!("Failed to read metadata for {}: {}", target.display(), err);
                return;
            }
        }

        // If confirmation is suppressed, delete immediately.
        if self.suppress_delete_confirm {
            info!("Delete confirmation suppressed, deleting: {}", target.display());
            self.finalize_delete(target, false);
            return;
        }

        // Otherwise, show the confirmation overlay.
        let display = self.format_display_path(&target);
        info!("Requesting delete confirmation for: {}", target.display());
        self.overlay = Some(OverlayState::ConfirmDelete(ConfirmDeleteState::new(
            target,
            display.clone(),
        )));
        self.status_message = format!("Confirm deletion: {}", display);
    }

    /// Performs the actual file deletion after confirmation.
    pub(crate) fn finalize_delete(&mut self, path: PathBuf, suppress_future: bool) {
        if suppress_future {
            self.suppress_delete_confirm = true;
        }

        let display = self.format_display_path(&path);
        match fs::metadata(&path) {
            Ok(meta) => {
                if !meta.is_file() {
                    self.status_message = format!("Deletion failed: {} is not a file", display);
                    return;
                }
            }
            Err(err) => {
                self.status_message = format!("Deletion failed (could not read metadata): {}", err);
                return;
            }
        }

        match fs::remove_file(&path) {
            Ok(_) => {
                // If the deleted file was open in the editor, create a new blank document.
                if self.editor.file_path().map(|p| self.canonicalize_path(p)) == Some(self.canonicalize_path(&path)) {
                    self.editor.new_document();
                }
                self.file_tree.refresh();
                if self.is_in_workspace(&path) {
                    self.file_tree.set_selection(0);
                }
                self.status_message = format!("File deleted: {}", display);
                info!("Successfully deleted file: {}", path.display());
            }
            Err(err) => {
                self.status_message = format!("Deletion failed: {}", err);
                error!("Failed to delete file {}: {}", path.display(), err);
            }
        }
    }

    /// Checks if a given path is within the current workspace.
    pub(crate) fn is_in_workspace(&self, path: &Path) -> bool {
        path.starts_with(&self.workspace_root)
    }

    /// Gets the user's home directory path.
    fn home_directory() -> Option<PathBuf> {
        if cfg!(windows) {
            std::env::var("USERPROFILE").map(PathBuf::from).ok()
        } else {
            std::env::var("HOME").map(PathBuf::from).ok()
        }
    }

    /// Expands a path that starts with `~` to the user's home directory.
    ///
    /// 注意：在 Windows 系统上，`PathBuf::join` 通常能正确处理路径分隔符，
    /// 但在处理 `~` 后面的路径时，需要确保其行为符合预期。
    fn expand_user_path(raw: &str) -> Result<PathBuf, String> {
        if let Some(rest) = raw.strip_prefix('~') {
            let home = Self::home_directory().ok_or_else(|| String::from("Could not find home directory"))?;
            if rest.is_empty() {
                return Ok(home);
            }
            let mut chars = rest.chars();
            match chars.next() {
                Some('/') | Some('\\') => Ok(home.join(chars.as_str())),
                _ => Err(String::from("Unsupported home directory format")),
            }
        } else {
            Ok(PathBuf::from(raw))
        }
    }
}
