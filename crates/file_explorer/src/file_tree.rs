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
    pub fn icon_svg(&self) -> &'static str {
        if self.is_dir {
            "crates/app/assets/folder.svg"
        } else if self.name.ends_with(".md") {
            "crates/app/assets/markdown.svg"
        } else {
            "crates/app/assets/file.svg"
        }
    }

    pub fn expand_indicator(&self) -> &'static str {
        if self.is_dir {
            if self.expanded { "v " } else { "> " }
        } else {
            "  "
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
        .sort_by_file_name(|a, b| a.cmp(b))
        .build();

    for result in walker {
        if let Ok(entry) = result {
            let path = entry.path().to_path_buf();

            if path == root {
                continue;
            }

            let is_dir = path.is_dir();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            // Skip hidden dirs, build artifacts
            if is_dir && (name.starts_with('.') || name == "target" || name == "node_modules") {
                continue;
            }
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

/// Get git statuses for ALL files in the repo, keyed by absolute path.
/// Also propagates status to parent directories.
pub fn get_git_statuses(root: &Path) -> std::collections::HashMap<PathBuf, GitFileStatus> {
    let mut statuses = std::collections::HashMap::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return statuses,
    };

    let workdir = match repo.workdir() {
        Some(w) => w.to_path_buf(),
        None => return statuses,
    };

    let git_statuses = match repo.statuses(None) {
        Ok(s) => s,
        Err(_) => return statuses,
    };

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
            let abs_path = workdir.join(path);
            statuses.insert(abs_path.clone(), file_status.clone());

            // Propagate to all parent directories up to root
            // Priority: Deleted(3) > Modified(2) > Added/Untracked(1) > Clean(0)
            let new_priority = match &file_status {
                GitFileStatus::Deleted => 3,
                GitFileStatus::Modified => 2,
                GitFileStatus::Added | GitFileStatus::Untracked => 1,
                _ => 0,
            };

            let mut parent = abs_path.parent();
            while let Some(p) = parent {
                if p < workdir {
                    break;
                }
                let existing_priority = match statuses.get(p) {
                    Some(GitFileStatus::Deleted) => 3,
                    Some(GitFileStatus::Modified) => 2,
                    Some(GitFileStatus::Added) | Some(GitFileStatus::Untracked) => 1,
                    Some(GitFileStatus::Renamed) => 1,
                    Some(GitFileStatus::Clean) | None => 0,
                };
                if new_priority > existing_priority {
                    statuses.insert(p.to_path_buf(), file_status.clone());
                }
                parent = p.parent();
            }
        }
    }

    statuses
}
