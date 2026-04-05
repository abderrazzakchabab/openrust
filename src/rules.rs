use std::fs;
use std::path::PathBuf;

use crate::config::RulesConfig;

pub fn default_project_rules_path(working_dir: &PathBuf) -> PathBuf {
    working_dir.join(".openrust").join("rules.md")
}

pub fn default_user_rules_path() -> Option<PathBuf> {
    dirs::config_dir().map(|p| p.join("openrust").join("rules.md"))
}

pub fn load_merged_rules(working_dir: &PathBuf, cfg: &RulesConfig) -> Option<String> {
    if !cfg.enabled {
        return None;
    }

    let mut sections: Vec<String> = Vec::new();

    let user_path = cfg
        .user_path
        .as_ref()
        .map(PathBuf::from)
        .or_else(default_user_rules_path);
    if let Some(path) = user_path {
        if let Some(content) = read_rules_file(&path) {
            sections.push(format!("## User Rules\n\n{}", content));
        }
    }

    let project_path = cfg
        .project_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_project_rules_path(working_dir));
    if let Some(content) = read_rules_file(&project_path) {
        sections.push(format!("## Project Rules\n\n{}", content));
    }

    if let Some(inline) = cfg
        .inline
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        sections.push(format!("## Inline Rules\n\n{}", inline));
    }

    if sections.is_empty() {
        None
    } else {
        Some(format!(
            "Follow these rules for this conversation:\n\n{}",
            sections.join("\n\n")
        ))
    }
}

pub fn prepend_rules(base_prompt: String, rules: Option<String>) -> String {
    match rules {
        Some(rules_text) if !rules_text.trim().is_empty() => {
            format!("{}\n\n---\n\n{}", rules_text, base_prompt)
        }
        _ => base_prompt,
    }
}

fn read_rules_file(path: &PathBuf) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let trimmed = content.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{load_merged_rules, prepend_rules, RulesConfig};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be valid")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{}-{}", prefix, unique));
        fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn prepend_rules_noop_when_missing() {
        let prompt = "base prompt".to_string();
        assert_eq!(prepend_rules(prompt.clone(), None), prompt);
    }

    #[test]
    fn load_merged_rules_respects_order() {
        let project_dir = create_temp_dir("openrust-rules-project");
        let user_dir = create_temp_dir("openrust-rules-user");

        let project_rules = project_dir.join(".openrust").join("rules.md");
        fs::create_dir_all(project_rules.parent().expect("parent exists"))
            .expect("project rules dir should be created");
        fs::write(&project_rules, "project rule").expect("project rules should be written");

        let user_rules = user_dir.join("rules.md");
        fs::write(&user_rules, "user rule").expect("user rules should be written");

        let cfg = RulesConfig {
            enabled: true,
            project_path: None,
            user_path: Some(user_rules.to_string_lossy().to_string()),
            inline: Some("inline rule".to_string()),
        };

        let merged = load_merged_rules(&project_dir, &cfg).expect("rules should be loaded");

        let user_pos = merged.find("user rule").expect("user rule should exist");
        let project_pos = merged
            .find("project rule")
            .expect("project rule should exist");
        let inline_pos = merged
            .find("inline rule")
            .expect("inline rule should exist");

        assert!(user_pos < project_pos);
        assert!(project_pos < inline_pos);
    }

    #[test]
    fn load_merged_rules_ignores_missing_files() {
        let project_dir = create_temp_dir("openrust-rules-missing");
        let cfg = RulesConfig {
            enabled: true,
            project_path: Some(
                project_dir
                    .join("missing-project.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            user_path: Some(
                project_dir
                    .join("missing-user.md")
                    .to_string_lossy()
                    .to_string(),
            ),
            inline: None,
        };

        assert!(load_merged_rules(&project_dir, &cfg).is_none());
    }
}
