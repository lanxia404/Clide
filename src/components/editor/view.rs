use crate::app::{App, Focus};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use syntect::easy::HighlightLines;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::components::header::{ACCENT_COLOR, TEXT_COLOR};

pub fn render_editor(app: &mut App, f: &mut Frame, area: Rect) {
    if app.editor.terminal_width != area.width {
        app.editor.layout_cache.clear();
        app.editor.terminal_width = area.width;
    }

    // Collect lines that need layout calculation to avoid borrow checker issues.
    let lines_to_layout: Vec<(usize, String)> = app.editor.content.iter().cloned().enumerate().collect();

    // Now, perform the mutable operation (caching).
    for (line_idx, line_content) in lines_to_layout {
        ensure_line_layout_cached(app, line_idx, &line_content, area.width as usize);
    }

    let border_style = if app.focus == Focus::Editor {
        Style::default().fg(ACCENT_COLOR)
    } else {
        Style::default().fg(TEXT_COLOR)
    };
    let title = app
        .editor
        .path
        .as_ref()
        .map_or_else(
            || app.lang.editor_title().to_string(),
            |p| p.file_name().unwrap_or_default().to_string_lossy().to_string(),
        );
    let editor_block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().fg(TEXT_COLOR))
        .border_style(border_style);

    let inner_area = editor_block.inner(area);
    f.render_widget(editor_block, area);

    let line_number_width = app.editor.content.len().to_string().len();
    let editor_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(line_number_width as u16 + 3), Constraint::Min(0)])
        .split(inner_area);

    let line_number_area = editor_chunks[0];
    let text_area = editor_chunks[1];
    if text_area.width == 0 {
        return;
    }

    // --- Optimized Rendering Pipeline ---
    let (line_number_lines, text_lines, cursor_visual_pos) =
        build_visible_lines(app, text_area.width as usize, text_area.height as usize);
    adjust_scroll(app, text_area.height as usize, cursor_visual_pos);
    render_paragraphs_and_cursor(
        f,
        app,
        line_number_area,
        text_area,
        line_number_lines,
        text_lines,
        cursor_visual_pos,
    );
}

/// Optimized function to build only the visible lines, calculate cursor position, and adjust scroll.
fn build_visible_lines(
    app: &App,
    _text_area_width: usize,
    view_height: usize,
) -> (Vec<Line<'static>>, Vec<Line<'static>>, Option<(usize, u16)>) {
    let mut line_number_lines = Vec::new();
    let mut text_lines = Vec::new();
    let mut cursor_visual_pos = None;
    let mut current_visual_line = 0;

    for (line_idx, _line_content) in app.editor.content.iter().enumerate() {
        let cached_layout = app.editor.layout_cache.get(&line_idx).unwrap();
        let num_visual_lines_for_this_logical_line = cached_layout.len();

        // Check if this logical line is within the viewport
        if current_visual_line + num_visual_lines_for_this_logical_line > app.editor.vertical_scroll
            && current_visual_line < app.editor.vertical_scroll + view_height
        {
            for (i, visual_line) in cached_layout.iter().enumerate() {
                let visual_line_abs_idx = current_visual_line + i;
                if visual_line_abs_idx >= app.editor.vertical_scroll
                    && visual_line_abs_idx < app.editor.vertical_scroll + view_height
                {
                    let line_num = if i == 0 { Some(line_idx + 1) } else { None };
                    let (line_num_line, text_line) =
                        prepare_line_for_rendering(app, visual_line, line_num, line_idx);
                    line_number_lines.push(line_num_line);
                    text_lines.push(text_line);
                }
            }
        }

        // Calculate cursor position if it's on this logical line
        if line_idx == app.editor.cursor_row {
            let mut char_count = 0;
            for (i, line) in cached_layout.iter().enumerate() {
                let line_char_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
                if app.editor.cursor_col >= char_count && app.editor.cursor_col <= char_count + line_char_count {
                    let relative_col = app.editor.cursor_col - char_count;
                    let x_pos = line
                        .spans
                        .iter()
                        .flat_map(|s| s.content.chars())
                        .take(relative_col)
                        .collect::<String>()
                        .width_cjk();
                    cursor_visual_pos = Some((current_visual_line + i, x_pos as u16));
                    break;
                }
                char_count += line_char_count;
            }
        }

        current_visual_line += num_visual_lines_for_this_logical_line;

        // Optimization: if we have rendered all visible lines and the cursor is not beyond this point, we can stop.
        if text_lines.len() >= view_height && (cursor_visual_pos.is_some() || app.editor.cursor_row < line_idx) {
            break;
        }
    }

    (line_number_lines, text_lines, cursor_visual_pos)
}

fn prepare_line_for_rendering<'a>(
    app: &App,
    visual_line: &Line<'a>,
    line_num: Option<usize>,
    line_idx: usize,
) -> (Line<'static>, Line<'static>) {
    let line_number_width = app.editor.content.len().to_string().len();
    let is_cursor_line = line_idx == app.editor.cursor_row;
    let line_number_style = if is_cursor_line {
        Style::default().fg(ACCENT_COLOR)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let gutter_symbol = get_gutter_symbol(app, line_num.map(|_| line_idx));
    let line_num_display = line_num.map_or("~".to_string(), |n| n.to_string());

    let line_number_line = Line::from(Span::styled(
        format!("{:>width$}{}", line_num_display, gutter_symbol, width = line_number_width),
        line_number_style,
    ));

    let text_style = if is_cursor_line {
        Style::default().bg(Color::Rgb(40, 40, 40))
    } else {
        Style::default()
    };
    let owned_spans: Vec<Span<'static>> = visual_line.spans.iter().map(|s| {
        Span::styled(s.content.to_string(), s.style)
    }).collect();
    let text_line = Line::from(owned_spans).style(text_style);

    (line_number_line, text_line)
}

fn adjust_scroll(app: &mut App, view_height: usize, cursor_visual_pos: Option<(usize, u16)>) {
    if let Some((cursor_y, _)) = cursor_visual_pos {
        if cursor_y < app.editor.vertical_scroll {
            app.editor.vertical_scroll = cursor_y;
        } else if cursor_y >= app.editor.vertical_scroll + view_height {
            app.editor.vertical_scroll = cursor_y - view_height + 1;
        }
    }
}

fn render_paragraphs_and_cursor(
    f: &mut Frame,
    app: &App,
    line_number_area: Rect,
    text_area: Rect,
    line_number_lines: Vec<Line>,
    text_lines: Vec<Line>,
    cursor_visual_pos: Option<(usize, u16)>,
) {
    f.render_widget(Paragraph::new(line_number_lines), line_number_area);
    f.render_widget(Paragraph::new(text_lines), text_area);

    if app.focus == Focus::Editor {
        if let Some((abs_y, x)) = cursor_visual_pos {
            let relative_y = abs_y.saturating_sub(app.editor.vertical_scroll);
            if relative_y < text_area.height as usize {
                f.set_cursor_position((text_area.x + x, text_area.y + relative_y as u16));
            }
        }
    }
}

fn get_gutter_symbol(app: &App, line_idx: Option<usize>) -> &'static str {
    if let Some(idx) = line_idx {
        if let Some(path) = &app.editor.path {
            if let Ok(path_str) = path.to_str().ok_or(()) {
                if let Ok(uri) = format!("file://{}", path_str).parse::<lsp_types::Uri>() {
                    if let Some(diagnostics) = app.diagnostics.get(&uri) {
                        let mut highest_severity = None;
                        for d in diagnostics {
                            if d.range.start.line as usize == idx {
                                let severity = d.severity.unwrap_or(lsp_types::DiagnosticSeverity::HINT);
                                if highest_severity.is_none() || severity < highest_severity.unwrap() {
                                    highest_severity = Some(severity);
                                }
                            }
                        }
                        if let Some(severity) = highest_severity {
                            return match severity {
                                lsp_types::DiagnosticSeverity::ERROR => " E ",
                                lsp_types::DiagnosticSeverity::WARNING => " W ",
                                lsp_types::DiagnosticSeverity::INFORMATION => " I ",
                                lsp_types::DiagnosticSeverity::HINT => " H ",
                                _ => " ? ",
                            };
                        }
                    }
                }
            }
        }
    }
    " │ "
}

fn ensure_line_layout_cached(app: &mut App, line_idx: usize, line_content: &str, text_area_width: usize) {
    if app.editor.layout_cache.contains_key(&line_idx) {
        return;
    }

    let theme = &app.syntax_highlighter.theme_set.themes["base16-ocean.dark"];
    let syntax = app
        .editor
        .path
        .as_ref()
        .and_then(|p| p.extension().and_then(|s| s.to_str()))
        .and_then(|ext| app.syntax_highlighter.syntax_set.find_syntax_by_extension(ext))
        .unwrap_or_else(|| app.syntax_highlighter.syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, theme);

    let ranges = highlighter.highlight_line(line_content, &app.syntax_highlighter.syntax_set).unwrap();
    let spans: Vec<Span> = ranges
        .into_iter()
        .map(|(style, text)| {
            Span::styled(
                text.to_string(),
                Style::default().fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)),
            )
        })
        .collect();

    let mut current_visual_line_spans = Vec::new();
    let mut current_visual_line_width = 0;
    let mut new_cached_lines = Vec::new();

    for span in spans {
        let mut span_content = span.content.as_ref();
        let style = span.style;

        while !span_content.is_empty() {
            let mut split_width = 0;
            let mut split_idx = span_content.len();

            for (char_idx, c) in span_content.char_indices() {
                let char_width = c.width_cjk().unwrap_or(0);
                if current_visual_line_width + split_width + char_width > text_area_width {
                    split_idx = char_idx;
                    break;
                }
                split_width += char_width;
            }

            let chunk = &span_content[..split_idx];
            current_visual_line_spans.push(Span::styled(chunk.to_string(), style));
            current_visual_line_width += split_width;
            span_content = &span_content[split_idx..];

            if !span_content.is_empty() || current_visual_line_width >= text_area_width {
                new_cached_lines.push(Line::from(current_visual_line_spans.clone()));
                current_visual_line_spans.clear();
                current_visual_line_width = 0;
            }
        }
    }
    if !current_visual_line_spans.is_empty() {
        new_cached_lines.push(Line::from(current_visual_line_spans));
    }

    if new_cached_lines.is_empty() {
        new_cached_lines.push(Line::from(vec![]));
    }

    app.editor.layout_cache.insert(line_idx, new_cached_lines.into_iter().map(|line| line.to_owned()).collect());
}