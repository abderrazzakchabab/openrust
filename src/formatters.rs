use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use tracing;

/// Configuration for file formatters
#[derive(Debug, Clone)]
pub struct FormatterConfig {
    pub enabled: bool,
    pub formatters: HashMap<String, String>,
}

impl Default for FormatterConfig {
    fn default() -> Self {
        let mut formatters = HashMap::new();
        formatters.insert("rs".to_string(), "rustfmt".to_string());
        formatters.insert("js".to_string(), "prettier --write".to_string());
        formatters.insert("ts".to_string(), "prettier --write".to_string());
        formatters.insert("tsx".to_string(), "prettier --write".to_string());
        formatters.insert("jsx".to_string(), "prettier --write".to_string());
        formatters.insert("py".to_string(), "black".to_string());
        formatters.insert("go".to_string(), "gofmt -w".to_string());
        Self {
            enabled: true,
            formatters,
        }
    }
}

/// Format a file using the configured formatter for its extension
pub fn format_file(path: &Path, config: &FormatterConfig) -> Result<Option<String>> {
    if !config.enabled {
        return Ok(None);
    }

    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let formatter_cmd = match config.formatters.get(extension) {
        Some(cmd) => cmd,
        None => return Ok(None), // No formatter for this extension
    };

    // Check if formatter is available
    let cmd_name = formatter_cmd.split_whitespace().next().unwrap_or("");
    if !is_command_available(cmd_name) {
        tracing::debug!("Formatter '{}' not found, skipping", cmd_name);
        return Ok(None);
    }

    // Build command with file path appended
    let full_cmd = format!("{} {}", formatter_cmd, path.display());
    let parts: Vec<&str> = full_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let output = Command::new(parts[0]).args(&parts[1..]).output()?;

    if output.status.success() {
        Ok(Some(format!("Formatted {}", path.display())))
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("Formatter failed for {}: {}", path.display(), stderr);
        Ok(None) // Don't fail on formatter errors
    }
}

fn is_command_available(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_format_file_unknown_extension() {
        let config = FormatterConfig::default();
        let path = PathBuf::from("test.unknown");
        let result = format_file(&path, &config).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_format_file_disabled() {
        let mut config = FormatterConfig::default();
        config.enabled = false;
        let path = PathBuf::from("test.rs");
        let result = format_file(&path, &config).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_default_formatters() {
        let config = FormatterConfig::default();
        assert!(config.formatters.contains_key("rs"));
        assert!(config.formatters.contains_key("js"));
        assert!(config.formatters.contains_key("ts"));
        assert!(config.formatters.contains_key("tsx"));
        assert!(config.formatters.contains_key("jsx"));
        assert!(config.formatters.contains_key("py"));
        assert!(config.formatters.contains_key("go"));

        assert_eq!(config.formatters.get("rs").unwrap(), "rustfmt");
        assert_eq!(config.formatters.get("js").unwrap(), "prettier --write");
        assert_eq!(config.formatters.get("py").unwrap(), "black");
        assert_eq!(config.formatters.get("go").unwrap(), "gofmt -w");
    }

    #[test]
    fn test_formatter_config_default() {
        let config = FormatterConfig::default();
        assert!(config.enabled);
        assert!(!config.formatters.is_empty());
    }

    #[test]
    fn test_is_command_available() {
        // ls should be available on all Unix-like systems
        assert!(is_command_available("ls"));
        // This command should not exist
        assert!(!is_command_available("nonexistent_command_xyz_12345"));
    }
}
