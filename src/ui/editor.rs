use crate::app::{App, Focus};
use url::Url;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use syntect::easy::HighlightLines;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Re-exporting from layout.rs to keep it DRY
use super::layout::{ACCENT_COLOR, TEXT_COLOR};

pub fn render_editor(app: &mut App, f: &mut Frame, area: Rect) {
    if app.editor.terminal_width != area.width {
        app.editor.layout_cache.clear();
        app.editor.terminal_width = area.width;
    }

    let border_style = if app.focus == Focus::Editor { Style::default().fg(ACCENT_COLOR) } else { Style::default().fg(TEXT_COLOR) };
    let title = app.editor.path.as_ref()
        .map_or_else(|| app.lang.editor_title().to_string(), |p| p.file_name().unwrap_or_default().to_string_lossy().to_string());
    let editor_block = Block::default().title(format!(" {} ", title)).borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().fg(TEXT_COLOR)).border_style(border_style);

    let inner_area = editor_block.inner(area);
    f.render_widget(editor_block, area);

    let line_number_width = app.editor.content.len().to_string().len();
    let editor_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(line_number_width as u16 + 2), // +2 for " │"
            Constraint::Min(0),
        ])
        .split(inner_area);

    let line_number_area = editor_chunks[0];
    let text_area = editor_chunks[1];
    let text_area_width = text_area.width as usize;

    if text_area_width == 0 { return; }

    let theme = &app.syntax_highlighter.theme_set.themes["base16-ocean.dark"];
    let syntax = app.editor.path.as_ref()
        .and_then(|p| p.extension().and_then(|s| s.to_str()))
        .and_then(|ext| app.syntax_highlighter.syntax_set.find_syntax_by_extension(ext))
        .unwrap_or_else(|| app.syntax_highlighter.syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, theme);

    let mut all_visual_lines: Vec<(Option<usize>, Line)> = Vec::new();
    let mut cursor_abs_visual_y: Option<usize> = None;
    let mut cursor_visual_x: Option<u16> = None;

    // --- 1. Calculate Visual Layout & Find Cursor ---
    for (line_idx, line_content) in app.editor.content.iter().enumerate() {
        let is_cursor_line = line_idx == app.editor.cursor_row;

        if !app.editor.layout_cache.contains_key(&line_idx) {
            let ranges = highlighter.highlight_line(line_content, &app.syntax_highlighter.syntax_set).unwrap();
            let spans: Vec<Span> = ranges.into_iter().map(|(style, text)| {
                Span::styled(text.to_string(), Style::default().fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b)))
            }).collect();

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
            new_cached_lines.push(Line::from(current_visual_line_spans));
            app.editor.layout_cache.insert(line_idx, new_cached_lines);
        }
        
        let cached_lines = app.editor.layout_cache.get(&line_idx).unwrap();
        for (i, line) in cached_lines.iter().enumerate() {
            let line_num = if i == 0 { Some(line_idx + 1) } else { None };
            all_visual_lines.push((line_num, line.clone()));
        }

        if is_cursor_line {
            let num_visual_lines_before = all_visual_lines.len() - cached_lines.len();
            let mut char_count = 0;
            for (i, line) in cached_lines.iter().enumerate() {
                let line_char_count: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
                if app.editor.cursor_col >= char_count && app.editor.cursor_col <= char_count + line_char_count {
                    let relative_col = app.editor.cursor_col - char_count;
                    let x_pos = line.spans.iter().flat_map(|s| s.content.chars()).take(relative_col).collect::<String>().width_cjk();
                    
                    cursor_visual_x = Some(x_pos as u16);
                    cursor_abs_visual_y = Some(num_visual_lines_before + i);
                    break;
                }
                char_count += line_char_count;
            }
        }
    }

    // --- 2. Adjust Scroll based on Cursor Position ---
    let view_height = text_area.height as usize;
    if let Some(cursor_y) = cursor_abs_visual_y {
        if cursor_y < app.editor.vertical_scroll {
            app.editor.vertical_scroll = cursor_y;
        } else if cursor_y >= app.editor.vertical_scroll + view_height {
            app.editor.vertical_scroll = cursor_y - view_height + 1;
        }
    }

    // --- 3. Prepare Lines for Rendering ---
    let mut line_number_lines = Vec::new();
    let mut text_lines = Vec::new();
    let visible_lines = all_visual_lines.into_iter().skip(app.editor.vertical_scroll).take(view_height);

    for (line_num, line) in visible_lines {
        let line_idx = line_num.map(|n| n - 1);
        let is_cursor_line = line_idx.is_some() && line_idx.unwrap() == app.editor.cursor_row;
        
        let line_number_style = if is_cursor_line { Style::default().fg(ACCENT_COLOR) } else { Style::default().fg(Color::DarkGray) };
        
        let mut gutter_symbol = " │ ";
        let mut highest_severity = None;

        if let Some(path) = &app.editor.path {
            if let Ok(uri) = Url::from_file_path(path) {
                if let Some(diagnostics) = app.diagnostics.get(&uri) {
                    if let Some(idx) = line_idx {
                        for d in diagnostics {
                            if d.range.start.line as usize == idx {
                                let severity = d.severity.unwrap_or(lsp_types::DiagnosticSeverity::HINT);
                                if highest_severity.is_none() || severity < highest_severity.unwrap() {
                                    highest_severity = Some(severity);
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(severity) = highest_severity {
            gutter_symbol = match severity {
                lsp_types::DiagnosticSeverity::ERROR => " E ",
                lsp_types::DiagnosticSeverity::WARNING => " W ",
                lsp_types::DiagnosticSeverity::INFORMATION => " I ",
                lsp_types::DiagnosticSeverity::HINT => " H ",
                _ => " ? ",
            };
        }

        if let Some(n) = line_num {
            line_number_lines.push(Line::from(Span::styled(format!("{:>width$}{}", n, gutter_symbol, width = line_number_width), line_number_style)));
        } else {
            line_number_lines.push(Line::from(Span::styled(format!("{:>width$}{}", "~", gutter_symbol, width = line_number_width), line_number_style)));
        }

        let text_style = if is_cursor_line { Style::default().bg(Color::Rgb(40, 40, 40)) } else { Style::default() };
        text_lines.push(line.style(text_style));
    }

    // --- 4. Render Paragraphs ---
    let line_number_paragraph = Paragraph::new(line_number_lines);
    let text_paragraph = Paragraph::new(text_lines);

    f.render_widget(line_number_paragraph, line_number_area);
    f.render_widget(text_paragraph, text_area);

    // --- 5. Set Cursor Position ---
    if app.focus == Focus::Editor {
        if let (Some(abs_y), Some(x)) = (cursor_abs_visual_y, cursor_visual_x) {
            let relative_y = abs_y.saturating_sub(app.editor.vertical_scroll);
            if relative_y < view_height {
                f.set_cursor(text_area.x + x, text_area.y + relative_y as u16);
            }
        }
    }
}