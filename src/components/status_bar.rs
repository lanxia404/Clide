use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};
use url::Url;
use super::header::{ACCENT_COLOR};

pub fn render_status_bar(app: &App, f: &mut Frame, area: Rect) {
    let dirty_indicator = if app.editor.dirty { "*" } else { "" };
    let file_path = app.editor.path.as_ref().map_or_else(
        || app.lang.footer_no_file().to_string(),
        |p| format!("{}{}", p.to_string_lossy(), dirty_indicator),
    );

    let mut right_side_text = app.lang.footer_lang_toggle().to_string();

    if let Some(msg) = &app.lsp_message {
        right_side_text = msg.clone();
    } else if let Some(path) = &app.editor.path && let Ok(uri) = Url::from_file_path(path) && let Some(diagnostics) = app.diagnostics.get(&uri) {
        for d in diagnostics {
            if d.range.start.line as usize == app.editor.cursor_row {
                right_side_text = d.message.clone();
                break;
            }
        }
    }

    let footer_text = format!(
        " {} | {} {}, {} {} | {} | {} ",
        file_path,
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
