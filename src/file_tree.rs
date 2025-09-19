use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub is_directory: bool,
    pub children: Vec<TreeNode>,
    pub is_expanded: bool,
}

impl TreeNode {
    fn new(path: PathBuf, is_directory: bool) -> Self {
        Self {
            path,
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
        let mut root_node = TreeNode::new(root_path.to_path_buf(), true);
        root_node.children = Self::scan_directory(root_path)?;
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
        for &index in &self.selected {
            node = &mut node.children[index];
        }
        node
    }

    pub fn get_selected_path(&self) -> PathBuf {
        let mut node = &self.root;
        for &index in &self.selected {
            node = &node.children[index];
        }
        node.path.clone()
    }

    pub fn handle_key_event(&mut self, key_event: crossterm::event::KeyEvent) {
        match key_event.code {
            crossterm::event::KeyCode::Up => self.select_previous(),
            crossterm::event::KeyCode::Down => self.select_next(),
            crossterm::event::KeyCode::Left | crossterm::event::KeyCode::Right => self.toggle_expansion(),
            _ => {}
        }
    }

    pub fn toggle_expansion(&mut self) {
        let node = self.get_selected_node_mut();
        if !node.is_directory {
            return;
        }
        if node.is_expanded {
            node.is_expanded = false;
        } else {
            if node.children.is_empty() {
                if let Ok(children) = Self::scan_directory(&node.path) {
                    node.children = children;
                }
            }
            node.is_expanded = true;
        }
    }

    pub fn select_next(&mut self) {
        let mut current_node = &self.root;
        for &index in &self.selected {
            current_node = &current_node.children[index];
        }

        if current_node.is_expanded && !current_node.children.is_empty() {
            self.selected.push(0);
        } else {
            while !self.selected.is_empty() {
                let last_index = self.selected.last().unwrap().clone();
                
                let mut parent_node = &self.root;
                for &index in &self.selected[..self.selected.len() - 1] {
                    parent_node = &parent_node.children[index];
                }

                if last_index + 1 < parent_node.children.len() {
                    *self.selected.last_mut().unwrap() += 1;
                    return;
                } else {
                    self.selected.pop();
                }
            }
            // If we popped everything, reset to the first element
            if self.selected.is_empty() {
                self.selected.push(0);
            }
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(last_index) = self.selected.last_mut() {
            if *last_index > 0 {
                *last_index -= 1;
                let mut current_node = &self.root;
                for &index in &self.selected {
                    current_node = &current_node.children[index];
                }
                while current_node.is_expanded && !current_node.children.is_empty() {
                    let last_child_index = current_node.children.len() - 1;
                    self.selected.push(last_child_index);
                    current_node = &current_node.children[last_child_index];
                }
            } else {
                if self.selected.len() > 1 {
                    self.selected.pop();
                }
            }
        }
    }
}
