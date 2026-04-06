use std::path::PathBuf;
use std::process::Command;

pub const CURRENT_VERSION: &str = "0.9.3";
const GITHUB_REPO: &str = "melvin-viougea/forge";

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

/// Check GitHub releases for a newer version
pub fn check_for_update() -> Option<UpdateInfo> {
    let output = Command::new("curl")
        .args([
            "-s",
            "-H", "Accept: application/vnd.github.v3+json",
            &format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO),
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);
    let tag = extract_json_string(&body, "tag_name")?;
    let version = tag.trim_start_matches('v').to_string();

    if is_newer(&version, CURRENT_VERSION) {
        let download_url = extract_tar_gz_url(&body)
            .unwrap_or_else(|| {
                format!("https://github.com/{}/releases/tag/{}", GITHUB_REPO, tag)
            });

        Some(UpdateInfo { version, download_url })
    } else {
        None
    }
}

/// Download and install update
pub fn download_and_install(info: &UpdateInfo) -> Result<(), String> {
    let tar_path = "/tmp/forge-update.tar.gz";
    let extract_dir = "/tmp/forge-update";

    // Clean previous attempt
    let _ = std::fs::remove_file(tar_path);
    let _ = std::fs::remove_dir_all(extract_dir);
    let _ = std::fs::create_dir_all(extract_dir);

    // Download
    let status = Command::new("curl")
        .args(["-L", "-s", "-o", tar_path, &info.download_url])
        .status()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !status.success() {
        return Err("Download failed".to_string());
    }

    // Extract
    let status = Command::new("tar")
        .args(["-xzf", tar_path, "-C", extract_dir])
        .status()
        .map_err(|e| format!("Extract failed: {}", e))?;

    if !status.success() {
        return Err("Extract failed".to_string());
    }

    // Find the app bundle to replace
    let app_path = find_app_bundle()?;

    // Verify extracted app exists
    let new_app = format!("{}/Forge.app", extract_dir);
    if !std::path::Path::new(&new_app).exists() {
        return Err("Extracted app not found".to_string());
    }

    // Remove old app and replace with new one
    let _ = std::fs::remove_dir_all(&app_path);
    let status = Command::new("cp")
        .args(["-R", &new_app, &app_path.display().to_string()])
        .status()
        .map_err(|e| format!("Install failed: {}", e))?;

    if !status.success() {
        return Err("Failed to copy new version".to_string());
    }

    // Cleanup
    let _ = std::fs::remove_file(tar_path);
    let _ = std::fs::remove_dir_all(extract_dir);

    Ok(())
}

/// Relaunch the app
pub fn relaunch() {
    let app_path = find_app_bundle().unwrap_or_default();
    // Clear quarantine on the new app
    let _ = Command::new("xattr").args(["-cr", &app_path.display().to_string()]).status();
    let _ = Command::new("open").args(["-n", &app_path.display().to_string()]).spawn();
    std::process::exit(0);
}

/// Find the .app bundle path
fn find_app_bundle() -> Result<PathBuf, String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Can't find current exe: {}", e))?;

    // Navigate up from .app/Contents/MacOS/forge to .app
    let mut path = current_exe.as_path();
    for _ in 0..5 {
        if let Some(parent) = path.parent() {
            if parent.extension().map_or(false, |e| e == "app") {
                return Ok(parent.to_path_buf());
            }
            path = parent;
        }
    }

    // Fallback: check /Applications/Forge.app
    let applications = PathBuf::from("/Applications/Forge.app");
    if applications.exists() {
        return Ok(applications);
    }

    Err("Can't find Forge.app bundle".to_string())
}

// ── Helpers ──

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after_key = &json[start + pattern.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    if after_colon.starts_with('"') {
        let value_start = 1;
        let value_end = after_colon[value_start..].find('"')?;
        Some(after_colon[value_start..value_start + value_end].to_string())
    } else {
        None
    }
}

fn extract_tar_gz_url(json: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(pos) = json[search_from..].find("browser_download_url") {
        let abs_pos = search_from + pos;
        if let Some(url) = extract_json_string(&json[abs_pos..], "browser_download_url") {
            if url.ends_with(".tar.gz") {
                return Some(url);
            }
        }
        search_from = abs_pos + 20;
    }
    None
}

fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    parse(remote) > parse(local)
}
