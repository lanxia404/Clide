// src/components/file_tree/mod.rs

pub mod view;

// ... (The rest of the original src/file_tree.rs content)
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::Result;
use tokio::fs as async_fs;

#[derive(Debug, Clone)]
pub struct SelectedNodeInfo {
    pub path: PathBuf,
    pub is_directory: bool,
    pub is_parent_directory: bool,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub display_name: Option<String>,
    pub is_directory: bool,
    pub is_parent_directory: bool,
    pub children: Vec<TreeNode>,
    pub is_expanded: bool,
}

impl TreeNode {
    fn new(path: PathBuf, is_directory: bool) -> Self {
        Self {
            path,
            display_name: None,
            is_directory,
            is_parent_directory: false,
            children: Vec::new(),
            is_expanded: false,
        }
    }
}

#[derive(Debug)]
pub struct FileTree {
    pub root: TreeNode,
    pub selected: Vec<usize>,
    visible_nodes_cache: Vec<Vec<usize>>,
    is_dirty: bool,
}

impl FileTree {
    pub fn new(root_path: &Path) -> Result<Self, std::io::Error> {
        // Canonicalize the path to get a clean, absolute path. This is crucial for stability.
        let absolute_path = fs::canonicalize(root_path)?;

        let mut root_node = TreeNode::new(absolute_path.clone(), true);
        root_node.children = Self::scan_directory_sync(&absolute_path)?;
        
        if let Some(parent) = absolute_path.parent() {
            let mut parent_node = TreeNode::new(parent.to_path_buf(), true);
            parent_node.display_name = Some("..".to_string());
            parent_node.is_parent_directory = true;
            root_node.children.insert(0, parent_node);
        }

        root_node.is_expanded = true;
        Ok(Self {
            root: root_node,
            selected: vec![0],
            visible_nodes_cache: Vec::new(),
            is_dirty: true,
        })
    }

    // The new async version for non-blocking IO
    pub async fn new_async(root_path: &Path) -> Result<Self> {
        let absolute_path = async_fs::canonicalize(root_path).await?;
        let mut root_node = TreeNode::new(absolute_path.clone(), true);
        root_node.children = scan_directory_async(&absolute_path).await?;

        if let Some(parent) = absolute_path.parent() {
            let mut parent_node = TreeNode::new(parent.to_path_buf(), true);
            parent_node.display_name = Some("..".to_string());
            parent_node.is_parent_directory = true;
            root_node.children.insert(0, parent_node);
        }

        root_node.is_expanded = true;
        Ok(Self {
            root: root_node,
            selected: vec![0],
            visible_nodes_cache: Vec::new(),
            is_dirty: true,
        })
    }

    // Renamed original scan_directory
    fn scan_directory_sync(path: &Path) -> Result<Vec<TreeNode>, std::io::Error> {
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

    pub fn get_selected_node(&self) -> &TreeNode {
        let mut node = &self.root;
        if self.selected.is_empty() {
            return node;
        }
        for &index in &self.selected {
            if index >= node.children.len() {
                return node;
            }
            node = &node.children[index];
        }
        node
    }

    pub fn get_selected_node_info(&self) -> SelectedNodeInfo {
        let node = self.get_selected_node();
        SelectedNodeInfo {
            path: node.path.clone(),
            is_directory: node.is_directory,
            is_parent_directory: node.is_parent_directory,
            display_name: node.display_name.clone(),
        }
    }

    pub fn mark_dirty(&mut self) {
        self.is_dirty = true;
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

    pub fn expand_selected(&mut self) {
        let node = self.get_selected_node_mut();
        if !node.is_directory { return; }
        if !node.is_expanded {
            // This is now handled asynchronously in app.rs
            // if node.children.is_empty() && let Ok(children) = Self::scan_directory_sync(&node.path) {
            //     node.children = children;
            // }
            node.is_expanded = true;
            self.is_dirty = true;
        }
    }

    pub fn collapse_selected(&mut self) {
        let node = self.get_selected_node_mut();
        if node.is_directory && node.is_expanded {
            node.is_expanded = false;
            self.is_dirty = true;
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

    // --- New, clearer navigation logic ---

    fn rebuild_cache(&mut self) {
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
        self.visible_nodes_cache = nodes;
        self.is_dirty = false;
    }

    fn ensure_cache_is_updated(&mut self) {
        if self.is_dirty {
            self.rebuild_cache();
        }
    }

    pub fn select_next(&mut self) {
        self.ensure_cache_is_updated();
        if let Some(current_index) = self.visible_nodes_cache.iter().position(|path| path == &self.selected) {
            if current_index + 1 < self.visible_nodes_cache.len() {
                self.selected = self.visible_nodes_cache[current_index + 1].clone();
            }
        } else if !self.visible_nodes_cache.is_empty() {
             self.selected = self.visible_nodes_cache[0].clone();
        }
    }

    pub fn select_previous(&mut self) {
        self.ensure_cache_is_updated();
        if let Some(current_index) = self.visible_nodes_cache.iter().position(|path| path == &self.selected) {
            if current_index > 0 {
                self.selected = self.visible_nodes_cache[current_index - 1].clone();
            }
        } else if !self.visible_nodes_cache.is_empty() {
             self.selected = self.visible_nodes_cache[0].clone();
        }
    }

    pub fn select_by_index(&mut self, index: usize) {
        self.ensure_cache_is_updated();
        if index < self.visible_nodes_cache.len() {
            self.selected = self.visible_nodes_cache[index].clone();
        }
    }

    /// Selects a given path in the file tree, expanding parent directories as needed.
    pub fn select_path(&mut self, path_to_select: &Path) {
        // Helper function to recursively find the path.
        fn find_path_recursive(
            current_node: &mut TreeNode,
            path_to_select: &Path,
            current_selection_path: &mut Vec<usize>,
        ) -> Option<Vec<usize>> {
            // Check if the current node's path is a parent of or equal to the target path.
            if path_to_select.starts_with(&current_node.path) {
                // If it's a direct match, we found it.
                if current_node.path == path_to_select {
                    return Some(current_selection_path.clone());
                }

                // If it's a directory and a potential ancestor, expand and search its children.
                if current_node.is_directory {
                    if !current_node.is_expanded {
                        // Synchronous expansion during selection is a trade-off.
                        // For a fully async solution, this would need to become async and spawn tasks.
                        if let Ok(children) = FileTree::scan_directory_sync(&current_node.path) {
                            current_node.children = children;
                        }
                        current_node.is_expanded = true;
                    }

                    for (i, child) in current_node.children.iter_mut().enumerate() {
                        current_selection_path.push(i);
                        if let Some(found_path) = find_path_recursive(child, path_to_select, current_selection_path) {
                            return Some(found_path);
                        }
                        current_selection_path.pop();
                    }
                }
            }
            None
        }

        if let Some(new_selection) = find_path_recursive(&mut self.root, path_to_select, &mut vec![]) {
            self.selected = new_selection;
        }
    }
}

pub async fn scan_directory_async(path: &Path) -> Result<Vec<TreeNode>> {
    let mut entries = vec![];
    let mut read_dir = async_fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;
        let is_directory = metadata.is_dir();
        entries.push(TreeNode::new(path, is_directory));
    }
    entries.sort_by(|a, b| b.is_directory.cmp(&a.is_directory).then_with(|| a.path.cmp(&b.path)));
    Ok(entries)
}
