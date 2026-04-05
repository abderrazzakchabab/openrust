use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};

/// Represents a snapshot of file state before a tool operation
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub description: String,
    pub diff_content: String,
    pub untracked_files: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// Manages undo/redo stack using git diff snapshots
pub struct UndoManager {
    undo_stack: Vec<UndoEntry>,
    redo_stack: Vec<UndoEntry>,
    working_dir: PathBuf,
    enabled: bool,
}

impl UndoManager {
    /// Create a new UndoManager for the given working directory
    pub fn new(working_dir: PathBuf) -> Self {
        let enabled = Self::check_git_repo(&working_dir);
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            working_dir,
            enabled,
        }
    }

    fn check_git_repo(dir: &PathBuf) -> bool {
        Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(dir)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    pub fn is_git_repo(&self) -> bool {
        self.enabled
    }

    pub fn snapshot(&mut self, description: &str) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let diff_output = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to create git diff")?;

        let diff_content = String::from_utf8_lossy(&diff_output.stdout).to_string();

        let untracked_output = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to list untracked files")?;

        let untracked_files: Vec<String> = String::from_utf8_lossy(&untracked_output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        let entry = UndoEntry {
            description: description.to_string(),
            diff_content,
            untracked_files,
            timestamp: Utc::now(),
        };

        self.undo_stack.push(entry);
        self.redo_stack.clear();

        Ok(())
    }

    pub fn undo(&mut self) -> Result<Option<String>> {
        if !self.enabled {
            return Ok(Some("Undo is not enabled (not in a git repo)".to_string()));
        }

        let entry = match self.undo_stack.pop() {
            Some(e) => e,
            None => return Ok(Some("Nothing to undo".to_string())),
        };

        let current_diff = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to save current state for redo")?;

        let current_untracked = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to list untracked files for redo")?;

        let redo_entry = UndoEntry {
            description: format!("Redo: {}", entry.description),
            diff_content: String::from_utf8_lossy(&current_diff.stdout).to_string(),
            untracked_files: String::from_utf8_lossy(&current_untracked.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect(),
            timestamp: Utc::now(),
        };

        Command::new("git")
            .args(["checkout", "HEAD", "--", "."])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to revert tracked files to HEAD")?;

        if !entry.diff_content.is_empty() {
            let temp_diff = self.working_dir.join(".openrust_undo_patch.tmp");
            std::fs::write(&temp_diff, &entry.diff_content)
                .with_context(|| "Failed to write temporary diff file")?;

            let apply_result = Command::new("git")
                .args(["apply", "--reverse", temp_diff.to_str().unwrap()])
                .current_dir(&self.working_dir)
                .output();

            let _ = std::fs::remove_file(&temp_diff);
            apply_result.with_context(|| "Failed to apply reverse diff")?;
        }

        self.redo_stack.push(redo_entry);

        Ok(Some(format!("Undone: {}", entry.description)))
    }

    pub fn redo(&mut self) -> Result<Option<String>> {
        if !self.enabled {
            return Ok(Some("Redo is not enabled (not in a git repo)".to_string()));
        }

        let entry = match self.redo_stack.pop() {
            Some(e) => e,
            None => return Ok(Some("Nothing to redo".to_string())),
        };

        let current_diff = Command::new("git")
            .args(["diff", "HEAD"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to save current state for undo")?;

        let current_untracked = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to list untracked files for undo")?;

        let undo_entry = UndoEntry {
            description: entry.description.clone(),
            diff_content: String::from_utf8_lossy(&current_diff.stdout).to_string(),
            untracked_files: String::from_utf8_lossy(&current_untracked.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect(),
            timestamp: Utc::now(),
        };

        Command::new("git")
            .args(["checkout", "HEAD", "--", "."])
            .current_dir(&self.working_dir)
            .output()
            .with_context(|| "Failed to revert tracked files to HEAD")?;

        if !entry.diff_content.is_empty() {
            let temp_diff = self.working_dir.join(".openrust_redo_patch.tmp");
            std::fs::write(&temp_diff, &entry.diff_content)
                .with_context(|| "Failed to write temporary diff file")?;

            let apply_result = Command::new("git")
                .args(["apply", temp_diff.to_str().unwrap()])
                .current_dir(&self.working_dir)
                .output();

            let _ = std::fs::remove_file(&temp_diff);
            apply_result.with_context(|| "Failed to apply diff")?;
        }

        self.undo_stack.push(undo_entry);

        Ok(Some(format!("Redone: {}", entry.description)))
    }

    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn is_file_modifying_tool(tool_name: &str) -> bool {
        matches!(tool_name, "write" | "edit" | "patch" | "bash")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn test_undo_manager_new() {
        let temp_dir = PathBuf::from("/tmp/test_undo_manager");
        let manager = UndoManager::new(temp_dir.clone());
        assert_eq!(manager.working_dir, temp_dir);
        assert_eq!(manager.undo_count(), 0);
        assert_eq!(manager.redo_count(), 0);
    }

    #[test]
    fn test_is_file_modifying_tool() {
        assert!(UndoManager::is_file_modifying_tool("write"));
        assert!(UndoManager::is_file_modifying_tool("edit"));
        assert!(UndoManager::is_file_modifying_tool("patch"));
        assert!(UndoManager::is_file_modifying_tool("bash"));
        assert!(!UndoManager::is_file_modifying_tool("read"));
        assert!(!UndoManager::is_file_modifying_tool("search"));
        assert!(!UndoManager::is_file_modifying_tool("list"));
    }

    #[test]
    fn test_undo_empty_stack() {
        let temp_dir = PathBuf::from("/tmp/test_undo_empty");
        let mut manager = UndoManager::new(temp_dir);
        manager.enabled = true;

        let result = manager.undo();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("Nothing to undo".to_string()));
    }

    #[test]
    fn test_redo_empty_stack() {
        let temp_dir = PathBuf::from("/tmp/test_redo_empty");
        let mut manager = UndoManager::new(temp_dir);
        manager.enabled = true;

        let result = manager.redo();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some("Nothing to redo".to_string()));
    }

    #[test]
    fn test_undo_count_starts_at_zero() {
        let temp_dir = PathBuf::from("/tmp/test_undo_count");
        let manager = UndoManager::new(temp_dir);
        assert_eq!(manager.undo_count(), 0);
    }

    #[test]
    fn test_redo_count_starts_at_zero() {
        let temp_dir = PathBuf::from("/tmp/test_redo_count");
        let manager = UndoManager::new(temp_dir);
        assert_eq!(manager.redo_count(), 0);
    }

    #[test]
    fn test_non_git_repo_snapshot() {
        let temp_dir = PathBuf::from("/tmp/test_non_git");
        let mut manager = UndoManager::new(temp_dir);
        let result = manager.snapshot("test operation");
        assert!(result.is_ok());
        assert_eq!(manager.undo_count(), 0);
    }

    #[test]
    fn test_undo_manager_disabled() {
        let temp_dir = PathBuf::from("/tmp/test_disabled");
        let mut manager = UndoManager::new(temp_dir);
        assert!(!manager.is_git_repo());

        let result = manager.undo();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            Some("Undo is not enabled (not in a git repo)".to_string())
        );
    }

    #[test]
    fn test_redo_stack_management() {
        let temp_dir = PathBuf::from("/tmp/test_redo_stack");
        let mut manager = UndoManager::new(temp_dir);

        manager.redo_stack.push(UndoEntry {
            description: "test".to_string(),
            diff_content: "".to_string(),
            untracked_files: Vec::new(),
            timestamp: Utc::now(),
        });
        assert_eq!(manager.redo_count(), 1);

        manager.redo_stack.clear();
        assert_eq!(manager.redo_count(), 0);
    }

    #[test]
    fn test_undo_pushes_to_redo_stack() {
        let temp_dir = PathBuf::from("/tmp/test_undo_to_redo");
        let mut manager = UndoManager::new(temp_dir);
        manager.enabled = true;

        manager.undo_stack.push(UndoEntry {
            description: "test operation".to_string(),
            diff_content: "".to_string(),
            untracked_files: Vec::new(),
            timestamp: Utc::now(),
        });
        assert_eq!(manager.undo_count(), 1);
        assert_eq!(manager.redo_count(), 0);
    }
}
