use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::ai::types::{ContentBlock, Message, Role};

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
        let message = match role {
            Role::User => Message::new_user(content.clone()),
            Role::Assistant => Message::new_assistant(content.clone()),
            Role::System => Message {
                role: Role::System,
                content: vec![ContentBlock::Text {
                    text: content.clone(),
                }],
            },
        };

        self.messages.push(message);
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

    /// Returns the old JSON sessions directory path (for backward compatibility)
    pub fn sessions_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("openrust")
            .join("sessions")
    }

    pub fn save(&self) -> Result<()> {
        migrate_json_sessions_once()?;

        let conn = get_db_connection()?;

        conn.execute_batch("BEGIN TRANSACTION")?;

        conn.execute(
            "INSERT OR REPLACE INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &self.id,
                &self.title,
                self.created_at.to_rfc3339(),
                self.updated_at.to_rfc3339(),
                &self.provider,
                &self.model,
                &self.working_directory,
            ],
        )
        .with_context(|| "Failed to insert session")?;

        conn.execute(
            "DELETE FROM messages WHERE session_id = ?1",
            params![&self.id],
        )
        .with_context(|| "Failed to delete old messages")?;

        for (idx, message) in self.messages.iter().enumerate() {
            let content_json = serde_json::to_string(&message.content)
                .with_context(|| "Failed to serialize message content")?;
            let role_str = message.role.to_string();
            let now = Utc::now().to_rfc3339();

            conn.execute(
                "INSERT INTO messages (session_id, role, content_json, created_at, ordering)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![&self.id, role_str, content_json, now, idx as i64],
            )
            .with_context(|| format!("Failed to insert message {}", idx))?;
        }

        conn.execute_batch("COMMIT")?;

        Ok(())
    }

    pub fn load(id: &str) -> Result<Self> {
        migrate_json_sessions_once()?;

        let conn = get_db_connection()?;

        let mut stmt = conn
            .prepare("SELECT id, title, created_at, updated_at, provider, model, working_directory FROM sessions WHERE id = ?1")
            .with_context(|| "Failed to prepare session query")?;

        let session_result = stmt.query_row(params![id], |row| {
            let created_str: String = row.get(2)?;
            let updated_str: String = row.get(3)?;

            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                DateTime::parse_from_rfc3339(&created_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc),
                DateTime::parse_from_rfc3339(&updated_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc),
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        });

        let (id, title, created_at, updated_at, provider, model, working_directory) =
            session_result.with_context(|| format!("Session {} not found", id))?;

        let mut stmt = conn
            .prepare("SELECT role, content_json FROM messages WHERE session_id = ?1 ORDER BY ordering ASC")
            .with_context(|| "Failed to prepare messages query")?;

        let messages_iter = stmt.query_map(params![&id], |row| {
            let role_str: String = row.get(0)?;
            let content_json: String = row.get(1)?;

            let role = match role_str.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                _ => Role::User,
            };

            let content: Vec<ContentBlock> = serde_json::from_str(&content_json)
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

            Ok(Message { role, content })
        })?;

        let mut messages = Vec::new();
        for message_result in messages_iter {
            messages.push(message_result?);
        }

        Ok(Session {
            id,
            title,
            created_at,
            updated_at,
            messages,
            provider,
            model,
            working_directory,
        })
    }

    pub fn list_all() -> Result<Vec<SessionMeta>> {
        migrate_json_sessions_once()?;

        let conn = get_db_connection()?;

        let mut stmt = conn
            .prepare(
                "SELECT s.id, s.title, s.created_at, s.updated_at, s.provider, s.model,
                        (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) as message_count
                 FROM sessions s
                 ORDER BY s.updated_at DESC",
            )
            .with_context(|| "Failed to prepare list query")?;

        let sessions_iter = stmt.query_map([], |row| {
            let created_str: String = row.get(2)?;
            let updated_str: String = row.get(3)?;

            Ok(SessionMeta {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&updated_str)
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?
                    .with_timezone(&Utc),
                message_count: row.get::<_, i64>(6)? as usize,
                provider: row.get(4)?,
                model: row.get(5)?,
            })
        })?;

        let mut sessions = Vec::new();
        for session_result in sessions_iter {
            sessions.push(session_result?);
        }

        Ok(sessions)
    }

    pub fn delete(id: &str) -> Result<()> {
        let conn = get_db_connection()?;

        let rows_affected = conn
            .execute("DELETE FROM sessions WHERE id = ?1", params![id])
            .with_context(|| format!("Failed to delete session {}", id))?;

        if rows_affected == 0 {
            anyhow::bail!("Session {} not found", id);
        }

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

/// Returns the path to the SQLite database file
fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("openrust")
        .join("sessions.db")
}

/// Get a database connection with WAL mode and foreign keys enabled
fn get_db_connection() -> Result<Connection> {
    let db_path = db_path();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create database directory {}", parent.display()))?;
    }

    let conn = Connection::open(&db_path)
        .with_context(|| format!("Failed to open database at {}", db_path.display()))?;

    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .with_context(|| "Failed to set database pragmas")?;

    init_schema(&conn)?;

    Ok(conn)
}

/// Initialize database schema (idempotent)
fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            provider TEXT NOT NULL,
            model TEXT NOT NULL,
            working_directory TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            ordering INTEGER NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
        "#,
    )
    .with_context(|| "Failed to initialize database schema")?;

    Ok(())
}

/// Migrate old JSON session files to SQLite (runs once)
fn migrate_json_sessions_once() -> Result<()> {
    let sessions_dir = Session::sessions_dir();
    let backup_dir = sessions_dir.parent().unwrap().join("sessions_backup");

    if backup_dir.exists() {
        return Ok(());
    }

    if !sessions_dir.exists() {
        return Ok(());
    }

    let json_files: Vec<_> = std::fs::read_dir(&sessions_dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map_or(false, |ext| ext == "json"))
                .collect()
        })
        .unwrap_or_default();

    if json_files.is_empty() {
        std::fs::create_dir_all(&backup_dir)
            .with_context(|| "Failed to create backup directory")?;
        return Ok(());
    }

    eprintln!(
        "Migrating {} JSON session(s) to SQLite...",
        json_files.len()
    );

    let conn = get_db_connection()?;

    for entry in json_files {
        let path = entry.path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(session) = serde_json::from_str::<Session>(&content) {
                conn.execute(
                    "INSERT OR REPLACE INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        &session.id,
                        &session.title,
                        session.created_at.to_rfc3339(),
                        session.updated_at.to_rfc3339(),
                        &session.provider,
                        &session.model,
                        &session.working_directory,
                    ],
                )
                .ok();

                for (idx, message) in session.messages.iter().enumerate() {
                    if let Ok(content_json) = serde_json::to_string(&message.content) {
                        let role_str = message.role.to_string();
                        let now = Utc::now().to_rfc3339();

                        conn.execute(
                            "INSERT INTO messages (session_id, role, content_json, created_at, ordering)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![&session.id, role_str, content_json, now, idx as i64],
                        )
                        .ok();
                    }
                }
            }
        }
    }

    std::fs::rename(&sessions_dir, &backup_dir)
        .with_context(|| "Failed to move old sessions to backup")?;

    eprintln!(
        "Migration complete. Old sessions backed up to: {}",
        backup_dir.display()
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_db() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_sessions.db");
        (temp_dir, db_path)
    }

    fn with_test_db<F>(f: F)
    where
        F: FnOnce(&Connection),
    {
        let (_temp_dir, db_path) = setup_test_db();
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .unwrap();
        init_schema(&conn).unwrap();
        f(&conn);
    }

    #[test]
    fn test_session_roundtrip() {
        with_test_db(|conn| {
            // Create a session
            let mut session = Session::new("anthropic", "claude-3-5-sonnet-20241022");
            session.add_message(Role::User, "Hello".to_string());
            session.add_message(Role::Assistant, "Hi there!".to_string());

            // Save to database
            conn.execute(
                "INSERT INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    &session.id,
                    &session.title,
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                    &session.provider,
                    &session.model,
                    &session.working_directory,
                ],
            )
            .unwrap();

            for (idx, message) in session.messages.iter().enumerate() {
                let content_json = serde_json::to_string(&message.content).unwrap();
                conn.execute(
                    "INSERT INTO messages (session_id, role, content_json, created_at, ordering)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        &session.id,
                        message.role.to_string(),
                        content_json,
                        Utc::now().to_rfc3339(),
                        idx as i64
                    ],
                )
                .unwrap();
            }

            // Load from database
            let loaded_id = session.id.clone();
            let mut stmt = conn
                .prepare("SELECT id, title, created_at, updated_at, provider, model, working_directory FROM sessions WHERE id = ?1")
                .unwrap();

            let (id, title, created_at, updated_at, provider, model, working_directory) = stmt
                .query_row(params![&loaded_id], |row| {
                    let created_str: String = row.get(2)?;
                    let updated_str: String = row.get(3)?;
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        DateTime::parse_from_rfc3339(&created_str)
                            .unwrap()
                            .with_timezone(&Utc),
                        DateTime::parse_from_rfc3339(&updated_str)
                            .unwrap()
                            .with_timezone(&Utc),
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                })
                .unwrap();

            assert_eq!(id, session.id);
            assert_eq!(title, session.title);
            assert_eq!(provider, "anthropic");
            assert_eq!(model, "claude-3-5-sonnet-20241022");

            // Load messages
            let mut stmt = conn
                .prepare("SELECT role, content_json FROM messages WHERE session_id = ?1 ORDER BY ordering ASC")
                .unwrap();

            let messages: Vec<Message> = stmt
                .query_map(params![&loaded_id], |row| {
                    let role_str: String = row.get(0)?;
                    let content_json: String = row.get(1)?;
                    let role = match role_str.as_str() {
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        _ => Role::User,
                    };
                    let content: Vec<ContentBlock> = serde_json::from_str(&content_json).unwrap();
                    Ok(Message { role, content })
                })
                .unwrap()
                .map(|r| r.unwrap())
                .collect();

            assert_eq!(messages.len(), 2);
            assert_eq!(messages[0].text_content(), "Hello");
            assert_eq!(messages[1].text_content(), "Hi there!");
        });
    }

    #[test]
    fn test_list_sessions_sorted() {
        with_test_db(|conn| {
            // Create multiple sessions with different updated_at times
            let session1_id = Uuid::new_v4().to_string();
            let session2_id = Uuid::new_v4().to_string();

            let now = Utc::now();
            let earlier = now - chrono::Duration::hours(1);

            // Insert session 1 (older)
            conn.execute(
                "INSERT INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![&session1_id, "Session 1", earlier.to_rfc3339(), earlier.to_rfc3339(), "anthropic", "claude", None::<String>],
            )
            .unwrap();

            // Insert session 2 (newer)
            conn.execute(
                "INSERT INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![&session2_id, "Session 2", now.to_rfc3339(), now.to_rfc3339(), "openai", "gpt-4", None::<String>],
            )
            .unwrap();

            // List sessions
            let mut stmt = conn
                .prepare(
                    "SELECT s.id, s.title, s.created_at, s.updated_at, s.provider, s.model,
                            (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) as message_count
                     FROM sessions s
                     ORDER BY s.updated_at DESC",
                )
                .unwrap();

            let sessions: Vec<SessionMeta> = stmt
                .query_map([], |row| {
                    let created_str: String = row.get(2)?;
                    let updated_str: String = row.get(3)?;
                    Ok(SessionMeta {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at: DateTime::parse_from_rfc3339(&created_str)
                            .unwrap()
                            .with_timezone(&Utc),
                        updated_at: DateTime::parse_from_rfc3339(&updated_str)
                            .unwrap()
                            .with_timezone(&Utc),
                        message_count: row.get::<_, i64>(6)? as usize,
                        provider: row.get(4)?,
                        model: row.get(5)?,
                    })
                })
                .unwrap()
                .map(|r| r.unwrap())
                .collect();

            assert_eq!(sessions.len(), 2);
            // Newer session should be first
            assert_eq!(sessions[0].id, session2_id);
            assert_eq!(sessions[0].title, "Session 2");
            assert_eq!(sessions[1].id, session1_id);
            assert_eq!(sessions[1].title, "Session 1");
        });
    }

    #[test]
    fn test_delete_session() {
        with_test_db(|conn| {
            let session_id = Uuid::new_v4().to_string();
            let now = Utc::now();

            // Insert session
            conn.execute(
                "INSERT INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![&session_id, "Test Session", now.to_rfc3339(), now.to_rfc3339(), "anthropic", "claude", None::<String>],
            )
            .unwrap();

            // Insert a message
            conn.execute(
                "INSERT INTO messages (session_id, role, content_json, created_at, ordering)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    &session_id,
                    "user",
                    r#"[{"type":"text","text":"test"}]"#,
                    now.to_rfc3339(),
                    0
                ],
            )
            .unwrap();

            // Verify session exists
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                    params![&session_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);

            // Verify message exists
            let msg_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                    params![&session_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(msg_count, 1);

            // Delete session
            let rows = conn
                .execute("DELETE FROM sessions WHERE id = ?1", params![&session_id])
                .unwrap();
            assert_eq!(rows, 1);

            // Verify session deleted
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sessions WHERE id = ?1",
                    params![&session_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);

            // Verify messages also deleted (CASCADE)
            let msg_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
                    params![&session_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(msg_count, 0);
        });
    }

    #[test]
    fn test_message_ordering() {
        with_test_db(|conn| {
            let session_id = Uuid::new_v4().to_string();
            let now = Utc::now();

            // Insert session
            conn.execute(
                "INSERT INTO sessions (id, title, created_at, updated_at, provider, model, working_directory)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![&session_id, "Test", now.to_rfc3339(), now.to_rfc3339(), "anthropic", "claude", None::<String>],
            )
            .unwrap();

            // Insert messages in specific order
            for i in 0..5 {
                conn.execute(
                    "INSERT INTO messages (session_id, role, content_json, created_at, ordering)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        &session_id,
                        "user",
                        format!(r#"[{{"type":"text","text":"message {}"}}]"#, i),
                        now.to_rfc3339(),
                        i
                    ],
                )
                .unwrap();
            }

            // Load messages ordered
            let mut stmt = conn
                .prepare(
                    "SELECT content_json FROM messages WHERE session_id = ?1 ORDER BY ordering ASC",
                )
                .unwrap();

            let messages: Vec<String> = stmt
                .query_map(params![&session_id], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();

            assert_eq!(messages.len(), 5);
            for (i, content_json) in messages.iter().enumerate() {
                let content: Vec<ContentBlock> = serde_json::from_str(content_json).unwrap();
                assert_eq!(
                    content[0],
                    ContentBlock::Text {
                        text: format!("message {}", i)
                    }
                );
            }
        });
    }
}
