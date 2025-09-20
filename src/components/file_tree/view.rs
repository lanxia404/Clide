use super::TreeNode;
use crate::app::{App, Focus};
use crate::components::header::{ACCENT_COLOR, TEXT_COLOR};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub fn render_file_tree(app: &mut App, f: &mut Frame, area: Rect) {
    let (items, selection_index) = traverse_tree(app, &app.file_tree.root, &app.file_tree.selected);

    let border_style = if app.focus == Focus::FileTree {
        Style::default().fg(ACCENT_COLOR)
    } else {
        Style::default().fg(TEXT_COLOR)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .title(app.lang.file_tree_title())
                .borders(Borders::ALL)
                .border_type(ratatui::widgets::BorderType::Rounded)
                .style(Style::default().fg(TEXT_COLOR))
                .border_style(border_style),
        )
        .highlight_style(Style::default().bg(ACCENT_COLOR).add_modifier(Modifier::BOLD))
        .highlight_symbol(" > ");

    let mut state = ListState::default();
    state.select(selection_index);
    f.render_stateful_widget(list, area, &mut state);
}

/// Recursively traverses the file tree and returns a flattened list of items and the selected index.
fn traverse_tree<'a>(
    app: &'a App,
    node: &'a TreeNode,
    selected_path: &[usize],
) -> (Vec<ListItem<'a>>, Option<usize>) {
    let mut items = Vec::new();
    let mut selection_index = None;
    
    // A recursive helper function to build the list and track the selection index.
    fn build_list<'b>(
        app: &'b App,
        node: &'b TreeNode,
        depth: usize,
        current_path: &mut Vec<usize>,
        items: &mut Vec<ListItem<'b>>,
        selected_path: &[usize],
        selection_index: &mut Option<usize>,
    ) {
        let file_name_owned;
        let file_name = match &node.display_name {
            Some(name) => name.as_str(),
            None => {
                file_name_owned = node.path.file_name().unwrap_or_default().to_string_lossy().to_string();
                &file_name_owned
            }
        };

        let icon = if file_name == ".." {
            "↩"
        } else {
            match app.icon_set {
                crate::app::IconSet::NerdFont => get_nerd_font_icon_for_filename(file_name, node.is_directory, node.is_expanded),
                crate::app::IconSet::Unicode => {
                    if node.is_directory {
                        if node.is_expanded { "📂" } else { "📁" }
                    } else {
                        get_unicode_icon_for_filename(file_name)
                    }
                }
            }
        };
        let line = format!("{} {} {}", "  ".repeat(depth), icon, file_name);
        
        if current_path == selected_path {
            *selection_index = Some(items.len());
        }
        
        items.push(ListItem::new(line).style(Style::default().fg(TEXT_COLOR)));

        if node.is_expanded {
            for (i, child) in node.children.iter().enumerate() {
                current_path.push(i);
                build_list(app, child, depth + 1, current_path, items, selected_path, selection_index);
                current_path.pop();
            }
        }
    }

    build_list(app, node, 0, &mut vec![], &mut items, selected_path, &mut selection_index);
    (items, selection_index)
}

// --- ICON LOGIC ---
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
