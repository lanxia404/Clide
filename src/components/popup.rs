use crate::app::App;
use lsp_types::{self, HoverContents, MarkedString};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use super::header::ACCENT_COLOR;

pub fn render_popup(app: &mut App, f: &mut Frame) {
    if app.completion_list.is_none() && app.hover_info.is_none() {
        return;
    }

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

    f.render_widget(Clear, popup_area);

    if app.completion_list.is_some() {
        render_completion_popup(f, app, popup_area, block);
    } else if app.hover_info.is_some() {
        render_hover_popup(f, app, popup_area, block);
    }
}

fn render_completion_popup(f: &mut Frame, app: &App, area: Rect, block: Block) {
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
                ListItem::new(item.label.as_str()).style(style)
            })
            .collect();

        let list_widget = List::new(list_items).block(block);
        f.render_widget(list_widget, area);
    }
}

fn render_hover_popup(f: &mut Frame, app: &App, area: Rect, block: Block) {
    if let Some(hover) = &app.hover_info {
        let text = match &hover.contents {
            HoverContents::Scalar(marked_string) => get_marked_string_text(marked_string),
            HoverContents::Array(marked_strings) => marked_strings
                .iter()
                .map(get_marked_string_text)
                .collect::<Vec<_>>()
                .join("\n"),
            HoverContents::Markup(_) => return, // Not handled for now
        };
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: true }).block(block);
        f.render_widget(paragraph, area);
    }
}

fn get_marked_string_text(marked_string: &MarkedString) -> String {
    match marked_string {
        MarkedString::String(text) => text.clone(),
        MarkedString::LanguageString(lang_string) => lang_string.value.clone(),
    }
}
