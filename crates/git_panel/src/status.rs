use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GitFileChange {
    pub path: PathBuf,
    pub status: ChangeStatus,
    pub staged: bool,
    pub insertions: usize,
    pub deletions: usize,
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

/// Get all file changes with diff stats
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

    // Collect diff stats per file
    let mut file_stats: std::collections::HashMap<String, (usize, usize)> =
        std::collections::HashMap::new();

    // Unstaged diff (workdir vs index)
    if let Ok(diff) = repo.diff_index_to_workdir(None, None) {
        collect_diff_stats(&diff, &mut file_stats);
    }

    // Staged diff (index vs HEAD)
    if let Ok(head) = repo.head() {
        if let Ok(tree) = head.peel_to_tree() {
            if let Ok(diff) = repo.diff_tree_to_index(Some(&tree), None, None) {
                collect_diff_stats(&diff, &mut file_stats);
            }
        }
    }

    for entry in statuses.iter() {
        let s = entry.status();
        let path_str = match entry.path() {
            Some(p) => p.to_string(),
            None => continue,
        };
        let path = PathBuf::from(&path_str);
        let (ins, del) = file_stats.get(&path_str).copied().unwrap_or((0, 0));

        // Index (staged) changes
        if s.is_index_modified() || s.is_index_new() || s.is_index_deleted() {
            let status = if s.is_index_modified() {
                ChangeStatus::Modified
            } else if s.is_index_new() {
                ChangeStatus::Added
            } else {
                ChangeStatus::Deleted
            };
            changes.push(GitFileChange {
                path: path.clone(),
                status,
                staged: true,
                insertions: ins,
                deletions: del,
            });
        }

        // Working tree (unstaged) changes
        if s.is_wt_modified() || s.is_wt_new() || s.is_wt_deleted() {
            let status = if s.is_wt_modified() {
                ChangeStatus::Modified
            } else if s.is_wt_new() {
                ChangeStatus::Untracked
            } else {
                ChangeStatus::Deleted
            };
            // Don't duplicate if already added as staged
            if !(s.is_index_modified() || s.is_index_new() || s.is_index_deleted()) {
                changes.push(GitFileChange {
                    path,
                    status,
                    staged: false,
                    insertions: ins,
                    deletions: del,
                });
            }
        }
    }

    changes
}

fn collect_diff_stats(
    diff: &git2::Diff,
    stats: &mut std::collections::HashMap<String, (usize, usize)>,
) {
    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let entry = stats.entry(path).or_insert((0, 0));
        match line.origin() {
            '+' => entry.0 += 1,
            '-' => entry.1 += 1,
            _ => {}
        }
        true
    })
    .ok();
}

// ── Git log ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct GitCommit {
    pub hash: String,   // short 7-char hash
    pub message: String, // first line
    pub author: String,
    pub time_ago: String, // e.g. "2h ago", "3d ago"
}

/// Get recent commits from the repo.
pub fn get_commits(root: &Path, limit: usize) -> Vec<GitCommit> {
    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };

    let mut revwalk = match repo.revwalk() {
        Ok(rw) => rw,
        Err(_) => return Vec::new(),
    };
    revwalk.push_head().ok();
    revwalk.set_sorting(git2::Sort::TIME).ok();

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let mut commits = Vec::new();
    for oid in revwalk.take(limit).flatten() {
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let hash = format!("{}", &oid.to_string()[..7]);
        let message = commit
            .summary()
            .unwrap_or("")
            .to_string();
        let author = commit
            .author()
            .name()
            .unwrap_or("unknown")
            .to_string();
        let epoch = commit.time().seconds();
        let time_ago = format_time_ago(now - epoch);

        commits.push(GitCommit { hash, message, author, time_ago });
    }
    commits
}

fn format_time_ago(secs: i64) -> String {
    if secs < 60 { return "now".to_string(); }
    let mins = secs / 60;
    if mins < 60 { return format!("{}m", mins); }
    let hours = mins / 60;
    if hours < 24 { return format!("{}h", hours); }
    let days = hours / 24;
    if days < 30 { return format!("{}d", days); }
    let months = days / 30;
    if months < 12 { return format!("{}mo", months); }
    format!("{}y", days / 365)
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
