use crate::app::App;
use crate::file_tree::TreeNode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::Theme;
use syntect::util::LinesWithEndings;

pub fn render(app: &mut App, f: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25), // Increased width for file tree
            Constraint::Percentage(75),
        ].as_ref())
        .split(f.area());

    render_file_tree(app, f, chunks[0]);
    render_editor(app, f, chunks[1]);
}

fn render_file_tree(app: &mut App, f: &mut Frame, area: Rect) {
    // ... (render_file_tree remains the same)
    let mut items = Vec::new();
    let mut total_items = 0;
    let mut selection_index = 0;

    fn build_items<'a>(
        node: &'a TreeNode,
        items: &mut Vec<ListItem<'a>>,
        depth: usize,
        selection_path: &[usize],
        current_path: &mut Vec<usize>,
        total_items: &mut usize,
        selection_index: &mut usize,
    ) {
        let indent = "  ".repeat(depth);
        let icon = if node.is_directory { if node.is_expanded { "📂" } else { "📁" } } else { "📄" };
        let file_name = node.path.file_name().unwrap_or_default().to_string_lossy();
        let line = format!("{}{}{}", indent, icon, file_name);
        
        if current_path == selection_path {
            *selection_index = *total_items;
        }

        items.push(ListItem::new(line));
        *total_items += 1;

        if node.is_expanded {
            for (i, child) in node.children.iter().enumerate() {
                current_path.push(i);
                build_items(child, items, depth + 1, selection_path, current_path, total_items, selection_index);
                current_path.pop();
            }
        }
    }

    build_items(
        &app.file_tree.root,
        &mut items,
        0,
        &app.file_tree.selected,
        &mut vec![],
        &mut total_items,
        &mut selection_index,
    );

    let list = List::new(items)
        .block(Block::default().title(app.lang.file_tree_title()).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");

    let mut state = ListState::default();
    state.select(Some(selection_index));

    f.render_stateful_widget(list, area, &mut state);
}

fn render_editor(app: &mut App, f: &mut Frame, area: Rect) {
    let (title, syntax_def) = match &app.editor.path {
        Some(path) => {
            let path_str = path.to_string_lossy().to_string();
            let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            let syntax = app.syntax_highlighter.syntax_set
                .find_syntax_by_extension(extension)
                .unwrap_or_else(|| app.syntax_highlighter.syntax_set.find_syntax_plain_text());
            (path_str, syntax)
        }
        None => (
            app.lang.editor_title().to_string(),
            app.syntax_highlighter.syntax_set.find_syntax_plain_text(),
        ),
    };

    let theme = &app.syntax_highlighter.theme_set.themes["base16-ocean.dark"];
    let mut highlighter = HighlightLines::new(syntax_def, theme);
    let content = app.editor.content.join("\n");
    let mut lines: Vec<Line> = Vec::new();

    for line in LinesWithEndings::from(&content) {
        let ranges: Vec<(syntect::highlighting::Style, &str)> = highlighter.highlight_line(line, &app.syntax_highlighter.syntax_set).unwrap();
        let mut spans: Vec<Span> = Vec::new();
        for (style, text) in ranges {
            let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(text.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().title(title).borders(Borders::ALL));
    
    f.render_widget(paragraph, area);
}
