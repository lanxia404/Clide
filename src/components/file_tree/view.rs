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
    let mut items = Vec::new();
    let mut total_items = 0;
    let mut selection_index = 0;

    fn traverse_tree<'a>(
        app: &App, node: &'a TreeNode, items: &mut Vec<ListItem<'a>>, depth: usize,
        current_path: &mut Vec<usize>, total_items: &mut usize, selection_index: &mut usize,
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
