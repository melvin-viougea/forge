use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::session::{AgentSession, AgentStatus};

/// Manages multiple Claude Code agent sessions
pub struct AgentManager {
    sessions: HashMap<String, AgentSession>,
    active_session_id: Option<String>,
    default_working_dir: PathBuf,
    next_agent_num: usize,
}

impl AgentManager {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            sessions: HashMap::new(),
            active_session_id: None,
            default_working_dir: working_dir,
            next_agent_num: 1,
        }
    }

    /// Spawn a new agent with an initial prompt
    pub fn new_agent(&mut self, prompt: &str) -> Result<String> {
        let name = format!("Agent {}", self.next_agent_num);
        self.next_agent_num += 1;

        let mut session = AgentSession::new(name, self.default_working_dir.clone());
        session.spawn(prompt)?;

        let id = session.id.clone();
        self.active_session_id = Some(id.clone());
        self.sessions.insert(id.clone(), session);

        Ok(id)
    }

    /// Spawn a new agent in a specific directory
    pub fn new_agent_in_dir(&mut self, prompt: &str, working_dir: PathBuf) -> Result<String> {
        let name = format!("Agent {}", self.next_agent_num);
        self.next_agent_num += 1;

        let mut session = AgentSession::new(name, working_dir);
        session.spawn(prompt)?;

        let id = session.id.clone();
        self.active_session_id = Some(id.clone());
        self.sessions.insert(id.clone(), session);

        Ok(id)
    }

    /// Get a session by ID
    pub fn get_session(&self, id: &str) -> Option<&AgentSession> {
        self.sessions.get(id)
    }

    /// Get a mutable session by ID
    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut AgentSession> {
        self.sessions.get_mut(id)
    }

    /// Get the active session
    pub fn active_session(&self) -> Option<&AgentSession> {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.sessions.get(id))
    }

    /// Set active session
    pub fn set_active(&mut self, id: &str) {
        if self.sessions.contains_key(id) {
            self.active_session_id = Some(id.to_string());
        }
    }

    /// List all sessions
    pub fn list_sessions(&self) -> Vec<(&str, &str, &AgentStatus)> {
        self.sessions
            .values()
            .map(|s| (s.id.as_str(), s.name.as_str(), &s.status))
            .collect()
    }

    /// Kill an agent
    pub fn kill_agent(&mut self, id: &str) {
        if let Some(session) = self.sessions.get_mut(id) {
            session.terminate();
        }
    }

    /// Remove terminated sessions
    pub fn cleanup(&mut self) {
        self.sessions
            .retain(|_, s| s.status != AgentStatus::Terminated);

        if let Some(ref active_id) = self.active_session_id {
            if !self.sessions.contains_key(active_id) {
                self.active_session_id = self.sessions.keys().next().cloned();
            }
        }
    }

    /// Get agent count
    pub fn agent_count(&self) -> usize {
        self.sessions.len()
    }

    /// Get running agent count
    pub fn running_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.status == AgentStatus::Running)
            .count()
    }
}
