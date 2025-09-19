use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::agent::AgentPanelEntry;
use crate::app::{
    AgentSwitcherState, App, CommandPaletteState, ConfirmDeleteState, InputPromptState,
    OverlayState,
};
use crate::definitions::{DividerKind, FocusArea, PaneKind, StatusControlKind};
use crate::file_tree::FileEntryKind;

mod theme;
use theme::*;

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

    app.layout.calculate(workspace);

    app.menu_bar.layout.reset(app.menu_bar.items.len());
    app.status_controls.clear();

    render_menu_bar(f, app, menu_area);
    if let Some(geom) = app.layout.pane_geometry(PaneKind::FileTree) {
        render_file_tree(f, app, geom.area);
    }
    if let Some(area) = app.layout.center_area() {
        render_center(f, app, area);
    }
    if let Some(geom) = app.layout.pane_geometry(PaneKind::Agent) {
        render_agent(f, app, geom.area);
    }
    render_status_bar(f, app, status_area);

    if app.menu_bar.open {
        render_menu_dropdown(f, app, menu_area);
    }

    if let Some(overlay) = app.overlay.as_ref() {
        render_overlay(f, app, overlay);
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

    let inner = block.inner(area);
    f.render_widget(block, area);

    render_editor_lines(f, app, inner);
}

fn render_editor_lines(f: &mut Frame<'_>, app: &mut App, area: Rect) {
    let gutter_width: u16 = 7; // "0000 │ "
    let inner_height = area.height as usize;
    let text_width = area.width.saturating_sub(gutter_width).max(1) as usize;

    app.editor.set_viewport(inner_height, text_width);

    let start_line = app.editor.viewport_start();
    let mut line_index = start_line;
    let mut offset = app.editor.viewport_line_offset();
    let total_lines = app.editor.total_lines();
    let hover_line = app.editor_hover_line;
    let (cursor_line, cursor_subline, cursor_col) = app.editor.cursor_visual_position();

    let mut rows: Vec<Line> = Vec::new();
    let mut cursor_row: Option<usize> = None;

    while rows.len() < inner_height && line_index < total_lines {
        let selection_ranges = app.editor.selection_display_ranges(line_index);
        let segments = app.editor.visual_segments(line_index, text_width);
        for (segment_idx, segment) in segments.into_iter().enumerate() {
            if offset > 0 {
                offset -= 1;
                continue;
            }

            let highlight = line_index == cursor_line;
            let hover = hover_line == Some(line_index);
            let gutter_text = if segment_idx == 0 {
                format!("{:>4} │ ", line_index + 1)
            } else {
                String::from("     │ ")
            };

            let mut spans = Vec::new();
            let gutter_style = Style::default().fg(GUTTER_FG).bg(if highlight {
                EDITOR_LINE_HIGHLIGHT_BG
            } else {
                BG_PANEL
            });
            spans.push(Span::styled(gutter_text, gutter_style));

            let mut padded_segment = segment;
            if padded_segment.len() < text_width {
                padded_segment.push_str(&" ".repeat(text_width - padded_segment.len()));
            }

            let segment_start = segment_idx * text_width;
            let segment_end = segment_start + text_width;
            let overlaps: Vec<(usize, usize)> = selection_ranges
                .iter()
                .filter_map(|(start, end)| {
                    let overlap_start = (*start).max(segment_start);
                    let overlap_end = (*end).min(segment_end);
                    if overlap_start < overlap_end {
                        Some((overlap_start - segment_start, overlap_end - segment_start))
                    } else {
                        None
                    }
                })
                .collect();

            for (chunk, selected) in split_segment_by_ranges(app, &padded_segment, &overlaps) {
                if chunk.is_empty() {
                    continue;
                }
                let mut style = if highlight {
                    Style::default()
                        .fg(EDITOR_LINE_HIGHLIGHT_FG)
                        .bg(EDITOR_LINE_HIGHLIGHT_BG)
                } else if hover {
                    Style::default().fg(Color::White).bg(EDITOR_HOVER_BG)
                } else {
                    Style::default().fg(FG_PRIMARY).bg(BG_PANEL)
                };
                if selected {
                    style = Style::default()
                        .fg(EDITOR_SELECTION_FG)
                        .bg(EDITOR_SELECTION_BG);
                }
                spans.push(Span::styled(chunk, style));
            }

            rows.push(Line::from(spans));
            if highlight && segment_idx == cursor_subline {
                cursor_row = Some(rows.len() - 1);
            }

            if rows.len() >= inner_height {
                break;
            }
        }

        offset = 0;
        line_index += 1;
    }

    while rows.len() < inner_height {
        let gutter_style = Style::default().fg(GUTTER_FG).bg(BG_PANEL);
        let spans = vec![
            Span::styled("     │ ".to_string(), gutter_style),
            Span::styled(
                " ".repeat(text_width),
                Style::default().fg(FG_DIM).bg(BG_PANEL),
            ),
        ];
        rows.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(rows)
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(BG_PANEL));

    f.render_widget(paragraph, area);

    if app.focus == FocusArea::Editor
        && let Some(row) = cursor_row {
            let cursor_y = area.y + row as u16;
            let cursor_x = area.x + gutter_width + cursor_col as u16;
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
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(4)])
        .split(area);
    let history_area = sections[0];
    let input_area = sections[1];

    render_agent_history(f, app, history_area);
    render_agent_input(f, app, input_area);
}

fn render_agent_history(f: &mut Frame<'_>, app: &App, area: Rect) {
    let mut history_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled("代理對話", Style::default().fg(FG_PRIMARY)))
        .style(Style::default().bg(BG_PANEL));
    if app.focus == FocusArea::Agent {
        history_block = history_block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
    }

    let wrap_width = history_block.inner(area).width.max(1) as usize;

    let items: Vec<ListItem> = app
        .agent
        .entries()
        .iter()
        .map(|entry| match entry {
            AgentPanelEntry::UserPrompt { prompt } => {
                let mut lines = Vec::new();
                lines.push(Line::from(Span::styled(
                    "你",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )));
                if prompt.is_empty() {
                    push_wrapped_line(
                        &mut lines,
                        "",
                        Style::default().fg(Color::White),
                        wrap_width,
                    );
                } else {
                    for line in prompt.lines() {
                        push_wrapped_line(
                            &mut lines,
                            line,
                            Style::default().fg(Color::White),
                            wrap_width,
                        );
                    }
                }
                ListItem::new(lines)
            }
            AgentPanelEntry::Response(msg) => {
                let mut lines = vec![Line::from(Span::styled(
                    msg.title.clone(),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ))];
                if msg.detail.is_empty() {
                    push_wrapped_line(&mut lines, "", Style::default().fg(FG_DIM), wrap_width);
                } else {
                    for line in msg.detail.lines() {
                        push_wrapped_line(
                            &mut lines,
                            line,
                            Style::default().fg(FG_DIM),
                            wrap_width,
                        );
                    }
                }
                if let Some(file) = msg.file.as_ref() {
                    let line = msg
                        .line
                        .map(|l| (l + 1).to_string())
                        .unwrap_or_else(|| "?".into());
                    push_wrapped_line(
                        &mut lines,
                        &format!("檔案：{} (行 {})", file, line),
                        Style::default().fg(FG_DIM),
                        wrap_width,
                    );
                }
                if let Some(patch) = msg.patch.as_ref()
                    && !patch.trim().is_empty() {
                        push_wrapped_line(
                            &mut lines,
                            "建議變更：",
                            Style::default().fg(FG_DIM),
                            wrap_width,
                        );
                        for line in patch.lines() {
                            push_wrapped_line(
                                &mut lines,
                                line,
                                Style::default().fg(Color::Yellow),
                                wrap_width,
                            );
                        }
                    }
                ListItem::new(lines)
            }
            AgentPanelEntry::Info { title, detail } => {
                let mut lines = vec![Line::from(Span::styled(
                    title.clone(),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::ITALIC),
                ))];
                if detail.is_empty() {
                    push_wrapped_line(&mut lines, "", Style::default().fg(FG_DIM), wrap_width);
                } else {
                    for line in detail.lines() {
                        push_wrapped_line(
                            &mut lines,
                            line,
                            Style::default().fg(FG_DIM),
                            wrap_width,
                        );
                    }
                }
                ListItem::new(lines)
            }
            AgentPanelEntry::Error { title, detail } => {
                let mut lines = vec![Line::from(Span::styled(
                    title.clone(),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ))];
                if detail.is_empty() {
                    push_wrapped_line(
                        &mut lines,
                        "",
                        Style::default().fg(Color::LightRed),
                        wrap_width,
                    );
                } else {
                    for line in detail.lines() {
                        push_wrapped_line(
                            &mut lines,
                            line,
                            Style::default().fg(Color::LightRed),
                            wrap_width,
                        );
                    }
                }
                ListItem::new(lines)
            }
            AgentPanelEntry::ToolOutput { tool, detail } => {
                let mut lines = vec![Line::from(Span::styled(
                    format!("工具：{}", tool),
                    Style::default().fg(Color::Green),
                ))];
                if detail.is_empty() {
                    push_wrapped_line(&mut lines, "", Style::default().fg(FG_DIM), wrap_width);
                } else {
                    for line in detail.lines() {
                        push_wrapped_line(
                            &mut lines,
                            line,
                            Style::default().fg(FG_DIM),
                            wrap_width,
                        );
                    }
                }
                ListItem::new(lines)
            }
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.agent.selected_index()));

    let list = List::new(items)
        .block(history_block)
        .style(Style::default().bg(BG_PANEL))
        .highlight_style(
            Style::default()
                .bg(EDITOR_HOVER_BG)
                .fg(EDITOR_LINE_HIGHLIGHT_FG)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut state);
}

fn render_agent_input(f: &mut Frame<'_>, app: &App, area: Rect) {
    let mut input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER_IDLE))
        .title(Span::styled(
            "輸入訊息 (Enter 傳送 / Shift+Enter 換行)",
            Style::default().fg(FG_PRIMARY),
        ))
        .style(Style::default().bg(BG_PANEL));

    if app.focus == FocusArea::Agent {
        input_block = input_block.border_style(
            Style::default()
                .fg(BORDER_FOCUS)
                .add_modifier(Modifier::BOLD),
        );
    }

    let input_inner = input_block.inner(area);
    let input_lines: Vec<Line> = if app.agent_input.is_empty() {
        vec![Line::from(Span::styled(
            "輸入指令或提問給代理…",
            Style::default().fg(FG_DIM),
        ))]
    } else {
        app.agent_input
            .buffer()
            .lines()
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(FG_PRIMARY),
                ))
            })
            .collect()
    };

    let input_paragraph = Paragraph::new(input_lines)
        .block(input_block)
        .style(Style::default().bg(BG_PANEL))
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Left);

    f.render_widget(Clear, area);
    f.render_widget(input_paragraph, area);

    if app.focus == FocusArea::Agent {
        let width = input_inner.width.max(1) as usize;
        let (cursor_col, cursor_row) = app.agent_input.cursor_display_position(width);
        let cursor_x = input_inner
            .x
            .saturating_add(cursor_col.min(width.saturating_sub(1) as u16));
        let cursor_y = input_inner
            .y
            .saturating_add(cursor_row.min(input_inner.height.saturating_sub(1)));
        f.set_cursor(cursor_x, cursor_y);
    }
}

fn push_wrapped_line(lines: &mut Vec<Line>, text: &str, style: Style, width: usize) {
    for segment in wrap_to_width(text, width) {
        lines.push(Line::from(Span::styled(segment, style)));
    }
}

fn wrap_to_width(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1).max(1);
        if current_width + ch_width > width && !current.is_empty() {
            result.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }
    result.push(current);
    result
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
            Some(StatusControlKind::HiddenFiles),
            format!(
                "[HID:{}]",
                if app.file_tree.show_hidden() {
                    "ON"
                } else {
                    "OFF"
                }
            ),
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

fn render_overlay(f: &mut Frame<'_>, app: &App, overlay: &OverlayState) {
    match overlay {
        OverlayState::CommandPalette(state) => render_command_palette_overlay(f, state),
        OverlayState::InputPrompt(state) => render_input_prompt_overlay(f, app, state),
        OverlayState::ConfirmDelete(state) => render_confirm_delete_overlay(f, state),
        OverlayState::AgentSwitcher(state) => render_agent_switcher_overlay(f, state),
    }
}

fn split_segment_by_ranges(
    app: &App,
    segment: &str,
    overlaps: &[(usize, usize)],
) -> Vec<(String, bool)> {
    let mut result: Vec<(String, bool)> = Vec::new();
    let mut overlap_iter = overlaps.iter().peekable();
    let mut display_pos = 0;
    let mut current = String::new();
    let mut current_selected = false;

    for ch in segment.chars() {
        while let Some((_, end)) = overlap_iter.peek() {
            if display_pos >= *end {
                overlap_iter.next();
            } else {
                break;
            }
        }
        let selected = overlap_iter
            .peek()
            .map(|(start, end)| display_pos >= *start && display_pos < *end)
            .unwrap_or(false);

        if current.is_empty() {
            current_selected = selected;
        } else if selected != current_selected {
            result.push((std::mem::take(&mut current), current_selected));
            current_selected = selected;
        }

        current.push(ch);
        display_pos += app.editor.display_width(ch);
    }

    if !current.is_empty() {
        result.push((current, current_selected));
    }

    if result.is_empty() {
        result.push((String::new(), false));
    }

    result
}

fn render_command_palette_overlay(f: &mut Frame<'_>, state: &CommandPaletteState) {
    let area = centered_rect(60, 70, f.size());
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(Span::styled(
            "指令面板",
            Style::default().fg(BAR_TEXT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MENU_BORDER))
        .style(Style::default().bg(MENU_BG));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .split(inner);

    let filter_line = if state.filter.is_empty() {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(FG_PRIMARY)),
            Span::styled("輸入關鍵字以篩選", Style::default().fg(FG_DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(FG_PRIMARY)),
            Span::styled(
                state.filter.as_str(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };
    let filter = Paragraph::new(filter_line)
        .style(Style::default().bg(MENU_BG))
        .alignment(Alignment::Left);
    f.render_widget(filter, chunks[0]);

    let max_visible = state.visible.len().min(chunks[1].height as usize);
    let mut list_items: Vec<ListItem> = Vec::new();
    let mut list_state = ListState::default();
    if max_visible > 0 {
        let selected = state.selected.min(state.visible.len().saturating_sub(1));
        let start = if selected >= max_visible {
            selected + 1 - max_visible
        } else {
            0
        };
        for (offset, idx) in state
            .visible
            .iter()
            .enumerate()
            .skip(start)
            .take(max_visible)
        {
            let entry = &state.entries[*idx];
            let mut spans = vec![Span::styled(
                entry.label.clone(),
                Style::default().fg(FG_PRIMARY),
            )];
            if let Some(detail) = entry.detail.as_ref() {
                spans.push(Span::styled(
                    format!("  {detail}"),
                    Style::default().fg(FG_DIM),
                ));
            }
            list_items.push(ListItem::new(Line::from(spans)));
            if offset == selected {
                list_state.select(Some(offset - start));
            }
        }
    }

    let list = List::new(list_items)
        .block(Block::default().style(Style::default().bg(MENU_BG)))
        .highlight_style(
            Style::default()
                .bg(PANEL_HIGHLIGHT_BG)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(MENU_BG));
    f.render_stateful_widget(list, chunks[1], &mut list_state);

    let footer_text = if let Some(entry) = state.selected_entry() {
        if let Some(detail) = entry.detail.as_ref() {
            format!("Enter 執行 · Esc 關閉 · 快捷：{detail}")
        } else {
            String::from("Enter 執行 · Esc 關閉")
        }
    } else if state.visible.is_empty() {
        String::from("無符合項目 · Esc 關閉")
    } else {
        String::from("Enter 執行 · Esc 關閉")
    };
    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(FG_DIM).bg(MENU_BG))
        .alignment(Alignment::Left);
    f.render_widget(footer, chunks[2]);
}

fn render_input_prompt_overlay(f: &mut Frame<'_>, app: &App, state: &InputPromptState) {
    let area = centered_rect(60, 30, f.size());
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(Span::styled(
            state.title.as_str(),
            Style::default().fg(BAR_TEXT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MENU_BORDER))
        .style(Style::default().bg(MENU_BG));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let placeholder =
        Paragraph::new(state.placeholder.as_str()).style(Style::default().fg(FG_DIM).bg(MENU_BG));
    f.render_widget(placeholder, chunks[0]);

    let mut input_spans = vec![Span::styled("> ", Style::default().fg(FG_PRIMARY))];
    if state.value.is_empty() {
        input_spans.push(Span::styled("(尚未輸入)", Style::default().fg(FG_DIM)));
    } else {
        input_spans.push(Span::styled(
            state.value.as_str(),
            Style::default().fg(Color::White),
        ));
    }
    input_spans.push(Span::styled(" ▍", Style::default().fg(BORDER_FOCUS)));
    let input = Paragraph::new(Line::from(input_spans))
        .style(Style::default().bg(MENU_BG))
        .alignment(Alignment::Left);
    f.render_widget(input, chunks[1]);

    let workspace_hint = Paragraph::new(format!("工作目錄：{}", app.workspace_root.display()))
        .style(Style::default().fg(FG_DIM).bg(MENU_BG));
    f.render_widget(workspace_hint, chunks[2]);

    let message_area = chunks[3];
    if let Some(error) = state.error.as_ref() {
        let error_widget =
            Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red).bg(MENU_BG));
        f.render_widget(error_widget, message_area);
    } else {
        let hint =
            Paragraph::new("Enter 確認 · Esc 取消").style(Style::default().fg(FG_DIM).bg(MENU_BG));
        f.render_widget(hint, message_area);
    }
}

fn render_agent_switcher_overlay(f: &mut Frame<'_>, state: &AgentSwitcherState) {
    let area = centered_rect(50, 60, f.size());
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(Span::styled(
            "選擇代理",
            Style::default().fg(BAR_TEXT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MENU_BORDER))
        .style(Style::default().bg(MENU_BG));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(inner);

    let items: Vec<ListItem> = if state.profiles.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "沒有可用設定",
            Style::default().fg(FG_DIM),
        )))]
    } else {
        state
            .profiles
            .iter()
            .map(|profile| {
                let mut lines = vec![Line::from(Span::styled(
                    profile.label.clone(),
                    Style::default().fg(FG_PRIMARY).add_modifier(Modifier::BOLD),
                ))];
                if let Some(detail) = profile.description.as_ref() {
                    lines.push(Line::from(Span::styled(
                        detail.clone(),
                        Style::default().fg(FG_DIM),
                    )));
                }
                ListItem::new(lines)
            })
            .collect()
    };

    let mut list_state = ListState::default();
    if !state.profiles.is_empty() {
        list_state.select(Some(state.selected.min(state.profiles.len() - 1)));
    }

    let list = List::new(items)
        .style(Style::default().bg(MENU_BG))
        .highlight_style(
            Style::default()
                .bg(MENU_HIGHLIGHT_BG)
                .fg(MENU_HIGHLIGHT_TEXT),
        );

    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let instructions = Paragraph::new(Line::from(vec![Span::styled(
        "上/下鍵選擇，Enter 套用，Esc 取消",
        Style::default().fg(FG_DIM),
    )]))
    .style(Style::default().bg(MENU_BG))
    .alignment(Alignment::Center);
    f.render_widget(instructions, chunks[1]);
}

fn render_confirm_delete_overlay(f: &mut Frame<'_>, state: &ConfirmDeleteState) {
    let area = centered_rect(50, 28, f.size());
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(Span::styled(
            "確認刪除",
            Style::default().fg(BAR_TEXT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MENU_BORDER))
        .style(Style::default().bg(MENU_BG));
    f.render_widget(block.clone(), area);
    let inner = block.inner(area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let message = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("即將刪除：", Style::default().fg(FG_DIM)),
            Span::styled(state.display.as_str(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![Span::styled(
            "此動作無法復原，確定要刪除此檔案嗎？",
            Style::default().fg(FG_DIM),
        )]),
    ])
    .style(Style::default().bg(MENU_BG));
    f.render_widget(message, chunks[0]);

    let buttons = {
        let confirm_style = if state.selected_index == 0 {
            Style::default()
                .bg(PANEL_HIGHLIGHT_BG)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(BAR_TEXT)
        };
        let cancel_style = if state.selected_index == 1 {
            Style::default()
                .bg(PANEL_HIGHLIGHT_BG)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(BAR_TEXT)
        };
        Paragraph::new(Line::from(vec![
            Span::styled(" [刪除] ", confirm_style),
            Span::styled("  ", Style::default().bg(MENU_BG)),
            Span::styled(" [取消] ", cancel_style),
        ]))
        .style(Style::default().bg(MENU_BG))
        .alignment(Alignment::Center)
    };
    f.render_widget(buttons, chunks[1]);

    let checkbox = Paragraph::new(Line::from(vec![
        Span::styled(
            if state.suppress_future { "[x]" } else { "[ ]" },
            Style::default().fg(FG_PRIMARY),
        ),
        Span::styled(" 不再顯示此確認視窗", Style::default().fg(FG_DIM)),
    ]))
    .style(Style::default().bg(MENU_BG))
    .alignment(Alignment::Left);
    f.render_widget(checkbox, chunks[2]);

    let hint = Paragraph::new("Enter 確認 · Space 勾選 · Esc 取消")
        .style(Style::default().fg(FG_DIM).bg(MENU_BG))
        .alignment(Alignment::Left);
    f.render_widget(hint, chunks[3]);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(horizontal[1])[1]
}
