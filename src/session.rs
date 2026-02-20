use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::ai::types::{Message, Role};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    pub provider: String,
    pub model: String,
    pub working_directory: Option<String>,
}

impl Session {
    pub fn new(provider: &str, model: &str) -> Self {
        let now = Utc::now();
        Session {
            id: Uuid::new_v4().to_string(),
            title: format!("Session {}", now.format("%Y-%m-%d %H:%M")),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            provider: provider.to_string(),
            model: model.to_string(),
            working_directory: std::env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string()),
        }
    }

    pub fn add_message(&mut self, role: Role, content: String) {
        self.messages.push(Message { role, content: content.clone() });
        self.updated_at = Utc::now();

        // Auto-generate title from first user message
        if self.messages.len() == 1 {
            let title = content
                .lines()
                .next()
                .unwrap_or("New session")
                .chars()
                .take(50)
                .collect::<String>();
            self.title = title;
        }
    }

    pub fn sessions_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openrust")
            .join("sessions")
    }

    pub fn save(&self) -> Result<()> {
        let dir = Self::sessions_dir();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create sessions directory {}", dir.display()))?;

        let path = dir.join(format!("{}.json", self.id));
        let content = serde_json::to_string_pretty(self)
            .with_context(|| "Failed to serialize session")?;

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write session to {}", path.display()))?;

        Ok(())
    }

    pub fn load(id: &str) -> Result<Self> {
        let path = Self::sessions_dir().join(format!("{}.json", id));
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read session {}", id))?;
        let session = serde_json::from_str(&content)
            .with_context(|| "Failed to parse session")?;
        Ok(session)
    }

    pub fn list_all() -> Result<Vec<SessionMeta>> {
        let dir = Self::sessions_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions: Vec<SessionMeta> = std::fs::read_dir(&dir)
            .with_context(|| "Failed to read sessions directory")?
            .filter_map(|entry| entry.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
            .filter_map(|e| {
                let content = std::fs::read_to_string(e.path()).ok()?;
                let session: Session = serde_json::from_str(&content).ok()?;
                Some(SessionMeta {
                    id: session.id,
                    title: session.title,
                    created_at: session.created_at,
                    updated_at: session.updated_at,
                    message_count: session.messages.len(),
                    provider: session.provider,
                    model: session.model,
                })
            })
            .collect();

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn delete(id: &str) -> Result<()> {
        let path = Self::sessions_dir().join(format!("{}.json", id));
        std::fs::remove_file(&path)
            .with_context(|| format!("Failed to delete session {}", id))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub provider: String,
    pub model: String,
}
