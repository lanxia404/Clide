use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};
use super::header::ACCENT_COLOR;

pub fn render_status_bar(app: &App, f: &mut Frame, area: Rect) {
    let dirty_indicator = if app.editor.dirty { "*" } else { "" };
    let file_path = app.editor.path.as_ref().map_or_else(
        || app.lang.footer_no_file().to_string(),
        |p| format!("{}{}", p.to_string_lossy(), dirty_indicator),
    );

    let right_side_text = get_right_side_text(app);
    let editor_mode = if app.editor.overwrite_mode { "OVR" } else { "INS" };

    let footer_text = format!(
        " {} | {} | {} {}, {} {} | {} | {} ",
        file_path,
        editor_mode,
        app.lang.footer_line(),
        app.editor.cursor_row + 1,
        app.lang.footer_col(),
        app.editor.cursor_col + 1,
        "UTF-8",
        right_side_text
    );
    let footer = Paragraph::new(footer_text).style(Style::default().bg(ACCENT_COLOR).fg(Color::White));
    f.render_widget(footer, area);
}

fn get_right_side_text(app: &App) -> String {
    // Priority 1: Timed status message
    if let Some((msg, _)) = &app.status_message {
        return msg.clone();
    }

    // Priority 2: Diagnostics on the current line
    if let Some(path) = &app.editor.path {
        if let Ok(path_str) = path.to_str().ok_or(()) {
            if let Ok(uri) = format!("file://{}", path_str).parse::<lsp_types::Uri>() {
                if let Some(diagnostics) = app.diagnostics.get(&uri) {
                    for d in diagnostics {
                        if d.range.start.line as usize == app.editor.cursor_row {
                            return d.message.clone();
                        }
                    }
                }
            }
        }
    }
    
    // Default: Language toggle hint
    app.lang.footer_lang_toggle().to_string()
}