use std::path::PathBuf;

const SESSION_FILE: &str = ".forge/session.json";

fn session_path() -> Option<PathBuf> {
    dirs_next().map(|home| home.join(SESSION_FILE))
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

/// Saved session: list of project paths + which was active
pub struct SavedSession {
    pub projects: Vec<PathBuf>,
    pub active: usize,
}

/// Save the current session to ~/.forge/session.json
pub fn save(projects: &[PathBuf], active: usize) {
    let Some(path) = session_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    // Simple JSON: {"active":0,"projects":["/path/a","/path/b"]}
    let paths_json: Vec<String> = projects
        .iter()
        .map(|p| format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();

    let json = format!(
        "{{\"active\":{},\"projects\":[{}]}}",
        active,
        paths_json.join(",")
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

    Some(SavedSession {
        active: active.min(projects.len().saturating_sub(1)),
        projects,
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
