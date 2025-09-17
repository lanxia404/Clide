use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::definitions::{DividerKind, FocusArea, PaneKind, StatusControlKind};
use crate::file_tree::FileEntryKind;

const BG_PRIMARY: Color = Color::Rgb(0, 0, 0);
const BG_PANEL: Color = Color::Rgb(12, 12, 12);
const FG_PRIMARY: Color = Color::Rgb(190, 190, 190);
const FG_DIM: Color = Color::Rgb(128, 128, 128);

const BAR_BG: Color = Color::Rgb(23, 52, 127);
const BAR_TEXT: Color = Color::Rgb(235, 240, 255);
const BAR_HIGHLIGHT_BG: Color = Color::Rgb(73, 102, 177);
const BAR_HIGHLIGHT_TEXT: Color = Color::Rgb(255, 255, 255);

const MENU_BG: Color = Color::Rgb(79, 79, 79);
const MENU_BORDER: Color = Color::Rgb(208, 208, 208);
const MENU_TEXT: Color = Color::Rgb(240, 240, 240);
const MENU_HIGHLIGHT_BG: Color = Color::Rgb(220, 220, 220);
const MENU_HIGHLIGHT_TEXT: Color = Color::Rgb(30, 30, 30);

const BORDER_IDLE: Color = Color::Rgb(61, 120, 120);
const BORDER_FOCUS: Color = Color::Rgb(187, 94, 0);
const PANEL_HIGHLIGHT_BG: Color = Color::Rgb(120, 160, 255);
const EDITOR_CURSOR_BG: Color = Color::Rgb(40, 40, 40);
const EDITOR_HOVER_BG: Color = Color::Rgb(100, 130, 190);

fn cell_width(text: &str) -> u16 {
    UnicodeWidthStr::width(text).min(u16::MAX as usize) as u16
}

pub fn render(f: &mut Frame<'_>, app: &mut App) {
    let size = f.size();
    if size.width < 80 || size.height < 24 {
        let block = Paragraph::new("終端機視窗過小，請調整至至少 80x24。")
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title("Clide")
                    .borders(Borders::ALL)
                    .style(Style::default().fg(FG_PRIMARY).bg(MENU_BG)),
            )
            .style(Style::default().fg(FG_PRIMARY).bg(BG_PRIMARY));
        f.render_widget(block, size);
        return;
    }

    let base = Block::default().style(Style::default().bg(BG_PRIMARY));
    f.render_widget(base, size);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(size);
    let menu_area = vertical[0];
    let workspace = vertical[1];
    let status_area = vertical[2];

    app.layout.begin_frame(workspace);

    #[derive(Clone, Copy)]
    enum ColumnKind {
        Center,
        Pane(PaneKind),
    }

    let mut column_kinds = Vec::new();
    let mut constraints = Vec::new();
    let mut remaining_pct: i32 = 100;

    if app.layout.tree_visible {
        let mut pct = (app.layout.tree_ratio * 100.0).round() as i32;
        pct = pct.clamp(10, 60);
        pct = pct.min((remaining_pct - 20).max(10));
        constraints.push(Constraint::Percentage(pct as u16));
        column_kinds.push(ColumnKind::Pane(PaneKind::FileTree));
        remaining_pct -= pct;
    }

    let mut agent_pct_val = 0;
    if app.layout.agent_visible {
        let reserve = if remaining_pct > 20 {
            remaining_pct - 20
        } else {
            remaining_pct
        };
        agent_pct_val = (app.layout.agent_ratio * 100.0).round() as i32;
        agent_pct_val = agent_pct_val.clamp(12, reserve.max(12));
        remaining_pct -= agent_pct_val;
    }

    let center_pct = remaining_pct.max(10);
    constraints.push(Constraint::Percentage(center_pct as u16));
    column_kinds.push(ColumnKind::Center);

    if app.layout.agent_visible {
        constraints.push(Constraint::Percentage(agent_pct_val as u16));
        column_kinds.push(ColumnKind::Pane(PaneKind::Agent));
    }

    let column_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(workspace);

    let mut tree_area = None;
    let mut center_area = None;
    let mut agent_area = None;

    for (kind, area) in column_kinds.iter().zip(column_areas.iter().copied()) {
        match kind {
            ColumnKind::Pane(PaneKind::FileTree) => tree_area = Some(area),
            ColumnKind::Pane(PaneKind::Agent) => agent_area = Some(area),
            ColumnKind::Center => center_area = Some(area),
            _ => {}
        }
    }

    if let Some(area) = tree_area {
        app.layout.register_pane(PaneKind::FileTree, area);
    }
    if let Some(area) = agent_area {
        app.layout.register_pane(PaneKind::Agent, area);
    }
    if let Some(area) = center_area {
        app.layout.register_center_area(area);
    }

    if tree_area.is_some() {
        if let Some(center) = center_area {
            let x = center.x.saturating_sub(1);
            let width = if center.x > 0 { 2 } else { 1 };
            let divider = Rect {
                x,
                y: workspace.y,
                width,
                height: workspace.height,
            };
            app.layout
                .register_divider(DividerKind::TreeCenter, divider);
        }
    }

    if let Some(agent) = agent_area {
        if center_area.is_some() {
            let x = agent.x.saturating_sub(1);
            let width = if agent.x > 0 { 2 } else { 1 };
            let divider = Rect {
                x,
                y: workspace.y,
                width,
                height: workspace.height,
            };
            app.layout
                .register_divider(DividerKind::CenterAgent, divider);
        }
    }

    app.menu_bar.layout.reset(app.menu_bar.items.len());
    app.status_controls.clear();

    render_menu_bar(f, app, menu_area);
    if let Some(area) = tree_area.filter(|_| app.layout.tree_visible) {
        render_file_tree(f, app, area);
    }
    if let Some(area) = center_area {
        render_center(f, app, area);
    }
    if let Some(area) = agent_area.filter(|_| app.layout.agent_visible) {
        render_agent(f, app, area);
    }
    render_status_bar(f, app, status_area);

    if app.menu_bar.open {
        render_menu_dropdown(f, app, menu_area);
    }
}

fn render_menu_bar(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let menu_bar = &mut app.menu_bar;
    f.render_widget(Clear, area);
    let base = Block::default().style(Style::default().bg(BAR_BG));
    f.render_widget(base, area);

    let mut spans: Vec<Span> = Vec::new();
    let area_end = area.x.saturating_add(area.width);
    let mut cursor = area.x;

    for (idx, item) in menu_bar.items.iter().enumerate() {
        if cursor >= area_end {
            break;
        }
        let label = format!("[{}]", item.title);
        let width = cell_width(&label);
        let available = area_end.saturating_sub(cursor).max(1);
        let rect_width = width.min(available).max(1);
        let is_active = menu_bar.open && menu_bar.active_index == Some(idx);
        let mut style = Style::default().fg(BAR_TEXT).bg(BAR_BG);
        if is_active {
            style = style
                .fg(BAR_HIGHLIGHT_TEXT)
                .bg(BAR_HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(label, style));
        let rect = Rect {
            x: cursor,
            y: area.y,
            width: rect_width,
            height: 1,
        };
        menu_bar.layout.register_item(idx, rect);
        cursor = cursor.saturating_add(rect_width);
        if cursor < area_end {
            spans.push(Span::styled(" ", Style::default().bg(BAR_BG)));
            cursor = cursor.saturating_add(1);
        }
    }

    let line = Line::from(spans);
    let bar = Paragraph::new(line)
        .alignment(Alignment::Left)
        .style(Style::default().fg(BAR_TEXT).bg(BAR_BG));
    f.render_widget(bar, area);
}

fn render_menu_dropdown(f: &mut Frame<'_>, app: &mut App, menu_area: Rect) {
    let size = f.size();
    let menu_bar = &mut app.menu_bar;
    let Some(active_index) = menu_bar.active_index else {
        return;
    };
    let Some(item_rect) = menu_bar.layout.item_area(active_index) else {
        return;
    };
    let entries = &menu_bar.items[active_index].entries;
    if entries.is_empty() {
        return;
    }

    let max_len = entries
        .iter()
        .map(|entry| UnicodeWidthStr::width(entry.label))
        .max()
        .unwrap_or(0);
    let width = (max_len + 2 + 2) as u16; // [] + padding
    let available_width = size.width.saturating_sub(item_rect.x);
    if available_width == 0 {
        return;
    }
    let dropdown_width = width.min(available_width).max(6);
    let mut x = item_rect.x;
    if x + dropdown_width > size.width {
        x = size.width.saturating_sub(dropdown_width);
    }

    let y = menu_area.y.saturating_add(1);
    if y >= size.height {
        return;
    }

    let required_height = entries.len() as u16 + 2;
    let available_height = size.height.saturating_sub(y);
    if available_height < 3 {
        return;
    }
    let dropdown_height = required_height.min(available_height);

    let dropdown_area = Rect {
        x,
        y,
        width: dropdown_width,
        height: dropdown_height,
    };

    f.render_widget(Clear, dropdown_area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MENU_BORDER))
        .style(Style::default().fg(MENU_TEXT).bg(MENU_BG));
    f.render_widget(block, dropdown_area);

    let inner_width = dropdown_width.saturating_sub(2);
    let max_entries = dropdown_height.saturating_sub(2) as usize;
    for (idx, entry) in entries.iter().enumerate() {
        if idx >= max_entries {
            break;
        }
        let inner_rect = Rect {
            x: dropdown_area.x.saturating_add(1),
            y: dropdown_area.y.saturating_add(1 + idx as u16),
            width: inner_width,
            height: 1,
        };
        menu_bar
            .layout
            .register_entry(active_index, idx, inner_rect);
        let mut style = Style::default().fg(MENU_TEXT).bg(MENU_BG);
        if menu_bar.highlighted_entry == Some(idx) {
            style = Style::default()
                .fg(MENU_HIGHLIGHT_TEXT)
                .bg(MENU_HIGHLIGHT_BG)
                .add_modifier(Modifier::BOLD);
        }
        let label = format!("[{}]", entry.label);
        let paragraph = Paragraph::new(label)
            .style(style)
            .alignment(Alignment::Left);
        f.render_widget(paragraph, inner_rect);
    }
}

fn render_file_tree(f: &mut Frame<'_>, app: &App, area: Rect) {
    let current_dir_path = app.file_tree.current_dir();
    let current_dir = if let Ok(relative) = current_dir_path.strip_prefix(&app.workspace_root) {
        if relative.as_os_str().is_empty() {
            String::from("./")
        } else {
            format!("./{}", relative.display())
        }
    } else {
        current_dir_path.display().to_string()
    };

    let items: Vec<ListItem> = app
        .file_tree
        .entries()
        .iter()
        .map(|entry| {
            let mut line = String::new();
            match entry.kind {
                FileEntryKind::WorkspaceRoot => {
                    line.push_str("⌂ ./");
                }
                FileEntryKind::ParentLink => {
                    line.push_str("⬆ ../");
                }
                FileEntryKind::Directory => {
                    let indent = "  ".repeat(entry.depth);
                    line.push_str(&indent);
                    if entry.has_children {
                        line.push_str(if entry.expanded { "▾ " } else { "▸ " });
                    } else {
                        line.push_str("  ");
                    }
                    line.push_str("📁 ");
                    line.push_str(&entry.name);
                    line.push('/');
                }
                FileEntryKind::File => {
                    let indent = "  ".repeat(entry.depth);
                    line.push_str(&indent);
                    line.push_str("  📄 ");
                    line.push_str(&entry.name);
                }
            }
            ListItem::new(Line::from(vec![Span::styled(
                line,
                Style::default().fg(FG_PRIMARY),
            )]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.file_tree.selected_index()));

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled(
            format!("檔案樹 | {current_dir}"),
            Style::default().fg(FG_PRIMARY),
        ))
        .style(Style::default().bg(BG_PANEL));
    if app.focus == FocusArea::FileTree {
        block = block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
    }

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(BG_PANEL))
        .highlight_style(Style::default().bg(PANEL_HIGHLIGHT_BG).fg(Color::White));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_center(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let mut editor_area = None;
    let mut terminal_area = None;

    if app.layout.editor_visible && app.layout.terminal_visible {
        let mut editor_pct = (app.layout.editor_ratio * 100.0).round() as i32;
        editor_pct = editor_pct.clamp(20, 85);
        let mut terminal_pct = 100 - editor_pct;
        if terminal_pct < 15 {
            terminal_pct = 15;
            editor_pct = 100 - terminal_pct;
        }
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(editor_pct as u16),
                Constraint::Percentage(terminal_pct as u16),
            ])
            .split(area);
        editor_area = Some(rows[0]);
        terminal_area = Some(rows[1]);

        let divider = Rect {
            x: area.x,
            y: rows[1].y.saturating_sub(1),
            width: area.width,
            height: 2.min(area.height),
        };
        app.layout
            .register_divider(DividerKind::EditorTerminal, divider);
    } else if app.layout.editor_visible {
        editor_area = Some(area);
    } else if app.layout.terminal_visible {
        terminal_area = Some(area);
    }

    if let Some(editor_area) = editor_area {
        app.layout.register_pane(PaneKind::Editor, editor_area);
        render_editor(f, app, editor_area);
    }
    if let Some(terminal_area) = terminal_area {
        app.layout.register_pane(PaneKind::Terminal, terminal_area);
        render_terminal(f, app, terminal_area);
    }
}

fn render_editor(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let inner_height = area.height.saturating_sub(2) as usize;
    app.editor.set_viewport_height(inner_height);

    let start_line = app.editor.viewport_start();
    let lines = app.editor.lines_in_viewport();
    let (cursor_line, cursor_char_col) = app.editor.cursor();

    let gutter_width: u16 = 7; // "0000 │ "

    let hover_line = app.editor_hover_line;
    let text: Vec<Line> = lines
        .iter()
        .enumerate()
        .map(|(idx, content)| {
            let line_no = start_line + idx + 1;
            let gutter = format!(/* "{:>4} │ " */ "{:>4} │ ", line_no);
            let mut spans = vec![Span::styled(gutter, Style::default().fg(FG_DIM))];
            let absolute_line = start_line + idx;
            let style = if absolute_line == cursor_line {
                Style::default().bg(EDITOR_CURSOR_BG).fg(Color::White)
            } else if hover_line == Some(absolute_line) {
                Style::default().bg(EDITOR_HOVER_BG).fg(Color::White)
            } else {
                Style::default().fg(FG_PRIMARY)
            };
            let processed_content = content.replace('\t', "    ");
            spans.push(Span::styled(processed_content, style));
            Line::from(spans)
        })
        .collect();

    let file_display = app
        .editor
        .file_path()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "[暫存]".to_string());

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled(
            format!("編輯器 | {}", file_display),
            Style::default().fg(FG_PRIMARY),
        ))
        .style(Style::default().bg(BG_PANEL));
    if app.focus == FocusArea::Editor {
        block = block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
    }

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((0, 0))
        .style(Style::default().bg(BG_PANEL));

    f.render_widget(paragraph, area);

    if app.focus == FocusArea::Editor {
        let line_slice = app.editor.line(cursor_line);
        let portion_before_cursor = line_slice.slice(0..cursor_char_col);

        let mut visual_col = 0;
        for ch in portion_before_cursor.chars() {
            visual_col += if ch == '\t' { 4 } else { 1 };
        }

        let cursor_y = area.y + (cursor_line.saturating_sub(start_line)) as u16 + 1;
        let cursor_x = area.x + gutter_width + visual_col as u16;
        if cursor_y < area.y + area.height && cursor_x < area.x + area.width {
            f.set_cursor(cursor_x, cursor_y);
        }
    }
}


fn render_terminal(f: &mut Frame<'_>, app: &App, area: Rect) {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled("終端機", Style::default().fg(FG_PRIMARY)))
        .style(Style::default().bg(BG_PANEL));
    if app.focus == FocusArea::Terminal {
        block = block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
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
        .map(|line| Line::from(Span::styled(line, Style::default().fg(FG_PRIMARY))))
        .collect();

    let paragraph = Paragraph::new(text)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(BG_PANEL));
    f.render_widget(paragraph, area);
}

fn render_agent(f: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .agent
        .messages()
        .iter()
        .map(|msg| {
            let lines = vec![
                Line::from(Span::styled(
                    msg.title.clone(),
                    Style::default().fg(Color::White),
                )),
                Line::from(Span::styled(
                    msg.detail.clone(),
                    Style::default().fg(FG_DIM),
                )),
            ];
            ListItem::new(lines)
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.agent.selected_index()));

    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled("代理面板", Style::default().fg(FG_PRIMARY)))
        .style(Style::default().bg(BG_PANEL));
    if app.focus == FocusArea::Agent {
        block = block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
    }

    let list = List::new(items)
        .block(block)
        .style(Style::default().bg(BG_PANEL))
        .highlight_style(Style::default().bg(PANEL_HIGHLIGHT_BG).fg(Color::White));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_status_bar(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let (cursor_line, cursor_col) = app.editor.cursor();
    let cursor_display = format!("{}:{}", cursor_line + 1, cursor_col + 1);
    let dirty_indicator = if app.editor.is_dirty() { "*" } else { "OK" };

    let segments: Vec<(Option<StatusControlKind>, String)> = vec![
        (
            Some(StatusControlKind::Wrap),
            format!("[WRAP:{}]", app.preferences.wrap_mode.abbr()),
        ),
        (
            Some(StatusControlKind::LineEnding),
            format!("[EOL:{}]", app.preferences.line_ending.label()),
        ),
        (
            Some(StatusControlKind::Encoding),
            format!("[ENC:{}]", app.preferences.encoding.abbr()),
        ),
        (
            Some(StatusControlKind::Indent),
            format!("[IND:{}]", app.preferences.indent.abbr()),
        ),
        (
            Some(StatusControlKind::Cursor),
            format!("[POS:{}]", cursor_display),
        ),
        (
            Some(StatusControlKind::Dirty),
            format!("[SAVE:{}]", dirty_indicator),
        ),
    ];

    f.render_widget(Clear, area);
    let base = Block::default().style(Style::default().bg(BAR_BG));
    f.render_widget(base, area);

    let mut spans: Vec<Span> = Vec::new();
    let mut cursor = area.x;
    let area_end = area.x.saturating_add(area.width);

    for (kind, text) in segments.into_iter() {
        if cursor >= area_end {
            break;
        }
        let width = cell_width(&text);
        let available = area_end.saturating_sub(cursor).max(1);
        let rect_width = width.min(available).max(1);
        let style = Style::default().fg(BAR_TEXT).bg(BAR_BG);
        spans.push(Span::styled(text.clone(), style));
        let rect = Rect {
            x: cursor,
            y: area.y,
            width: rect_width,
            height: 1,
        };
        if let Some(kind) = kind {
            app.status_controls.register(kind, rect);
        }
        cursor = cursor.saturating_add(rect_width);
        if cursor < area_end {
            spans.push(Span::styled(" ", Style::default().bg(BAR_BG)));
            cursor = cursor.saturating_add(1);
        }
    }

    let paragraph = Paragraph::new(Line::from(spans))
        .style(Style::default().fg(BAR_TEXT).bg(BAR_BG))
        .alignment(Alignment::Left);
    f.render_widget(paragraph, area);
}
