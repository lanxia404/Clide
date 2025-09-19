use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileEntryKind {
    Directory,
    File,
    WorkspaceRoot,
    ParentLink,
}

#[derive(Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub depth: usize,
    pub kind: FileEntryKind,
    pub expanded: bool,
    pub has_children: bool,
}

pub struct FileTree {
    entries: Vec<FileEntry>,
    selected: usize,
    current_dir: PathBuf,
    expanded_dirs: HashSet<PathBuf>,
    show_hidden: bool,
}

impl FileTree {
    pub fn from_root(root: PathBuf) -> Self {
        let mut tree = Self {
            entries: Vec::new(),
            selected: 0,
            current_dir: root,
            expanded_dirs: HashSet::new(),
            show_hidden: false,
        };
        tree.refresh();
        tree
    }

    fn canonicalize(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn ensure_expanded_root(&mut self) {
        let root = self.canonicalize(&self.current_dir);
        self.expanded_dirs.insert(root);
    }

    fn append_directory_entries(
        &mut self,
        dir: &Path,
        depth: usize,
        entries: &mut Vec<FileEntry>,
        has_content: &mut bool,
    ) {
        let children = match Self::read_and_sort_directory(dir, self.show_hidden) {
            Ok(children) => children,
            Err(_) => return,
        };

        for child in children {
            let name = match child.file_name().to_str() {
                Some(name) => name.to_string(),
                None => continue,
            };

            let path = self.canonicalize(&child.path());
            let is_dir = child.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let display_depth = depth + 1;
            if is_dir {
                let expanded = self.expanded_dirs.contains(&path);
                entries.push(FileEntry {
                    name,
                    path: path.clone(),
                    depth: display_depth,
                    kind: FileEntryKind::Directory,
                    expanded,
                    has_children: true, // Always expandable
                });
                *has_content = true;
                if expanded {
                    self.append_directory_entries(&path, display_depth, entries, has_content);
                }
            } else {
                entries.push(FileEntry {
                    name,
                    path: path.clone(),
                    depth: display_depth,
                    kind: FileEntryKind::File,
                    expanded: false,
                    has_children: false,
                });
                *has_content = true;
            }
        }
    }

    /// Reads, filters, and sorts the entries of a directory.
    fn read_and_sort_directory(
        dir: &Path,
        show_hidden: bool,
    ) -> std::io::Result<Vec<fs::DirEntry>> {
        let mut children: Vec<fs::DirEntry> = fs::read_dir(dir)?
            .filter_map(|res| res.ok())
            .filter(|entry| {
                if show_hidden {
                    return true;
                }
                if let Some(name) = entry.file_name().to_str() {
                    !name.starts_with('.')
                } else {
                    false
                }
            })
            .collect();

        children.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
            match b_is_dir.cmp(&a_is_dir) {
                std::cmp::Ordering::Equal => a.file_name().cmp(&b.file_name()),
                other => other,
            }
        });

        Ok(children)
    }

    pub fn refresh(&mut self) {
        let previously_selected = self
            .entries
            .get(self.selected)
            .map(|entry| entry.path.clone());

        let current_canon = self.canonicalize(&self.current_dir);
        self.current_dir = current_canon.clone();
        let root_has_children = current_canon.is_dir();

        self.ensure_expanded_root();

        let mut entries = Vec::new();
        entries.push(FileEntry {
            name: String::from("./"),
            path: current_canon.clone(),
            depth: 0,
            kind: FileEntryKind::WorkspaceRoot,
            expanded: true,
            has_children: root_has_children,
        });

        let parent_target = self
            .current_dir
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| self.current_dir.clone());
        entries.push(FileEntry {
            name: String::from("../"),
            path: parent_target,
            depth: 0,
            kind: FileEntryKind::ParentLink,
            expanded: false,
            has_children: true,
        });

        let mut has_content = false;
        let dir_to_scan = self.current_dir.clone();
        self.append_directory_entries(&dir_to_scan, 0, &mut entries, &mut has_content);

        if !has_content {
            entries.push(FileEntry {
                name: String::from("(空目錄)"),
                path: self.current_dir.clone(),
                depth: 1,
                kind: FileEntryKind::File,
                expanded: false,
                has_children: false,
            });
        }

        if let Some(path) = previously_selected
            && let Some(idx) = entries.iter().position(|entry| entry.path == path) {
                self.selected = idx;
            }

        self.entries = entries;
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
    }

    pub fn populate_with_placeholder(&mut self) {
        self.expanded_dirs.clear();
        let canon = self.canonicalize(&self.current_dir);
        self.expanded_dirs.insert(canon.clone());
        self.entries = vec![
            FileEntry {
                name: String::from("./"),
                path: canon.clone(),
                depth: 0,
                kind: FileEntryKind::WorkspaceRoot,
                expanded: true,
                has_children: true,
            },
            FileEntry {
                name: String::from("../"),
                path: canon
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or(canon.clone()),
                depth: 0,
                kind: FileEntryKind::ParentLink,
                expanded: false,
                has_children: true,
            },
            FileEntry {
                name: String::from("src/main.rs"),
                path: canon.join("src").join("main.rs"),
                depth: 1,
                kind: FileEntryKind::File,
                expanded: false,
                has_children: false,
            },
        ];
        self.selected = 0;
    }

    pub fn entries(&self) -> &[FileEntry] {
        &self.entries
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        let mut new_index = self.selected as isize + delta;
        if new_index < 0 {
            new_index = 0;
        }
        if new_index >= len {
            new_index = len - 1;
        }
        self.selected = new_index as usize;
    }

    pub fn set_selection(&mut self, index: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = index.min(self.entries.len().saturating_sub(1));
    }

    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.entries.get(self.selected)
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_dir(&self) -> &PathBuf {
        &self.current_dir
    }

    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    pub fn toggle_show_hidden(&mut self) -> bool {
        self.show_hidden = !self.show_hidden;
        self.refresh();
        self.show_hidden
    }

    pub fn activate_selected(&mut self) -> FileTreeAction {
        if let Some(entry) = self.selected_entry().cloned() {
            match entry.kind {
                FileEntryKind::File => {
                    return FileTreeAction::OpenFile(entry.path);
                }
                FileEntryKind::Directory | FileEntryKind::ParentLink => {
                    if self.navigate_to(&entry.path) {
                        return FileTreeAction::ChangedDir(entry.path);
                    }
                }
                FileEntryKind::WorkspaceRoot => {
                    // no-op for current directory entry
                }
            }
        }
        FileTreeAction::None
    }

    fn navigate_to(&mut self, path: &Path) -> bool {
        let target = self.canonicalize(path);
        if target == self.current_dir {
            return false;
        }
        if !target.is_dir() {
            return false;
        }
        self.current_dir = target.clone();
        self.expanded_dirs.insert(target);
        self.selected = 0;
        self.refresh();
        true
    }
}

pub enum FileTreeAction {
    OpenFile(PathBuf),
    ChangedDir(PathBuf),
    None,
}
