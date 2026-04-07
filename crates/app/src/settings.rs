use std::path::PathBuf;
use ide_workspace::theme::ThemeName;

const SETTINGS_FILE: &str = ".forge/settings.json";

fn settings_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(SETTINGS_FILE))
}

pub struct SavedSettings {
    pub theme: ThemeName,
}

pub fn save(theme: ThemeName) {
    let Some(path) = settings_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let json = format!("{{\"theme\":\"{}\"}}", theme.as_str());
    let _ = std::fs::write(&path, json);
}

pub fn load() -> SavedSettings {
    let theme = settings_path()
        .and_then(|p| std::fs::read_to_string(&p).ok())
        .and_then(|content| parse_string(&content, "theme"))
        .and_then(|s| ThemeName::from_str(&s))
        .unwrap_or(ThemeName::ForgeDark);
    SavedSettings { theme }
}

fn parse_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let after_colon = after[colon + 1..].trim_start();
    if after_colon.starts_with('"') {
        let end = after_colon[1..].find('"')?;
        Some(after_colon[1..1 + end].to_string())
    } else {
        None
    }
}
