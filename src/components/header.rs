use crate::app::App;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};

// Re-export color constants for other modules to use
pub const BG_COLOR: Color = Color::Rgb(21, 21, 21);
pub const TEXT_COLOR: Color = Color::Rgb(220, 220, 220);
pub const ACCENT_COLOR: Color = Color::Rgb(0, 122, 204);
pub const BAR_BG_COLOR: Color = Color::Rgb(37, 37, 38);

pub fn render_header(app: &App, f: &mut Frame, area: Rect) {
    let header_text = format!(
        " ☰ {}  {}  {}  {}  {}  {}  {} ",
        app.lang.header_file(),
        app.lang.header_edit(),
        app.lang.header_view(),
        app.lang.header_go(),
        app.lang.header_run(),
        app.lang.header_terminal(),
        app.lang.header_help()
    );
    let header = Paragraph::new(header_text).style(Style::default().bg(BAR_BG_COLOR).fg(TEXT_COLOR));
    f.render_widget(header, area);
}
