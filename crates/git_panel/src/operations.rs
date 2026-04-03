use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Stage all files (including untracked) via git CLI
pub fn stage_all(root: &Path) -> Result<()> {
    let output = Command::new("git")
        .args(["add", "-A"])
        .current_dir(root)
        .output()
        .context("Failed to execute git add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git add failed: {}", stderr);
    }
    Ok(())
}

/// Commit with a message via git CLI
pub fn commit(root: &Path, message: &str) -> Result<String> {
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(root)
        .output()
        .context("Failed to execute git commit")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git commit failed: {}", stderr);
    }

    // Get the commit hash
    let hash_output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .context("Failed to get commit hash")?;

    Ok(String::from_utf8_lossy(&hash_output.stdout).trim().to_string())
}

/// Push to remote origin
pub fn push(root: &Path) -> Result<()> {
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

    let log_output = Command::new("git")
        .args(["log", "--oneline", "-10"])
        .current_dir(root)
        .output()
        .context("Failed to get git log")?;

    let recent_commits = String::from_utf8_lossy(&log_output.stdout);

    let prompt = format!(
        "Generate a concise git commit message (just the message, no quotes, no explanation). \
         Max 72 chars. Match the style of recent commits.\n\n\
         Recent commits:\n{}\n\n\
         Changes:\n{}\n\n\
         Diff:\n{}",
        recent_commits,
        diff_stat,
        if diff_content.len() > 4000 {
            &diff_content[..4000]
        } else {
            &diff_content
        }
    );

    let output = Command::new("claude")
        .args(["-p", &prompt, "--output-format", "text"])
        .current_dir(root)
        .output()
        .context("Failed to run Claude Code CLI. Is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Claude Code failed: {}", stderr);
    }

    let message = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if message.is_empty() {
        anyhow::bail!("Claude returned empty message");
    }
    Ok(message)
}

/// One-button: stage all + generate message + commit + push (blocking)
pub fn one_button_commit_and_push(root: &Path) -> Result<String> {
    stage_all(root)?;
    let message = generate_commit_message(root)?;
    let oid = commit(root, &message)?;
    push(root)?;
    Ok(format!("{} pushed: {}", oid, message))
}
