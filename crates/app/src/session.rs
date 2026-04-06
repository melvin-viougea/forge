use std::path::PathBuf;

const SESSION_FILE: &str = ".forge/session.json";

fn session_path() -> Option<PathBuf> {
    dirs_next().map(|home| home.join(SESSION_FILE))
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Layout dimensions to persist
#[derive(Clone)]
pub struct SavedLayout {
    pub left_dock_width: f32,
    pub right_dock_width: f32,
    pub pane_sidebar_width: f32,
    pub log_height: f32,
    pub log_expanded: bool,
}

impl Default for SavedLayout {
    fn default() -> Self {
        Self {
            left_dock_width: 200.,
            right_dock_width: 280.,
            pane_sidebar_width: 240.,
            log_height: 250.,
            log_expanded: false,
        }
    }
}

/// Saved session: list of project paths + which was active + layout
pub struct SavedSession {
    pub projects: Vec<PathBuf>,
    pub active: usize,
    pub layout: SavedLayout,
}

/// Save the current session to ~/.forge/session.json
pub fn save(projects: &[PathBuf], active: usize, layout: &SavedLayout) {
    let Some(path) = session_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let paths_json: Vec<String> = projects
        .iter()
        .map(|p| format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();

    let json = format!(
        concat!(
            "{{\"active\":{},\"projects\":[{}],",
            "\"left_dock_width\":{},\"right_dock_width\":{},",
            "\"pane_sidebar_width\":{},\"log_height\":{},\"log_expanded\":{}}}"
        ),
        active,
        paths_json.join(","),
        layout.left_dock_width,
        layout.right_dock_width,
        layout.pane_sidebar_width,
        layout.log_height,
        layout.log_expanded,
    );

    let _ = std::fs::write(&path, json);
}

/// Load the saved session from ~/.forge/session.json
pub fn load() -> Option<SavedSession> {
    let path = session_path()?;
    let content = std::fs::read_to_string(&path).ok()?;

    // Parse "active" field
    let active = parse_number(&content, "active").unwrap_or(0);

    // Parse "projects" array
    let projects = parse_string_array(&content, "projects")?;
    let projects: Vec<PathBuf> = projects
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect();

    if projects.is_empty() {
        return None;
    }

    // Parse layout fields (with defaults)
    let layout = SavedLayout {
        left_dock_width: parse_float(&content, "left_dock_width").unwrap_or(200.),
        right_dock_width: parse_float(&content, "right_dock_width").unwrap_or(280.),
        pane_sidebar_width: parse_float(&content, "pane_sidebar_width").unwrap_or(240.),
        log_height: parse_float(&content, "log_height").unwrap_or(250.),
        log_expanded: parse_bool(&content, "log_expanded").unwrap_or(false),
    };

    Some(SavedSession {
        active: active.min(projects.len().saturating_sub(1)),
        projects,
        layout,
    })
}

fn parse_number(json: &str, key: &str) -> Option<usize> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let value_str = after[colon + 1..].trim_start();
    let end = value_str.find(|c: char| !c.is_ascii_digit())?;
    value_str[..end].parse().ok()
}

fn parse_float(json: &str, key: &str) -> Option<f32> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let value_str = after[colon + 1..].trim_start();
    let end = value_str.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    value_str[..end].parse().ok()
}

fn parse_bool(json: &str, key: &str) -> Option<bool> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let colon = after.find(':')?;
    let value_str = after[colon + 1..].trim_start();
    if value_str.starts_with("true") {
        Some(true)
    } else if value_str.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn parse_string_array(json: &str, key: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)?;
    let after = &json[start + pattern.len()..];
    let bracket_start = after.find('[')?;
    let bracket_end = after.find(']')?;
    let inner = &after[bracket_start + 1..bracket_end];

    let mut result = Vec::new();
    let mut rest = inner.trim();

    while !rest.is_empty() {
        if !rest.starts_with('"') {
            break;
        }
        // Find the closing quote (handle escaped quotes)
        let mut end = 1;
        loop {
            match rest[end..].find('"') {
                Some(pos) => {
                    // Check if escaped
                    if pos > 0 && rest.as_bytes()[end + pos - 1] == b'\\' {
                        end += pos + 1;
                    } else {
                        end += pos;
                        break;
                    }
                }
                None => return Some(result),
            }
        }

        let value = rest[1..end].replace("\\\"", "\"").replace("\\\\", "\\");
        result.push(value);

        rest = rest[end + 1..].trim_start();
        if rest.starts_with(',') {
            rest = rest[1..].trim_start();
        }
    }

    Some(result)
}
