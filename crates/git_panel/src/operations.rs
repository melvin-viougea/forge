use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Stage all modified files
pub fn stage_all(root: &Path) -> Result<()> {
    let repo = git2::Repository::discover(root).context("No git repository found")?;
    let mut index = repo.index().context("Failed to get index")?;
    index
        .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("Failed to stage files")?;
    index.write().context("Failed to write index")?;
    Ok(())
}

/// Stage a specific file
pub fn stage_file(root: &Path, file_path: &Path) -> Result<()> {
    let repo = git2::Repository::discover(root).context("No git repository found")?;
    let mut index = repo.index().context("Failed to get index")?;
    let rel_path = file_path.strip_prefix(root).unwrap_or(file_path);
    index
        .add_path(rel_path)
        .context("Failed to stage file")?;
    index.write().context("Failed to write index")?;
    Ok(())
}

/// Commit with a message
pub fn commit(root: &Path, message: &str) -> Result<String> {
    let repo = git2::Repository::discover(root).context("No git repository found")?;
    let mut index = repo.index().context("Failed to get index")?;
    let tree_oid = index.write_tree().context("Failed to write tree")?;
    let tree = repo.find_tree(tree_oid).context("Failed to find tree")?;

    let sig = repo
        .signature()
        .context("Failed to get git signature. Configure user.name and user.email.")?;

    let head = repo.head().context("Failed to get HEAD")?;
    let parent_commit = repo
        .find_commit(head.target().context("HEAD has no target")?)
        .context("Failed to find parent commit")?;

    let oid = repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent_commit])
        .context("Failed to create commit")?;

    Ok(oid.to_string())
}

/// Push to remote origin
pub fn push(root: &Path) -> Result<()> {
    // Use git CLI for push (handles SSH auth via ssh-agent)
    let output = Command::new("git")
        .arg("push")
        .current_dir(root)
        .output()
        .context("Failed to execute git push")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git push failed: {}", stderr);
    }

    Ok(())
}

/// Generate a commit message using Claude Code CLI
pub fn generate_commit_message(root: &Path) -> Result<String> {
    // Get the staged diff
    let diff_output = Command::new("git")
        .args(["diff", "--cached", "--stat"])
        .current_dir(root)
        .output()
        .context("Failed to get staged diff")?;

    let diff_detail = Command::new("git")
        .args(["diff", "--cached"])
        .current_dir(root)
        .output()
        .context("Failed to get staged diff detail")?;

    let diff_stat = String::from_utf8_lossy(&diff_output.stdout);
    let diff_content = String::from_utf8_lossy(&diff_detail.stdout);

    if diff_stat.trim().is_empty() {
        anyhow::bail!("No staged changes to commit");
    }

    // Get recent commit messages for style matching
    let log_output = Command::new("git")
        .args(["log", "--oneline", "-10"])
        .current_dir(root)
        .output()
        .context("Failed to get git log")?;

    let recent_commits = String::from_utf8_lossy(&log_output.stdout);

    let prompt = format!(
        "Generate a concise git commit message (max 72 chars for title, optional body). \
         Match the style of recent commits.\n\n\
         Recent commits:\n{}\n\n\
         Changes:\n{}\n\n\
         Diff:\n{}",
        recent_commits,
        diff_stat,
        // Limit diff to prevent token overflow
        if diff_content.len() > 4000 {
            &diff_content[..4000]
        } else {
            &diff_content
        }
    );

    let output = Command::new("claude")
        .args(["-p", &prompt, "--output-format", "text", "--bare"])
        .current_dir(root)
        .output()
        .context("Failed to run Claude Code CLI. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude Code failed: {}", stderr);
    }

    let message = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(message)
}

/// One-button: stage all + generate message + commit + push
pub fn one_button_commit_and_push(root: &Path) -> Result<String> {
    stage_all(root)?;
    let message = generate_commit_message(root)?;
    let oid = commit(root, &message)?;
    push(root)?;
    Ok(format!("Committed {} and pushed: {}", &oid[..8], message))
}
