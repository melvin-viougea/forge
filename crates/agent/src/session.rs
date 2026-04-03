use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Represents a single Claude Code CLI session
pub struct AgentSession {
    pub id: String,
    pub name: String,
    pub working_dir: PathBuf,
    pub status: AgentStatus,
    process: Option<Child>,
    output_lines: Arc<Mutex<Vec<AgentOutputLine>>>,
    _reader_handle: Option<std::thread::JoinHandle<()>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AgentStatus {
    Starting,
    Running,
    Idle,
    Terminated,
    Error(String),
}

#[derive(Clone, Debug)]
pub struct AgentOutputLine {
    pub text: String,
    pub kind: OutputKind,
}

#[derive(Clone, Debug)]
pub enum OutputKind {
    Text,
    ToolUse,
    System,
    Error,
}

impl AgentSession {
    pub fn new(name: String, working_dir: PathBuf) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self {
            id,
            name,
            working_dir,
            status: AgentStatus::Starting,
            process: None,
            output_lines: Arc::new(Mutex::new(Vec::new())),
            _reader_handle: None,
        }
    }

    /// Spawn a Claude Code CLI session with an initial prompt
    pub fn spawn(&mut self, initial_prompt: &str) -> Result<()> {
        let mut child = Command::new("claude")
            .args([
                initial_prompt,
                "--output-format",
                "stream-json",
                "--bare",
                "--name",
                &self.name,
            ])
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .context("Failed to spawn Claude Code CLI. Is it installed?")?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to capture stdout")?;

        let output_lines = self.output_lines.clone();

        let reader_handle = std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(line_str) => {
                        let output_line = parse_stream_json(&line_str);
                        let mut lines = output_lines.lock().unwrap();
                        lines.push(output_line);
                        // Cap at 5000 lines
                        if lines.len() > 5000 {
                            let excess = lines.len() - 5000;
                            lines.drain(..excess);
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        self.process = Some(child);
        self._reader_handle = Some(reader_handle);
        self.status = AgentStatus::Running;

        Ok(())
    }

    /// Send a follow-up message to the agent
    pub fn send_message(&mut self, message: &str) -> Result<()> {
        if let Some(ref mut process) = self.process {
            if let Some(ref mut stdin) = process.stdin {
                stdin
                    .write_all(message.as_bytes())
                    .context("Failed to write to agent stdin")?;
                stdin
                    .write_all(b"\n")
                    .context("Failed to write newline")?;
                stdin.flush().context("Failed to flush stdin")?;
            }
        }
        Ok(())
    }

    /// Get output lines
    pub fn get_output(&self) -> Vec<AgentOutputLine> {
        self.output_lines.lock().unwrap().clone()
    }

    /// Get visible output (last N lines)
    pub fn get_visible_output(&self, max_lines: usize) -> Vec<AgentOutputLine> {
        let lines = self.output_lines.lock().unwrap();
        let start = if lines.len() > max_lines {
            lines.len() - max_lines
        } else {
            0
        };
        lines[start..].to_vec()
    }

    /// Check if process is still running
    pub fn is_alive(&mut self) -> bool {
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(_)) => {
                    self.status = AgentStatus::Terminated;
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    /// Kill the agent process
    pub fn terminate(&mut self) {
        if let Some(ref mut process) = self.process {
            let _ = process.kill();
        }
        self.status = AgentStatus::Terminated;
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// Parse a stream-json line from Claude Code CLI
fn parse_stream_json(line: &str) -> AgentOutputLine {
    if let Ok(json) = serde_json::from_str::<Value>(line) {
        let event_type = json
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match event_type {
            "stream_event" => {
                // Extract text delta
                if let Some(text) = json.pointer("/event/delta/text").and_then(|v| v.as_str()) {
                    return AgentOutputLine {
                        text: text.to_string(),
                        kind: OutputKind::Text,
                    };
                }
                // Extract tool use
                if let Some(tool) = json.pointer("/event/content/name").and_then(|v| v.as_str()) {
                    return AgentOutputLine {
                        text: format!("[Tool: {}]", tool),
                        kind: OutputKind::ToolUse,
                    };
                }
                AgentOutputLine {
                    text: String::new(),
                    kind: OutputKind::System,
                }
            }
            "system" => AgentOutputLine {
                text: json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                kind: OutputKind::System,
            },
            "error" => AgentOutputLine {
                text: json
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
                    .to_string(),
                kind: OutputKind::Error,
            },
            _ => AgentOutputLine {
                text: String::new(),
                kind: OutputKind::System,
            },
        }
    } else {
        // Plain text fallback
        AgentOutputLine {
            text: line.to_string(),
            kind: OutputKind::Text,
        }
    }
}
