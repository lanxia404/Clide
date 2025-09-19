use crate::app::{App, Focus};
use crate::file_tree::TreeNode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use syntect::easy::HighlightLines;

// --- COLOR CONSTANTS ---
const BG_COLOR: Color = Color::Rgb(21, 21, 21);
const TEXT_COLOR: Color = Color::Rgb(220, 220, 220);
const ACCENT_COLOR: Color = Color::Rgb(0, 122, 204);
const BAR_BG_COLOR: Color = Color::Rgb(37, 37, 38);

// --- ICON LOGIC ---
// (Icon functions are unchanged and correct)
fn get_nerd_font_icon_for_filename(filename: &str, is_dir: bool, is_expanded: bool) -> &'static str {
    if is_dir { return if is_expanded { "" } else { "" }; }
    let lower_filename = filename.to_lowercase();
    if lower_filename == ".gitignore" { return ""; }
    if lower_filename == "dockerfile" { return "🐳"; }
    if lower_filename.starts_with("license") { return "📜"; }
    let extension = lower_filename.split_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "rs" => "", "py" => "", "js" | "mjs" | "cjs" => "", "ts" | "mts" | "cts" => "",
        "jsx" | "tsx" => "", "java" => "", "go" => "", "rb" => "", "php" => "",
        "swift" => "", "kt" | "kts" => "", "c" => "", "h" => "", "cpp" | "hpp" | "cc" => "",
        "cs" => "", "html" => "", "css" => "", "scss" | "sass" => "", "sh" | "bash" => "",
        "md" | "markdown" => "", "toml" => "", "yml" | "yaml" => "", "json" => "",
        "lock" => "🔒", "zip" | "tar" | "gz" => "", _ => "",
    }
}
fn get_unicode_icon_for_filename(filename: &str) -> &'static str {
    let lower_filename = filename.to_lowercase();
    if lower_filename == ".gitignore" { return "🚫"; }
    if lower_filename == "dockerfile" { return "🐳"; }
    if lower_filename.starts_with("license") { return "📜"; }
    let extension = lower_filename.split_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "rs" => "🦀", "py" => "🐍", "js" | "mjs" | "cjs" => "JS", "ts" | "mts" | "cts" => "TS",
        "jsx" | "tsx" => "⚛️", "java" => "☕", "go" => "🐹", "rb" => "💎", "php" => "🐘",
        "swift" => "🐦", "kt" | "kts" => " K", "c" | "h" => " C", "cpp" | "hpp" | "cc" => "++",
        "cs" => "C#", "html" => "🌐", "css" => "🎨", "scss" | "sass" => "🎨", "sh" | "bash" => "❯",
        "asm" | "s" => "🔧", "zig" => "⚡", "hs" | "lhs" => "λ", "ex" | "exs" => "💧",
        "dart" => "🎯", "lua" => "🌙", "pl" => "🐪", "ps1" => ">_", "vue" => " V", "svelte" => " S",
        "md" | "markdown" => "📝", "toml" => "⚙️", "yml" | "yaml" => "📋", "json" => "{}",
        "xml" => "</>", "lock" => "🔒", "sql" => "🗃️",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" => "🖼️", "svg" => "🎨",
        "ttf" | "otf" | "woff" | "woff2" => "🔤", "zip" | "tar" | "gz" | "rar" | "7z" => "📦",
        _ => "📄",
    }
}

// --- RENDER FUNCTIONS ---

pub fn render(app: &mut App, f: &mut Frame) {
    f.render_widget(Block::default().style(Style::default().bg(BG_COLOR)), f.area());
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0), Constraint::Length(1)].as_ref())
        .split(f.area());
    render_header(app, f, main_chunks[0]);
    render_footer(app, f, main_chunks[2]);
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
        .split(main_chunks[1]);
    app.file_tree_area = content_chunks[0];
    app.editor_area = content_chunks[1];
    render_file_tree(app, f, app.file_tree_area);
    render_editor(app, f, app.editor_area);
}

fn render_header(app: &App, f: &mut Frame, area: Rect) {
    let header_text = format!(
        " ☰ {}  {}  {}  {}  {}  {}  {} ",
        app.lang.header_file(), app.lang.header_edit(), app.lang.header_view(),
        app.lang.header_go(), app.lang.header_run(), app.lang.header_terminal(),
        app.lang.header_help()
    );
    let header = Paragraph::new(header_text).style(Style::default().bg(BAR_BG_COLOR).fg(TEXT_COLOR));
    f.render_widget(header, area);
}

fn render_footer(app: &App, f: &mut Frame, area: Rect) {
    let file_path = app.editor.path.as_ref()
        .map_or_else(|| app.lang.footer_no_file().to_string(), |p| p.to_string_lossy().to_string());
    let footer_text = format!(
        " {} | {} {}, {} {} | {} | {} ",
        file_path, app.lang.footer_line(), app.editor.cursor_row + 1,
        app.lang.footer_col(), app.editor.cursor_col + 1, "UTF-8",
        app.lang.footer_lang_toggle()
    );
    let footer = Paragraph::new(footer_text).style(Style::default().bg(ACCENT_COLOR).fg(Color::White));
    f.render_widget(footer, area);
}

fn render_file_tree(app: &mut App, f: &mut Frame, area: Rect) {
    let mut items = Vec::new();
    let mut total_items = 0;
    let mut selection_index = 0;

    fn traverse_tree<'a>(
        app: &App, node: &'a TreeNode, items: &mut Vec<ListItem<'a>>, depth: usize,
        current_path: &mut Vec<usize>, total_items: &mut usize, selection_index: &mut usize,
    ) {
        let file_name = node.path.file_name().unwrap_or_default().to_string_lossy();
        let icon = if file_name == ".." {
            "↩"
        } else {
            match app.icon_set {
                crate::app::IconSet::NerdFont => get_nerd_font_icon_for_filename(&file_name, node.is_directory, node.is_expanded),
                crate::app::IconSet::Unicode => {
                    if node.is_directory {
                        if node.is_expanded { "📂" } else { "📁" }
                    } else {
                        get_unicode_icon_for_filename(&file_name)
                    }
                }
            }
        };
        let line = format!("{} {} {}", "  ".repeat(depth), icon, file_name);
        if current_path == &app.file_tree.selected { *selection_index = *total_items; }
        items.push(ListItem::new(line).style(Style::default().fg(TEXT_COLOR)));
        *total_items += 1;
        if node.is_expanded {
            for (i, child) in node.children.iter().enumerate() {
                current_path.push(i);
                traverse_tree(app, child, items, depth + 1, current_path, total_items, selection_index);
                current_path.pop();
            }
        }
    }

    traverse_tree(app, &app.file_tree.root, &mut items, 0, &mut vec![], &mut total_items, &mut selection_index);

    let border_style = if app.focus == Focus::FileTree { Style::default().fg(ACCENT_COLOR) } else { Style::default().fg(TEXT_COLOR) };
    let list = List::new(items)
        .block(
            Block::default().title(app.lang.file_tree_title()).borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .style(Style::default().fg(TEXT_COLOR)).border_style(border_style)
        )
        .highlight_style(Style::default().bg(ACCENT_COLOR).add_modifier(Modifier::BOLD))
        .highlight_symbol(" > ");
    let mut state = ListState::default();
    state.select(Some(selection_index));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_editor(app: &mut App, f: &mut Frame, area: Rect) {
    let border_style = if app.focus == Focus::Editor { Style::default().fg(ACCENT_COLOR) } else { Style::default().fg(TEXT_COLOR) };
    let title = app.editor.path.as_ref()
        .map_or_else(|| app.lang.editor_title().to_string(), |p| p.file_name().unwrap_or_default().to_string_lossy().to_string());
    let editor_block = Block::default().title(format!(" {} ", title)).borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().fg(TEXT_COLOR)).border_style(border_style);
    let inner_area = editor_block.inner(area);
    f.render_widget(editor_block, area);

    let theme = &app.syntax_highlighter.theme_set.themes["base16-ocean.dark"];
    let syntax = app.editor.path.as_ref()
        .and_then(|p| p.extension().and_then(|s| s.to_str()))
        .and_then(|ext| app.syntax_highlighter.syntax_set.find_syntax_by_extension(ext))
        .unwrap_or_else(|| app.syntax_highlighter.syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut lines: Vec<Line> = Vec::new();
    let line_number_width = app.editor.content.len().to_string().len();

    for (i, line_content) in app.editor.content.iter().enumerate() {
        let line_style = if i == app.editor.cursor_row { Style::default().bg(Color::Rgb(50, 50, 50)) } else { Style::default() };
        let line_number = format!("{:>width$} │ ", i + 1, width = line_number_width);
        let mut spans = vec![Span::styled(line_number, Style::default().fg(Color::DarkGray))];
        let ranges: Vec<(syntect::highlighting::Style, &str)> = highlighter.highlight_line(line_content, &app.syntax_highlighter.syntax_set).unwrap();
        for (style, text) in ranges {
            spans.push(Span::styled(text.to_string(), Style::default().fg(Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b))));
        }
        lines.push(Line::from(spans).style(line_style));
    }
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((app.editor.vertical_scroll as u16, 0));
    
    f.render_widget(paragraph, inner_area);

    // Set the terminal cursor position
    if app.focus == Focus::Editor {
        f.set_cursor(
            inner_area.x + (app.editor.cursor_col + line_number_width + 3) as u16,
            inner_area.y + (app.editor.cursor_row - app.editor.vertical_scroll) as u16,
        );
    }
}