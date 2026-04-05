use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Created(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatcherConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            paths: Vec::new(),
            ignore_patterns: vec![
                ".git/".to_string(),
                "target/".to_string(),
                "node_modules/".to_string(),
            ],
        }
    }
}

impl WatcherConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.paths = paths;
        self
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn should_ignore(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        for pattern in &self.ignore_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }
        false
    }
}

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    config: WatcherConfig,
}

impl FileWatcher {
    pub fn new(config: WatcherConfig) -> Self {
        Self {
            watcher: None,
            config,
        }
    }

    pub fn start(&mut self) -> Result<tokio::sync::mpsc::Receiver<FileEvent>> {
        if !self.config.enabled {
            anyhow::bail!("File watcher is disabled");
        }

        let (event_tx, event_rx) = tokio::sync::mpsc::channel(100);
        let (notify_tx, notify_rx) = mpsc::channel();

        let config = self.config.clone();

        let mut watcher = RecommendedWatcher::new(
            notify_tx,
            notify::Config::default().with_poll_interval(Duration::from_secs(2)),
        )
        .context("Failed to create file watcher")?;

        for path in &self.config.paths {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch path: {}", path.display()))?;
        }

        self.watcher = Some(watcher);

        tokio::spawn(async move {
            for event_result in notify_rx {
                match event_result {
                    Ok(event) => {
                        for path in &event.paths {
                            if config.should_ignore(path) {
                                continue;
                            }

                            let file_event = match event.kind {
                                EventKind::Create(_) => Some(FileEvent::Created(path.clone())),
                                EventKind::Modify(_) => Some(FileEvent::Modified(path.clone())),
                                EventKind::Remove(_) => Some(FileEvent::Deleted(path.clone())),
                                _ => None,
                            };

                            if let Some(file_event) = file_event {
                                if event_tx.send(file_event).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("File watcher error: {}", e);
                    }
                }
            }
        });

        Ok(event_rx)
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
        }
        Ok(())
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.paths.len(), 0);
        assert_eq!(config.ignore_patterns.len(), 3);
        assert!(config.ignore_patterns.contains(&".git/".to_string()));
        assert!(config.ignore_patterns.contains(&"target/".to_string()));
        assert!(config.ignore_patterns.contains(&"node_modules/".to_string()));
    }

    #[test]
    fn test_watcher_config_new() {
        let config = WatcherConfig::new();
        assert!(!config.enabled);
        assert_eq!(config.paths.len(), 0);
    }

    #[test]
    fn test_watcher_config_with_paths() {
        let paths = vec![PathBuf::from("/tmp/test")];
        let config = WatcherConfig::new().with_paths(paths.clone());
        assert_eq!(config.paths, paths);
    }

    #[test]
    fn test_watcher_config_with_enabled() {
        let config = WatcherConfig::new().with_enabled(true);
        assert!(config.enabled);
    }

    #[test]
    fn test_should_ignore_git() {
        let config = WatcherConfig::default();
        assert!(config.should_ignore(Path::new("/tmp/repo/.git/config")));
    }

    #[test]
    fn test_should_ignore_target() {
        let config = WatcherConfig::default();
        assert!(config.should_ignore(Path::new("/tmp/project/target/debug/app")));
    }

    #[test]
    fn test_should_ignore_node_modules() {
        let config = WatcherConfig::default();
        assert!(config.should_ignore(Path::new("/tmp/project/node_modules/package")));
    }

    #[test]
    fn test_should_not_ignore_regular_file() {
        let config = WatcherConfig::default();
        assert!(!config.should_ignore(Path::new("/tmp/project/src/main.rs")));
    }

    #[test]
    fn test_file_event_equality() {
        let path1 = PathBuf::from("/tmp/test.txt");
        let path2 = PathBuf::from("/tmp/test.txt");
        let path3 = PathBuf::from("/tmp/other.txt");

        assert_eq!(FileEvent::Created(path1.clone()), FileEvent::Created(path2));
        assert_ne!(
            FileEvent::Created(path1.clone()),
            FileEvent::Created(path3.clone())
        );
        assert_ne!(
            FileEvent::Created(path1.clone()),
            FileEvent::Modified(path1.clone())
        );
    }

    #[test]
    fn test_watcher_creation() {
        let config = WatcherConfig::default();
        let watcher = FileWatcher::new(config);
        assert!(watcher.watcher.is_none());
    }

    #[test]
    fn test_watcher_disabled_start() {
        let config = WatcherConfig::default();
        let mut watcher = FileWatcher::new(config);
        let result = watcher.start();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "File watcher is disabled");
    }

    #[test]
    fn test_watcher_config_serialization() {
        let config = WatcherConfig {
            enabled: true,
            paths: vec![PathBuf::from("/tmp/test")],
            ignore_patterns: vec![".git/".to_string()],
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: WatcherConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.enabled, true);
        assert_eq!(deserialized.paths.len(), 1);
        assert_eq!(deserialized.ignore_patterns.len(), 1);
    }

    #[tokio::test]
    async fn test_watcher_stop() {
        let config = WatcherConfig::default();
        let mut watcher = FileWatcher::new(config);
        assert!(watcher.stop().is_ok());
    }
}
