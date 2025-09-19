use crate::app::App;
use crate::terminal::TerminalState;
use ratatui::{
    layout::Rect,
    style::{Color, Style, Stylize},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

pub fn render_terminal_panel(f: &mut Frame, area: Rect, state: &TerminalState, app: &App) {
    let border_style = if app.active_panel == Some(crate::app::ActivePanel::Terminal) {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::White)
    };

    let block = Block::default()
        .title(" Terminal ")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .border_style(border_style);

    // Combine output buffer and current input line for display
    let mut output_text = state.output_buffer.join("\n");
    output_text.push_str("\n> ");
    output_text.push_str(&state.input_line);

    let paragraph = Paragraph::new(output_text)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);

    // Set cursor for the input line
    if app.active_panel == Some(crate::app::ActivePanel::Terminal) {
        f.set_cursor(
            area.x + 2 + state.input_line.len() as u16,
            area.y + 1 + state.output_buffer.len() as u16,
        );
    }
}