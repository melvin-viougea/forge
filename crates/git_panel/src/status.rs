use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GitFileChange {
    pub path: PathBuf,
    pub status: ChangeStatus,
    pub staged: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChangeStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
}

impl ChangeStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ChangeStatus::Modified => "M",
            ChangeStatus::Added => "A",
            ChangeStatus::Deleted => "D",
            ChangeStatus::Renamed => "R",
            ChangeStatus::Untracked => "?",
        }
    }
}

/// Get all file changes in the repository
pub fn get_changes(root: &Path) -> Vec<GitFileChange> {
    let mut changes = Vec::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return changes,
    };

    let statuses = match repo.statuses(None) {
        Ok(s) => s,
        Err(_) => return changes,
    };

    for entry in statuses.iter() {
        let s = entry.status();
        let path = match entry.path() {
            Some(p) => PathBuf::from(p),
            None => continue,
        };

        // Index (staged) changes
        if s.is_index_modified() {
            changes.push(GitFileChange {
                path: path.clone(),
                status: ChangeStatus::Modified,
                staged: true,
            });
        } else if s.is_index_new() {
            changes.push(GitFileChange {
                path: path.clone(),
                status: ChangeStatus::Added,
                staged: true,
            });
        } else if s.is_index_deleted() {
            changes.push(GitFileChange {
                path: path.clone(),
                status: ChangeStatus::Deleted,
                staged: true,
            });
        }

        // Working tree (unstaged) changes
        if s.is_wt_modified() {
            changes.push(GitFileChange {
                path: path.clone(),
                status: ChangeStatus::Modified,
                staged: false,
            });
        } else if s.is_wt_new() {
            changes.push(GitFileChange {
                path: path.clone(),
                status: ChangeStatus::Untracked,
                staged: false,
            });
        } else if s.is_wt_deleted() {
            changes.push(GitFileChange {
                path,
                status: ChangeStatus::Deleted,
                staged: false,
            });
        }
    }

    changes
}

/// Get the current branch name
pub fn get_branch_name(root: &Path) -> String {
    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return "no repo".to_string(),
    };

    let branch_name = match repo.head() {
        Ok(head) => head
            .shorthand()
            .unwrap_or("detached")
            .to_string(),
        Err(_) => "no branch".to_string(),
    };
    branch_name
}
