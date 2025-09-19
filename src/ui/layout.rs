use crate::app::{ActivePanel, App, Focus};
use crate::ui::git::render_git_panel;
use crate::ui::terminal::render_terminal_panel;
use lsp_types;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use url::Url;

use super::editor::render_editor;
use super::file_tree::render_file_tree;

pub const BG_COLOR: Color = Color::Rgb(21, 21, 21);
pub const TEXT_COLOR: Color = Color::Rgb(220, 220, 220);
pub const ACCENT_COLOR: Color = Color::Rgb(0, 122, 204);
pub const BAR_BG_COLOR: Color = Color::Rgb(37, 37, 38);

pub fn render(app: &mut App, f: &mut Frame) {
    f.render_widget(Block::default().style(Style::default().bg(BG_COLOR)), f.area());

    let (header_area, content_area, panel_area, footer_area) = if app.active_panel.is_some() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(10),
                Constraint::Length(1),
            ])
            .split(f.area());
        (chunks[0], chunks[1], Some(chunks[2]), chunks[3])
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)])
            .split(f.area());
        (chunks[0], chunks[1], None, chunks[2])
    };

    render_header(app, f, header_area);
    render_footer(app, f, footer_area);

    const MIN_WIDTH_FOR_DUAL_PANE: u16 = 80;
    if f.area().width < MIN_WIDTH_FOR_DUAL_PANE {
        match app.focus {
            Focus::FileTree => {
                app.file_tree_area = content_area;
                app.editor_area = Rect::default();
                render_file_tree(app, f, content_area);
            }
            Focus::Editor => {
                app.editor_area = content_area;
                app.file_tree_area = Rect::default();
                render_editor(app, f, content_area);
            }
        }
    } else {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
            .split(content_area);
        app.file_tree_area = content_chunks[0];
        app.editor_area = content_chunks[1];
        render_file_tree(app, f, app.file_tree_area);
        render_editor(app, f, app.editor_area);
    }

    if let Some(panel_area) = panel_area {
        if let Some(active_panel) = &app.active_panel {
            match active_panel {
                ActivePanel::Terminal => render_terminal_panel(f, panel_area, &app.terminal, app),
                ActivePanel::Git => render_git_panel(f, panel_area, &app.git, app),
            }
        }
    }

    render_lsp_popup(app, f);
}

fn render_header(app: &App, f: &mut Frame, area: Rect) {
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

fn render_footer(app: &App, f: &mut Frame, area: Rect) {
    let dirty_indicator = if app.editor.dirty { "*" } else { "" };
    let file_path = app.editor.path.as_ref().map_or_else(
        || app.lang.footer_no_file().to_string(),
        |p| format!("{}{}", p.to_string_lossy(), dirty_indicator),
    );

    let mut right_side_text = app.lang.footer_lang_toggle().to_string();
    if let Some(path) = &app.editor.path {
        if let Ok(uri) = Url::from_file_path(path) {
            if let Some(diagnostics) = app.diagnostics.get(&uri) {
                for d in diagnostics {
                    if d.range.start.line as usize == app.editor.cursor_row {
                        right_side_text = d.message.clone();
                        break;
                    }
                }
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

fn render_lsp_popup(app: &mut App, f: &mut Frame) {
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
        let list_items: Vec<ListItem> = items.iter().map(|item| ListItem::new(item.label.clone())).collect();
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
