use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

use crate::app::{App, FocusArea};

pub fn render(f: &mut Frame<'_>, app: &mut App) {
    let size = f.size();
    if size.width < 80 || size.height < 24 {
        let block = Paragraph::new("Terminal window is too small. Resize to 80x24 or larger.")
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .block(Block::default().title("Clide").borders(Borders::ALL));
        f.render_widget(block, size);
        return;
    }

    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(size);
    let workspace = outer[0];
    let status_area = outer[1];

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24),
            Constraint::Percentage(55),
            Constraint::Percentage(20),
        ])
        .split(workspace);

    let tree_area = columns[0];
    let center_area = columns[1];
    let agent_area = columns[2];

    render_file_tree(f, app, tree_area);
    render_center(f, app, center_area);
    render_agent(f, app, agent_area);
    render_status(f, app, status_area);
}

fn render_file_tree(f: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .file_tree
        .entries()
        .iter()
        .map(|entry| {
            let indent = "  ".repeat(entry.depth);
            let prefix = if entry.is_dir { "[d]" } else { "   " };
            ListItem::new(format!("{}{} {}", indent, prefix, entry.name))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.file_tree.selected_index()));

    let mut block = Block::default().borders(Borders::ALL).title("Files");
    if app.focus == FocusArea::FileTree {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_center(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ])
        .split(area);
    let editor_area = rows[0];
    let terminal_area = rows[1];

    render_editor(f, app, editor_area);
    render_terminal(f, app, terminal_area);
}

fn render_editor(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize;
    app.editor.set_viewport_height(inner_height);

    let start_line = app.editor.viewport_start();
    let lines = app.editor.lines_in_viewport();
    let (cursor_line, cursor_col) = app.editor.cursor();

    let gutter_width: u16 = 7; // "0000 | "

    let text: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(idx, content)| {
            let line_no = start_line + idx + 1;
            let gutter = format!("{:>4} | ", line_no);
            let mut spans = vec![Span::styled(gutter, Style::default().fg(Color::DarkGray))];
            let style = if start_line + idx == cursor_line {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            spans.push(Span::styled(content.clone(), style));
            Line::from(spans)
        })
        .collect();

    let mut block = Block::default().borders(Borders::ALL).title("Editor");
    if app.focus == FocusArea::Editor {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((0, 0));

    f.render_widget(paragraph, area);

    if app.focus == FocusArea::Editor {
        let cursor_y = area.y + (cursor_line.saturating_sub(start_line)) as u16 + 1;
        let cursor_x = area.x + gutter_width + cursor_col as u16 + 1;
        if cursor_y < area.y + area.height && cursor_x < area.x + area.width {
            f.set_cursor(cursor_x, cursor_y);
        }
    }
}

fn render_terminal(f: &mut Frame<'_>, app: &App, area: Rect) {
    let mut block = Block::default().borders(Borders::ALL).title("Terminal");
    if app.focus == FocusArea::Terminal {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }

    let height = area.height.saturating_sub(2) as usize;
    let lines = app.terminal.lines();
    let offset = app.terminal.scroll_offset();
    let total = lines.len();
    let start = total.saturating_sub(height + offset).min(total);
    let end = total.saturating_sub(offset).min(total);
    let visible: Vec<String> = if start < end {
        lines[start..end].to_vec()
    } else {
        Vec::new()
    };

    let text: Vec<Line> = visible
        .into_iter()
        .map(|line| Line::from(Span::raw(line)))
        .collect();

    let paragraph = Paragraph::new(text).block(block).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_agent(f: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .agent
        .messages()
        .iter()
        .map(|msg| {
            let content = format!("{}\n{}", msg.title, msg.detail);
            ListItem::new(content)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.agent.selected_index()));

    let mut block = Block::default().borders(Borders::ALL).title("Agent");
    if app.focus == FocusArea::Agent {
        block = block.border_style(Style::default().fg(Color::Cyan));
    }

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_status(f: &mut Frame<'_>, app: &App, area: Rect) {
    let focus = app.focus.label();
    let file_label = app
        .editor
        .file_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| String::from("untitled"));
    let status_text = format!(
        "Focus: {} | File: {} | Root: {} | {}",
        focus,
        file_label,
        app.workspace_root.display(),
        app.status_message
    );

    let paragraph = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}
