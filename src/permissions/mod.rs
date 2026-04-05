use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::PermissionsConfig;

/// Permission level for a tool or operation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionLevel {
    Allow,
    Ask,
    Deny,
}

/// Result of a permission check
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionResult {
    Allowed,
    NeedsApproval {
        tool_name: String,
        description: String,
    },
    Denied {
        reason: String,
    },
}

/// Permission checker for tools and operations
pub struct PermissionChecker {
    allow_all: bool,
    tool_permissions: HashMap<String, PermissionLevel>,
    bash_allow_patterns: Vec<String>,
    bash_deny_patterns: Vec<String>,
    session_allowed: HashSet<String>,
    agent_overrides: HashMap<String, HashMap<String, PermissionLevel>>,
    allowed_directories: Vec<PathBuf>,
    denied_paths: Vec<String>,
}

impl PermissionChecker {
    /// Create a new permission checker from configuration
    pub fn new(config: &PermissionsConfig) -> Self {
        let mut tool_permissions = HashMap::new();

        for tool_name in &config.allow {
            tool_permissions.insert(tool_name.clone(), PermissionLevel::Allow);
        }

        for tool_name in &config.ask {
            tool_permissions.insert(tool_name.clone(), PermissionLevel::Ask);
        }

        for tool_name in &config.deny {
            tool_permissions.insert(tool_name.clone(), PermissionLevel::Deny);
        }

        let bash_allow_patterns = if config.bash_allow_patterns.is_empty() {
            vec![
                "git *".to_string(),
                "cargo *".to_string(),
                "ls *".to_string(),
                "cat *".to_string(),
                "echo *".to_string(),
            ]
        } else {
            config.bash_allow_patterns.clone()
        };

        let bash_deny_patterns = if config.bash_deny_patterns.is_empty() {
            vec!["rm -rf /*".to_string(), "sudo *".to_string()]
        } else {
            config.bash_deny_patterns.clone()
        };

        let denied_paths = vec![
            "~/.ssh/*".to_string(),
            "~/.env".to_string(),
            "~/.aws/*".to_string(),
            "~/.gnupg/*".to_string(),
            "*/.git/config".to_string(),
        ];

        PermissionChecker {
            allow_all: false,
            tool_permissions,
            bash_allow_patterns,
            bash_deny_patterns,
            session_allowed: HashSet::new(),
            agent_overrides: HashMap::new(),
            allowed_directories: Vec::new(),
            denied_paths,
        }
    }

    /// Create a permission checker that allows all operations (for CLI mode)
    pub fn new_allow_all() -> Self {
        PermissionChecker {
            allow_all: true,
            tool_permissions: HashMap::new(),
            bash_allow_patterns: vec!["*".to_string()],
            bash_deny_patterns: Vec::new(),
            session_allowed: HashSet::new(),
            agent_overrides: HashMap::new(),
            allowed_directories: Vec::new(),
            denied_paths: Vec::new(),
        }
    }

    /// Check if a tool execution is allowed
    pub fn check_tool(&self, tool_name: &str, input: &serde_json::Value) -> PermissionResult {
        if self.allow_all {
            return PermissionResult::Allowed;
        }

        if tool_name == "bash" {
            if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                let command_key = format!("bash:{}", command);

                if self.session_allowed.contains(&command_key) {
                    return PermissionResult::Allowed;
                }

                return self.check_bash_command(command);
            }
        }

        let tool_key = format!("{}:{}", tool_name, input.to_string());
        if self.session_allowed.contains(&tool_key) {
            return PermissionResult::Allowed;
        }

        match self.tool_permissions.get(tool_name) {
            Some(PermissionLevel::Allow) => PermissionResult::Allowed,
            Some(PermissionLevel::Deny) => PermissionResult::Denied {
                reason: format!("Tool '{}' is denied by configuration", tool_name),
            },
            Some(PermissionLevel::Ask) | None => {
                let description = if tool_name == "bash" {
                    if let Some(command) = input.get("command").and_then(|v| v.as_str()) {
                        format!("Execute bash command: {}", command)
                    } else {
                        format!("Execute tool: {}", tool_name)
                    }
                } else {
                    format!("Execute tool: {} with input: {}", tool_name, input)
                };

                PermissionResult::NeedsApproval {
                    tool_name: tool_name.to_string(),
                    description,
                }
            }
        }
    }

    /// Check if a bash command is allowed
    fn check_bash_command(&self, command: &str) -> PermissionResult {
        for pattern in &self.bash_deny_patterns {
            if glob_match(pattern, command) {
                return PermissionResult::Denied {
                    reason: format!("Command '{}' matches deny pattern '{}'", command, pattern),
                };
            }
        }

        for pattern in &self.bash_allow_patterns {
            if glob_match(pattern, command) {
                return PermissionResult::Allowed;
            }
        }

        PermissionResult::NeedsApproval {
            tool_name: "bash".to_string(),
            description: format!("Execute bash command: {}", command),
        }
    }

    /// Remember a permission decision for the session
    pub fn remember_allow(&mut self, tool_name: &str, command_key: &str) {
        let key = format!("{}:{}", tool_name, command_key);
        self.session_allowed.insert(key);
    }

    /// Set allowed directories for file operations (default: working directory)
    pub fn set_allowed_directories(&mut self, directories: Vec<PathBuf>) {
        self.allowed_directories = directories;
    }

    /// Add an agent-specific permission override
    pub fn add_agent_override(
        &mut self,
        agent_name: String,
        tool_name: String,
        level: PermissionLevel,
    ) {
        self.agent_overrides
            .entry(agent_name)
            .or_insert_with(HashMap::new)
            .insert(tool_name, level);
    }

    /// Check if a tool execution is allowed for a specific agent
    pub fn check_tool_for_agent(
        &self,
        agent_name: &str,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> PermissionResult {
        if self.allow_all {
            return PermissionResult::Allowed;
        }

        // Check agent-specific overrides first
        if let Some(agent_perms) = self.agent_overrides.get(agent_name) {
            if let Some(level) = agent_perms.get(tool_name) {
                return match level {
                    PermissionLevel::Allow => PermissionResult::Allowed,
                    PermissionLevel::Deny => PermissionResult::Denied {
                        reason: format!(
                            "Tool '{}' is denied for agent '{}' by override",
                            tool_name, agent_name
                        ),
                    },
                    PermissionLevel::Ask => {
                        let description =
                            format!("Execute tool: {} with input: {}", tool_name, input);
                        PermissionResult::NeedsApproval {
                            tool_name: tool_name.to_string(),
                            description,
                        }
                    }
                };
            }
        }

        // Fall back to global permissions
        self.check_tool(tool_name, input)
    }

    /// Check if a file path is allowed for access
    pub fn check_path_access(&self, path: &str) -> PermissionResult {
        let path_buf = PathBuf::from(path);

        // Expand ~ to home directory
        let expanded_path = if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(path.strip_prefix("~/").unwrap_or(&path[1..]))
            } else {
                path_buf.clone()
            }
        } else {
            path_buf.clone()
        };

        // Check denied paths first
        for pattern in &self.denied_paths {
            let expanded_pattern = if pattern.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    home.join(pattern.strip_prefix("~/").unwrap_or(&pattern[1..]))
                        .to_string_lossy()
                        .to_string()
                } else {
                    pattern.clone()
                }
            } else {
                pattern.clone()
            };

            if glob_match(&expanded_pattern, &expanded_path.to_string_lossy()) {
                return PermissionResult::Denied {
                    reason: format!(
                        "Access to '{}' is denied (matches sensitive path pattern '{}')",
                        path, pattern
                    ),
                };
            }
        }

        // If no allowed directories configured, allow all (except denied)
        if self.allowed_directories.is_empty() {
            return PermissionResult::Allowed;
        }

        // Check if path is within allowed directories
        for allowed_dir in &self.allowed_directories {
            let canonical_path = expanded_path.canonicalize().ok();
            let canonical_allowed = allowed_dir.canonicalize().ok();

            if let (Some(ref cp), Some(ref ca)) = (canonical_path, canonical_allowed) {
                if cp.starts_with(ca) {
                    return PermissionResult::Allowed;
                }
            }

            if expanded_path.starts_with(allowed_dir) {
                return PermissionResult::Allowed;
            }
        }

        PermissionResult::Denied {
            reason: format!(
                "Access to '{}' is denied (outside allowed directories)",
                path
            ),
        }
    }
}

/// Simple glob pattern matching
/// Supports * (matches any sequence of characters) and ? (matches single character)
fn glob_match(pattern: &str, text: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    let mut text_chars = text.chars().peekable();

    while let Some(&p) = pattern_chars.peek() {
        match p {
            '*' => {
                pattern_chars.next();
                // If * is at the end of pattern, match everything remaining
                if pattern_chars.peek().is_none() {
                    return true;
                }

                // Try to match the rest of the pattern at different positions
                let remaining_pattern: String = pattern_chars.clone().collect();
                let mut text_clone = text_chars.clone();

                // Try matching at current position first (empty match for *)
                if glob_match(&remaining_pattern, &text_clone.collect::<String>()) {
                    return true;
                }

                // Try consuming characters from text
                while text_chars.peek().is_some() {
                    text_chars.next();
                    text_clone = text_chars.clone();
                    if glob_match(&remaining_pattern, &text_clone.collect::<String>()) {
                        return true;
                    }
                }

                return false;
            }
            '?' => {
                pattern_chars.next();
                if text_chars.next().is_none() {
                    return false;
                }
            }
            _ => {
                pattern_chars.next();
                if text_chars.next() != Some(p) {
                    return false;
                }
            }
        }
    }

    // Both should be exhausted for a match
    text_chars.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_allow_permission() {
        let config = PermissionsConfig {
            allow: vec!["read".to_string()],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("read", &json!({"path": "/test"}));

        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_deny_permission() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec!["write".to_string()],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("write", &json!({"path": "/test"}));

        match result {
            PermissionResult::Denied { reason } => {
                assert!(reason.contains("write"));
                assert!(reason.contains("denied"));
            }
            _ => panic!("Expected Denied result"),
        }
    }

    #[test]
    fn test_ask_permission() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec!["bash".to_string()],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("bash", &json!({"command": "ls"}));

        match result {
            PermissionResult::NeedsApproval { tool_name, .. } => {
                assert_eq!(tool_name, "bash");
            }
            _ => panic!("Expected NeedsApproval result"),
        }
    }

    #[test]
    fn test_default_is_ask() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("unknown_tool", &json!({}));

        match result {
            PermissionResult::NeedsApproval { tool_name, .. } => {
                assert_eq!(tool_name, "unknown_tool");
            }
            _ => panic!("Expected NeedsApproval result for unconfigured tool"),
        }
    }

    #[test]
    fn test_bash_allow_pattern() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec!["git *".to_string()],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("bash", &json!({"command": "git status"}));

        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_bash_deny_pattern() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec!["rm -rf *".to_string()],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("bash", &json!({"command": "rm -rf /"}));

        match result {
            PermissionResult::Denied { reason } => {
                assert!(reason.contains("rm -rf"));
                assert!(reason.contains("deny pattern"));
            }
            _ => panic!("Expected Denied result"),
        }
    }

    #[test]
    fn test_deny_takes_priority() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec!["rm *".to_string()],
            bash_deny_patterns: vec!["rm -rf *".to_string()],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool("bash", &json!({"command": "rm -rf /tmp"}));

        match result {
            PermissionResult::Denied { .. } => {}
            _ => panic!("Expected Denied result - deny should take priority over allow"),
        }
    }

    #[test]
    fn test_session_remember() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec!["bash".to_string()],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let mut checker = PermissionChecker::new(&config);

        // First check should need approval
        let result = checker.check_tool("bash", &json!({"command": "ls"}));
        match result {
            PermissionResult::NeedsApproval { .. } => {}
            _ => panic!("Expected NeedsApproval on first check"),
        }

        // Remember the decision
        checker.remember_allow("bash", "ls");

        // Second check should be allowed
        let result = checker.check_tool("bash", &json!({"command": "ls"}));
        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("git *", "git status"));
        assert!(glob_match("git *", "git commit -m test"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*.txt", "file.txt"));
        assert!(!glob_match("*.txt", "file.rs"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("?.txt", "a.txt"));
        assert!(!glob_match("?.txt", "ab.txt"));
        assert!(glob_match("test?", "test1"));
        assert!(glob_match("test?", "testa"));
        assert!(!glob_match("test?", "test12"));
    }

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("exact match", "exact match"));
        assert!(!glob_match("exact match", "not exact match"));
        assert!(glob_match("git status", "git status"));
        assert!(!glob_match("git status", "git commit"));
    }

    #[test]
    fn test_agent_override_allows() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec!["write".to_string()],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let mut checker = PermissionChecker::new(&config);
        checker.add_agent_override(
            "test_agent".to_string(),
            "write".to_string(),
            PermissionLevel::Allow,
        );

        let result = checker.check_tool_for_agent("test_agent", "write", &json!({"path": "/test"}));
        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_agent_override_denies() {
        let config = PermissionsConfig {
            allow: vec!["read".to_string()],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let mut checker = PermissionChecker::new(&config);
        checker.add_agent_override(
            "restricted_agent".to_string(),
            "read".to_string(),
            PermissionLevel::Deny,
        );

        let result =
            checker.check_tool_for_agent("restricted_agent", "read", &json!({"path": "/test"}));
        match result {
            PermissionResult::Denied { reason } => {
                assert!(reason.contains("restricted_agent"));
                assert!(reason.contains("read"));
            }
            _ => panic!("Expected Denied result"),
        }
    }

    #[test]
    fn test_agent_fallback_to_global() {
        let config = PermissionsConfig {
            allow: vec!["read".to_string()],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);
        let result = checker.check_tool_for_agent("any_agent", "read", &json!({"path": "/test"}));
        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_path_within_allowed() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let mut checker = PermissionChecker::new(&config);
        let working_dir = std::env::current_dir().expect("get current dir");
        checker.set_allowed_directories(vec![working_dir.clone()]);

        let test_path = working_dir.join("test.txt");
        let result = checker.check_path_access(&test_path.to_string_lossy());
        assert_eq!(result, PermissionResult::Allowed);
    }

    #[test]
    fn test_path_outside_allowed() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let mut checker = PermissionChecker::new(&config);
        let working_dir = std::env::current_dir().expect("get current dir");
        checker.set_allowed_directories(vec![working_dir]);

        let result = checker.check_path_access("/etc/passwd");
        match result {
            PermissionResult::Denied { reason } => {
                assert!(reason.contains("outside allowed directories"));
            }
            _ => panic!("Expected Denied result for path outside allowed directories"),
        }
    }

    #[test]
    fn test_sensitive_path_blocked() {
        let config = PermissionsConfig {
            allow: vec![],
            ask: vec![],
            deny: vec![],
            bash_allow_patterns: vec![],
            bash_deny_patterns: vec![],
        };

        let checker = PermissionChecker::new(&config);

        if let Some(home) = dirs::home_dir() {
            let ssh_key = home.join(".ssh/id_rsa");
            let result = checker.check_path_access(&ssh_key.to_string_lossy());
            match result {
                PermissionResult::Denied { reason } => {
                    assert!(reason.contains("sensitive path"));
                }
                _ => panic!("Expected Denied result for sensitive path"),
            }
        }
    }
}
