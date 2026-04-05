use std::fs;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::traits::{Tool, ToolOutput};

pub struct SkillTool {
    working_dir: PathBuf,
}

impl SkillTool {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    fn skill_dir(&self) -> PathBuf {
        self.working_dir.join(".openrust").join("skills")
    }

    fn list_skills(&self) -> anyhow::Result<String> {
        let skill_dir = self.skill_dir();

        if !skill_dir.exists() {
            return Ok(
                "No skills available. Create skill files in .openrust/skills/ directory."
                    .to_string(),
            );
        }

        let mut skills = Vec::new();

        for entry in fs::read_dir(&skill_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Some(name) = path.file_stem() {
                    if let Some(name_str) = name.to_str() {
                        if path.extension().map_or(false, |ext| ext == "md") {
                            skills.push(name_str.to_string());
                        }
                    }
                }
            }
        }

        if skills.is_empty() {
            return Ok(
                "No skills available. Create skill files in .openrust/skills/ directory."
                    .to_string(),
            );
        }

        skills.sort();
        Ok(skills.join("\n"))
    }

    fn load_skill(&self, name: &str) -> anyhow::Result<ToolOutput> {
        let skill_path = self.skill_dir().join(format!("{}.md", name));

        match fs::read_to_string(&skill_path) {
            Ok(content) => Ok(ToolOutput::success(content)),
            Err(_) => {
                let available = self
                    .list_skills()
                    .unwrap_or_else(|_| "Unable to list skills".to_string());
                Ok(ToolOutput::error(format!(
                    "Skill '{}' not found. Available skills:\n{}",
                    name, available
                )))
            }
        }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "Load skill markdown files from .openrust/skills/ directory. If name is omitted, lists all available skills."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to load. If omitted, lists all available skills."
                }
            }
        })
    }

    async fn execute(&self, input: Value) -> anyhow::Result<ToolOutput> {
        match input.get("name").and_then(Value::as_str) {
            Some(name) => self.load_skill(name),
            None => Ok(ToolOutput::success(self.list_skills()?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::json;

    use super::*;

    fn create_temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time ok")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("openrust-skill-test-{unique}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn create_skill_file(skill_dir: &PathBuf, name: &str, content: &str) {
        fs::create_dir_all(skill_dir).expect("create skill dir");
        let skill_path = skill_dir.join(format!("{}.md", name));
        fs::write(skill_path, content).expect("write skill file");
    }

    #[tokio::test]
    async fn test_skill_load_existing() {
        let temp_dir = create_temp_dir();
        let skill_dir = temp_dir.join(".openrust").join("skills");
        create_skill_file(
            &skill_dir,
            "test_skill",
            "# Test Skill\n\nThis is a test skill.",
        );

        let tool = SkillTool::new(temp_dir);
        let output = tool
            .execute(json!({"name": "test_skill"}))
            .await
            .expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("# Test Skill"));
        assert!(output.content.contains("This is a test skill."));
    }

    #[tokio::test]
    async fn test_skill_list_available() {
        let temp_dir = create_temp_dir();
        let skill_dir = temp_dir.join(".openrust").join("skills");
        create_skill_file(&skill_dir, "skill_a", "Content A");
        create_skill_file(&skill_dir, "skill_b", "Content B");
        create_skill_file(&skill_dir, "skill_c", "Content C");

        let tool = SkillTool::new(temp_dir);
        let output = tool.execute(json!({})).await.expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("skill_a"));
        assert!(output.content.contains("skill_b"));
        assert!(output.content.contains("skill_c"));
    }

    #[tokio::test]
    async fn test_skill_missing() {
        let temp_dir = create_temp_dir();
        let skill_dir = temp_dir.join(".openrust").join("skills");
        create_skill_file(&skill_dir, "existing_skill", "Content");

        let tool = SkillTool::new(temp_dir);
        let output = tool
            .execute(json!({"name": "nonexistent"}))
            .await
            .expect("execute");

        assert!(output.is_error);
        assert!(output.content.contains("Skill 'nonexistent' not found"));
        assert!(output.content.contains("existing_skill"));
    }

    #[tokio::test]
    async fn test_skill_no_directory() {
        let temp_dir = create_temp_dir();

        let tool = SkillTool::new(temp_dir);
        let output = tool.execute(json!({})).await.expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("No skills available"));
    }

    #[tokio::test]
    async fn test_skill_empty_directory() {
        let temp_dir = create_temp_dir();
        let skill_dir = temp_dir.join(".openrust").join("skills");
        fs::create_dir_all(&skill_dir).expect("create skill dir");

        let tool = SkillTool::new(temp_dir);
        let output = tool.execute(json!({})).await.expect("execute");

        assert!(!output.is_error);
        assert!(output.content.contains("No skills available"));
    }
}
