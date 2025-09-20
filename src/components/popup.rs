use crate::app::App;
use lsp_types;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use super::header::ACCENT_COLOR;

pub fn render_popup(app: &mut App, f: &mut Frame) {
    let block = Block::default()
        .title("LSP Info")
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Double)
        .style(Style::default().bg(Color::DarkGray));

    let area = app.editor_area;
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = 10.min(area.height.saturating_sub(4));
    let popup_area = Rect {
        x: area.x + (area.width - popup_width) / 2,
        y: area.y + (area.height - popup_height) / 2,
        width: popup_width,
        height: popup_height,
    };

    if let Some(items) = &app.completion_list {
        let list_items: Vec<ListItem> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if app.completion_selection == Some(i) {
                    Style::default().bg(ACCENT_COLOR).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(item.label.clone()).style(style)
            })
            .collect();

        let list_widget = List::new(list_items).block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(list_widget, popup_area);
    } else if let Some(hover) = &app.hover_info {
        let text = match &hover.contents {
            lsp_types::HoverContents::Scalar(marked_string) => match marked_string {
                lsp_types::MarkedString::String(text) => text.clone(),
                lsp_types::MarkedString::LanguageString(lang_string) => lang_string.value.clone(),
            },
            lsp_types::HoverContents::Array(marked_strings) => marked_strings
                .iter()
                .map(|s| match s {
                    lsp_types::MarkedString::String(text) => text.clone(),
                    lsp_types::MarkedString::LanguageString(lang_string) => lang_string.value.clone(),
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => return,
        };
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);
        f.render_widget(Clear, popup_area);
        f.render_widget(paragraph, popup_area);
    }
}
