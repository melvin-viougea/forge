use std::path::PathBuf;
use ide_workspace::theme::ThemeName;

const SETTINGS_FILE: &str = ".forge/settings.json";

fn settings_path() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(|h| PathBuf::from(h).join(SETTINGS_FILE))
}

pub struct SavedSettings {
    pub theme: ThemeName,
    pub wallpaper: Option<String>,
    pub wallpaper_opacity: f32,
    pub wallpaper_crop_x: f32,
    pub wallpaper_crop_y: f32,
    pub wallpaper_crop_zoom: f32,
}

pub fn save(theme: ThemeName, wallpaper: Option<&str>, wallpaper_opacity: f32, crop_x: f32, crop_y: f32, crop_zoom: f32) {
    let Some(path) = settings_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let wp = match wallpaper {
        Some(p) => format!(",\"wallpaper\":\"{}\"", p.replace('\\', "\\\\").replace('"', "\\\"")),
        None => String::new(),
    };
    let json = format!(
        "{{\"theme\":\"{}\"{},\"wallpaper_opacity\":{:.2},\"wallpaper_crop_x\":{:.4},\"wallpaper_crop_y\":{:.4},\"wallpaper_crop_zoom\":{:.4}}}",
        theme.as_str(), wp, wallpaper_opacity, crop_x, crop_y, crop_zoom
    );
    let _ = std::fs::write(&path, json);
}

pub fn load() -> SavedSettings {
    let content = settings_path()
        .and_then(|p| std::fs::read_to_string(&p).ok());

    let theme = content.as_ref()
        .and_then(|c| parse_string(c, "theme"))
        .and_then(|s| ThemeName::from_str(&s))
        .unwrap_or(ThemeName::ForgeDark);

    let wallpaper = content.as_ref()
        .and_then(|c| parse_string(c, "wallpaper"))
        .filter(|p| PathBuf::from(p).exists());

    let wallpaper_opacity = content.as_ref()
        .and_then(|c| parse_number(c, "wallpaper_opacity"))
        .unwrap_or(0.65);

    let wallpaper_crop_x = content.as_ref()
        .and_then(|c| parse_number(c, "wallpaper_crop_x"))
        .unwrap_or(0.5);

    let wallpaper_crop_y = content.as_ref()
        .and_then(|c| parse_number(c, "wallpaper_crop_y"))
        .unwrap_or(0.5);

    let wallpaper_crop_zoom = content.as_ref()
        .and_then(|c| parse_number(c, "wallpaper_crop_zoom"))
        .unwrap_or(1.0);

    SavedSettings { theme, wallpaper, wallpaper_opacity, wallpaper_crop_x, wallpaper_crop_y, wallpaper_crop_zoom }
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

fn parse_number(json: &str, key: &str) -> Option<f32> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let after_colon = after[colon + 1..].trim_start();
    let end = after_colon.find(|c: char| c == ',' || c == '}').unwrap_or(after_colon.len());
    after_colon[..end].trim().parse().ok()
}
