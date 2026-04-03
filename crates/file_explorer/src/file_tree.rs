use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub enum GitFileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Clean,
}

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    pub git_status: GitFileStatus,
    pub children_loaded: bool,
}

impl FileEntry {
    pub fn icon(&self) -> &'static str {
        if self.is_dir {
            if self.expanded { "v " } else { "> " }
        } else {
            match self.path.extension().and_then(|e| e.to_str()) {
                Some("rs") => "# " ,
                Some("toml") => "@ ",
                Some("md") => "M ",
                Some("json") => "{ ",
                Some("yaml" | "yml") => "% ",
                Some("ts" | "tsx") => "T ",
                Some("js" | "jsx") => "J ",
                Some("py") => "P ",
                Some("lock") => "L ",
                _ => "- ",
            }
        }
    }
}

/// Build a file tree from a directory path
pub fn build_file_tree(root: &Path, depth: usize, max_depth: usize) -> Vec<FileEntry> {
    if depth > max_depth {
        return Vec::new();
    }

    let mut entries = Vec::new();

    let walker = ignore::WalkBuilder::new(root)
        .max_depth(Some(1))
        .hidden(false)
        .git_ignore(true)
        .sort_by_file_name(|a, b| {
            // Directories first, then alphabetical
            let a_is_dir = a.to_str().map_or(false, |_| true);
            let b_is_dir = b.to_str().map_or(false, |_| true);
            match (a_is_dir, b_is_dir) {
                _ => a.cmp(b),
            }
        })
        .build();

    for result in walker {
        if let Ok(entry) = result {
            let path = entry.path().to_path_buf();

            // Skip the root itself
            if path == root {
                continue;
            }

            let is_dir = path.is_dir();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip hidden dirs, build artifacts, and common noise
            if is_dir && (name.starts_with('.') || name == "target" || name == "node_modules") {
                continue;
            }
            // Skip hidden files
            if !is_dir && name.starts_with('.') {
                continue;
            }

            entries.push(FileEntry {
                name,
                path,
                is_dir,
                depth,
                expanded: false,
                git_status: GitFileStatus::Clean,
                children_loaded: false,
            });
        }
    }

    // Sort: directories first, then alphabetical
    entries.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    entries
}

/// Get git statuses for files in a directory
pub fn get_git_statuses(root: &Path) -> std::collections::HashMap<PathBuf, GitFileStatus> {
    let mut statuses = std::collections::HashMap::new();

    if let Ok(repo) = git2::Repository::discover(root) {
        if let Ok(git_statuses) = repo.statuses(None) {
            for entry in git_statuses.iter() {
                let status = entry.status();
                let file_status = if status.is_wt_modified() || status.is_index_modified() {
                    GitFileStatus::Modified
                } else if status.is_wt_new() {
                    GitFileStatus::Untracked
                } else if status.is_index_new() {
                    GitFileStatus::Added
                } else if status.is_wt_deleted() || status.is_index_deleted() {
                    GitFileStatus::Deleted
                } else if status.is_wt_renamed() || status.is_index_renamed() {
                    GitFileStatus::Renamed
                } else {
                    continue;
                };

                if let Some(path) = entry.path() {
                    statuses.insert(root.join(path), file_status);
                }
            }
        }
    }

    statuses
}
