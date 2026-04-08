use std::path::PathBuf;
use std::process::Command;

pub const CURRENT_VERSION: &str = "1.1.0";
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
        let download_url = extract_dmg_url(&body)
            .unwrap_or_else(|| {
                format!("https://github.com/{}/releases/tag/{}", GITHUB_REPO, tag)
            });

        Some(UpdateInfo { version, download_url })
    } else {
        None
    }
}

/// Get the expected download size via a HEAD request
pub fn get_download_size(url: &str) -> Option<u64> {
    let output = Command::new("curl")
        .args(["-sLI", "-o", "/dev/null", "-w", "%{size_download}\n%{redirect_url}", url])
        .output()
        .ok()?;
    // Try content-length from headers
    let headers = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    log::info!("Updater: HEAD response: {}", stdout.trim());

    // Use curl -sLI to follow redirects and get Content-Length
    let output2 = Command::new("curl")
        .args(["-sLI", url])
        .output()
        .ok()?;
    let header_text = String::from_utf8_lossy(&output2.stdout);
    for line in header_text.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(val) = line.split(':').nth(1) {
                if let Ok(size) = val.trim().parse::<u64>() {
                    if size > 1000 {
                        log::info!("Updater: expected download size: {} bytes", size);
                        return Some(size);
                    }
                }
            }
        }
    }
    None
}

/// Start download as a child process, returns the child
pub fn start_download(info: &UpdateInfo) -> Result<std::process::Child, String> {
    let dmg_path = "/tmp/forge-update.dmg";
    let mount_point = "/tmp/forge-update-mount";
    let _ = std::fs::remove_file(dmg_path);
    let _ = Command::new("hdiutil").args(["detach", mount_point, "-quiet"]).status();

    log::info!("Updater: downloading from {}", info.download_url);

    let child = Command::new("curl")
        .args(["-L", "-f", "-o", dmg_path, &info.download_url])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("Download failed to start: {}", e))?;

    Ok(child)
}

/// Check current download progress (file size)
pub fn download_progress() -> u64 {
    std::fs::metadata("/tmp/forge-update.dmg")
        .map(|m| m.len())
        .unwrap_or(0)
}

/// Verify download completed successfully
pub fn verify_download() -> Result<(), String> {
    let dmg_path = "/tmp/forge-update.dmg";
    match std::fs::metadata(dmg_path) {
        Ok(m) => {
            log::info!("Updater: downloaded {} bytes", m.len());
            if m.len() < 1000 {
                return Err(format!("Downloaded file too small ({} bytes)", m.len()));
            }
            Ok(())
        }
        Err(e) => Err(format!("Downloaded file not found: {}", e)),
    }
}

pub fn update_step_install() -> Result<(), String> {
    let dmg_path = "/tmp/forge-update.dmg";
    let mount_point = "/tmp/forge-update-mount";

    log::info!("Updater: mounting DMG...");
    let output = Command::new("hdiutil")
        .args(["attach", dmg_path, "-mountpoint", mount_point, "-nobrowse", "-quiet"])
        .output()
        .map_err(|e| format!("Mount failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Mount DMG failed: {}", stderr.trim()));
    }

    let new_app = format!("{}/Forge.app", mount_point);
    if !std::path::Path::new(&new_app).exists() {
        log::error!("Updater: Forge.app not found in DMG at {}", new_app);
        let _ = Command::new("hdiutil").args(["detach", mount_point, "-quiet"]).status();
        return Err("Forge.app not found in DMG".to_string());
    }

    let app_path = find_app_bundle()?;
    log::info!("Updater: installing to {:?}", app_path);
    let _ = std::fs::remove_dir_all(&app_path);
    let status = Command::new("cp")
        .args(["-R", &new_app, &app_path.display().to_string()])
        .status()
        .map_err(|e| format!("Install failed: {}", e))?;

    let _ = Command::new("hdiutil").args(["detach", mount_point, "-quiet"]).status();
    let _ = std::fs::remove_file(dmg_path);

    if !status.success() {
        return Err("Failed to copy new version".to_string());
    }
    log::info!("Updater: install complete");
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

fn extract_dmg_url(json: &str) -> Option<String> {
    let key = "\"browser_download_url\"";
    let mut search_from = 0;
    while let Some(pos) = json[search_from..].find(key) {
        let abs_pos = search_from + pos;
        if let Some(url) = extract_json_string(&json[abs_pos..], "browser_download_url") {
            if url.ends_with(".dmg") {
                return Some(url);
            }
        }
        search_from = abs_pos + key.len();
    }
    None
}

fn is_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    parse(remote) > parse(local)
}
