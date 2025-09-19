use super::App;

// Implementation block for preference-related logic in the App.
impl App {
    /// Toggles the editor's word wrap mode.
    pub(crate) fn toggle_wrap_mode(&mut self) {
        self.preferences.wrap_mode = self.preferences.wrap_mode.toggle();
        self.status_message = format!("Wrap Mode: {}", self.preferences.wrap_mode.label());
    }

    /// Toggles the line ending format (e.g., LF vs CRLF).
    pub(crate) fn toggle_line_ending(&mut self) {
        self.preferences.line_ending = self.preferences.line_ending.toggle();
        self.status_message = format!("Line Ending: {}", self.preferences.line_ending.label());
    }

    /// Toggles the file encoding.
    pub(crate) fn toggle_encoding(&mut self) {
        self.preferences.encoding = self.preferences.encoding.toggle();
        self.status_message = format!("Encoding: {}", self.preferences.encoding.label());
    }

    /// Cycles through the available indentation styles (e.g., Tabs, Spaces(2), Spaces(4)).
    pub(crate) fn cycle_indent_kind(&mut self) {
        self.preferences.indent = self.preferences.indent.next();
        self.status_message = format!("Indent Style: {}", self.preferences.indent.label());
    }
}