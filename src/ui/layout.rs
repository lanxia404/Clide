// src/ui/layout.rs

use crate::app::{ActivePanel, App, Focus};
use crate::components::{
    editor::view::render_editor,
    file_tree::view::render_file_tree,
    header::{render_header, BG_COLOR},
    popup::render_popup,
    status_bar::render_status_bar,
};
use crate::features::{
    git_view::render_git_panel,
    terminal_view::render_terminal_panel,
};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Style},
    widgets::Block,
    Frame,
};

const PANEL_HEIGHT: u16 = 10;
const MIN_WIDTH_FOR_DUAL_PANE: u16 = 80;

/// The main render function.
pub fn render(app: &mut App, f: &mut Frame) {
    // Create a base block to fill the background
    f.render_widget(Block::default().style(Style::default().bg(BG_COLOR)), f.area());

    // Define the main layout
    let panel_height = if app.active_panel.is_some() { PANEL_HEIGHT } else { 0 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),    // Header
            Constraint::Min(0),       // Content
            Constraint::Length(panel_height), // Panel (Terminal/Git)
            Constraint::Length(1),    // Footer
        ])
        .split(f.area());

    let header_area = chunks[0];
    let content_area = chunks[1];
    let panel_area = if panel_height > 0 { Some(chunks[2]) } else { None };
    let footer_area = chunks[3];

    // Render the main components
    render_header(app, f, header_area);
    render_status_bar(app, f, footer_area);

    // Render the content area, which can be single or dual pane
    render_content_area(app, f, content_area);

    // Render the active panel if there is one
    if let Some(panel_area) = panel_area && let Some(active_panel) = &app.active_panel {
        match active_panel {
            ActivePanel::Terminal => render_terminal_panel(f, panel_area, &app.terminal, app),
            ActivePanel::Git => render_git_panel(f, panel_area, &app.git, app),
        }
    }

    // Render any popups on top of everything else
    render_popup(app, f);
}

/// Renders the main content area, deciding between a single-pane or dual-pane view.
fn render_content_area(app: &mut App, f: &mut Frame, area: Rect) {
    if f.area().width < MIN_WIDTH_FOR_DUAL_PANE {
        // Single-pane view: render only the focused component
        match app.focus {
            Focus::FileTree => {
                app.file_tree_area = area;
                app.editor_area = Rect::default();
                render_file_tree(app, f, area);
            }
            Focus::Editor => {
                app.editor_area = area;
                app.file_tree_area = Rect::default();
                render_editor(app, f, area);
            }
        }
    } else {
        // Dual-pane view: render both file tree and editor
        let width_percent = app.file_tree_width_percent;
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(width_percent),
                Constraint::Percentage(100 - width_percent),
            ])
            .split(area);
        app.file_tree_area = content_chunks[0];
        app.editor_area = content_chunks[1];
        render_file_tree(app, f, app.file_tree_area);
        render_editor(app, f, app.editor_area);
    }
}