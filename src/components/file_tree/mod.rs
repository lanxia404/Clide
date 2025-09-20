// src/components/file_tree/mod.rs

pub mod view;

// ... (The rest of the original src/file_tree.rs content)
use crossterm::event::{KeyEvent, KeyCode, MouseEvent, MouseEventKind};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub display_name: Option<String>,
    pub is_directory: bool,
    pub children: Vec<TreeNode>,
    pub is_expanded: bool,
}

impl TreeNode {
    fn new(path: PathBuf, is_directory: bool) -> Self {
        Self {
            path,
            display_name: None,
            is_directory,
            children: Vec::new(),
            is_expanded: false,
        }
    }
}

#[derive(Debug)]
pub struct FileTree {
    pub root: TreeNode,
    pub selected: Vec<usize>,
}

impl FileTree {
    pub fn new(root_path: &Path) -> Result<Self, std::io::Error> {
        // Canonicalize the path to get a clean, absolute path. This is crucial for stability.
        let absolute_path = fs::canonicalize(root_path)?;

        let mut root_node = TreeNode::new(absolute_path.clone(), true);
        root_node.children = Self::scan_directory(&absolute_path)?;
        
        // Only add ".." to the root of the tree if it's not the filesystem root
        if let Some(parent) = absolute_path.parent() {
            let mut parent_node = TreeNode::new(parent.to_path_buf(), true);
            parent_node.display_name = Some("..".to_string());
            root_node.children.insert(0, parent_node);
        }

        root_node.is_expanded = true;
        Ok(Self {
            root: root_node,
            selected: vec![0],
        })
    }

    fn scan_directory(path: &Path) -> Result<Vec<TreeNode>, std::io::Error> {
        let mut entries = fs::read_dir(path)?
            .filter_map(Result::ok)
            .map(|entry| {
                let path = entry.path();
                let is_directory = path.is_dir();
                TreeNode::new(path, is_directory)
            })
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| b.is_directory.cmp(&a.is_directory).then_with(|| a.path.cmp(&b.path)));
        Ok(entries)
    }

    pub fn get_selected_node_mut(&mut self) -> &mut TreeNode {
        let mut node = &mut self.root;
        // The selection path is relative to the visible children, so we need to be careful
        if self.selected.is_empty() {
            // This case should ideally not happen if selection is always valid
            return node;
        }
        for &index in &self.selected {
            if index >= node.children.len() {
                // Selection is out of bounds, return the current node to avoid panic
                return node;
            }
            node = &mut node.children[index];
        }
        node
    }

    pub fn get_selected_path(&self) -> PathBuf {
        let mut node = &self.root;
        if self.selected.is_empty() {
            return node.path.clone();
        }
        for &index in &self.selected {
             if index >= node.children.len() {
                return node.path.clone(); // Out of bounds
            }
            node = &node.children[index];
        }
        node.path.clone()
    }

    pub fn handle_mouse_event(&mut self, event: MouseEvent) {
        match event.kind {
            MouseEventKind::ScrollUp => self.select_previous(),
            MouseEventKind::ScrollDown => self.select_next(),
            // Double click logic will be in app.rs
            _ => {}
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Up => self.select_previous(),
            KeyCode::Down => self.select_next(),
            KeyCode::Left => self.collapse_selected(),
            KeyCode::Right => self.expand_selected(),
            _ => {}
        }
    }

    pub fn expand_selected(&mut self) {
        let node = self.get_selected_node_mut();
        if !node.is_directory { return; }
        if !node.is_expanded {
            if node.children.is_empty() && let Ok(children) = Self::scan_directory(&node.path) {
                node.children = children;
            }
            node.is_expanded = true;
        }
    }

    pub fn collapse_selected(&mut self) {
        let node = self.get_selected_node_mut();
        if node.is_directory && node.is_expanded {
            node.is_expanded = false;
        }
    }
    
    pub fn toggle_expansion(&mut self) {
        let node = self.get_selected_node_mut();
        if !node.is_directory { return; }
        if node.is_expanded {
            self.collapse_selected();
        } else {
            self.expand_selected();
        }
    }

    pub fn scroll_up(&mut self) {
        self.select_previous();
    }

    pub fn scroll_down(&mut self) {
        self.select_next();
    }

    // --- New, clearer navigation logic ---

    fn get_visible_nodes(&self) -> Vec<Vec<usize>> {
        let mut nodes = Vec::new();
        fn traverse(node: &TreeNode, path: Vec<usize>, nodes: &mut Vec<Vec<usize>>) {
            if !path.is_empty() { // Don't include the root node itself
                nodes.push(path.clone());
            }
            if node.is_expanded {
                for (i, child) in node.children.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(i);
                    traverse(child, child_path, nodes);
                }
            }
        }
        traverse(&self.root, Vec::new(), &mut nodes);
        nodes
    }

    pub fn select_next(&mut self) {
        let visible_nodes = self.get_visible_nodes();
        if let Some(current_index) = visible_nodes.iter().position(|path| path == &self.selected) {
            if current_index + 1 < visible_nodes.len() {
                self.selected = visible_nodes[current_index + 1].clone();
            }
        } else if !visible_nodes.is_empty() {
             self.selected = visible_nodes[0].clone();
        }
    }

    pub fn select_previous(&mut self) {
        let visible_nodes = self.get_visible_nodes();
        if let Some(current_index) = visible_nodes.iter().position(|path| path == &self.selected) {
            if current_index > 0 {
                self.selected = visible_nodes[current_index - 1].clone();
            }
        } else if !visible_nodes.is_empty() {
             self.selected = visible_nodes[0].clone();
        }
    }

    pub fn select_by_index(&mut self, index: usize) {
        let visible_nodes = self.get_visible_nodes();
        if index < visible_nodes.len() {
            self.selected = visible_nodes[index].clone();
        }
    }
}
