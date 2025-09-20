use crate::app::App;
use super::git::GitState;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_git_panel(f: &mut Frame, area: Rect, state: &GitState, app: &App) {
    let is_active = app.active_panel == Some(crate::app::ActivePanel::Git);

    let border_style = if is_active {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title(format!(" Git (Branch: {})", state.current_branch))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(border_style);

    let text = if state.status.is_empty() {
        "No changes detected.".to_string()
    } else {
        state.status.join("\n")
    };

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}