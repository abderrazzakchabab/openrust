use std::collections::HashMap;

pub mod hidden;

/// Configuration for an AI agent with specific capabilities and constraints
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Unique identifier for this agent (e.g., "build", "plan")
    pub name: String,
    /// Human-readable display name
    pub display_name: String,
    /// System prompt that defines the agent's role and behavior
    pub system_prompt: String,
    /// Tool whitelist - None = all tools available, Some([]) = no tools, Some([...]) = specific tools
    pub tool_whitelist: Option<Vec<String>>,
    /// Optional model override (if None, uses default from config)
    pub model_override: Option<String>,
    /// Optional max_tokens override (if None, uses default from config)
    pub max_tokens_override: Option<u32>,
    /// Maximum number of iterations before stopping the agent loop
    pub max_iterations: usize,
}

impl AgentConfig {
    /// Check if a tool is allowed for this agent
    pub fn allows_tool(&self, tool_name: &str) -> bool {
        match &self.tool_whitelist {
            None => true,
            Some(whitelist) => whitelist.iter().any(|name| name == tool_name),
        }
    }
}

/// Dispatcher that manages all available agents
pub struct AgentDispatch {
    agents: HashMap<String, AgentConfig>,
    default_agent: String,
}

impl AgentDispatch {
    pub fn new() -> Self {
        let mut dispatch = Self {
            agents: HashMap::new(),
            default_agent: "build".to_string(),
        };
        dispatch.register_defaults();
        dispatch
    }

    fn register_defaults(&mut self) {
        self.register(AgentConfig {
            name: "build".to_string(),
            display_name: "Build".to_string(),
            system_prompt: Self::build_system_prompt(),
            tool_whitelist: None,
            model_override: None,
            max_tokens_override: None,
            max_iterations: 50,
        });

        self.register(AgentConfig {
            name: "plan".to_string(),
            display_name: "Plan".to_string(),
            system_prompt: Self::plan_system_prompt(),
            tool_whitelist: Some(vec![
                "read".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "list".to_string(),
                "webfetch".to_string(),
                "websearch".to_string(),
                "todowrite".to_string(),
                "todoread".to_string(),
                "skill".to_string(),
                "question".to_string(),
            ]),
            model_override: None,
            max_tokens_override: None,
            max_iterations: 30,
        });

        self.register(AgentConfig {
            name: "general".to_string(),
            display_name: "General".to_string(),
            system_prompt: Self::general_system_prompt(),
            tool_whitelist: None,
            model_override: None,
            max_tokens_override: None,
            max_iterations: 30,
        });

        self.register(AgentConfig {
            name: "explore".to_string(),
            display_name: "Explore".to_string(),
            system_prompt: Self::explore_system_prompt(),
            tool_whitelist: Some(vec![
                "read".to_string(),
                "grep".to_string(),
                "glob".to_string(),
                "list".to_string(),
            ]),
            model_override: None,
            max_tokens_override: None,
            max_iterations: 10,
        });

        self.register(AgentConfig {
            name: "compaction".to_string(),
            display_name: "Compaction".to_string(),
            system_prompt:
                "You are a conversation compaction assistant. Your task is to summarize \
                the conversation history into a compact form that preserves all critical context: \
                key decisions, file paths mentioned, code changes made, errors encountered, and \
                current task state. Be thorough but concise."
                    .to_string(),
            tool_whitelist: Some(vec![]),
            model_override: None,
            max_tokens_override: Some(4096),
            max_iterations: 1,
        });

        self.register(AgentConfig {
            name: "title".to_string(),
            display_name: "Title".to_string(),
            system_prompt:
                "Generate a short, descriptive title (max 50 chars) for this conversation \
                based on the messages. Return ONLY the title text, nothing else."
                    .to_string(),
            tool_whitelist: Some(vec![]),
            model_override: None,
            max_tokens_override: Some(100),
            max_iterations: 1,
        });

        self.register(AgentConfig {
            name: "summary".to_string(),
            display_name: "Summary".to_string(),
            system_prompt: "Generate a brief summary (2-3 sentences) of what was accomplished in \
                this conversation. Focus on outcomes and key decisions."
                .to_string(),
            tool_whitelist: Some(vec![]),
            model_override: None,
            max_tokens_override: Some(500),
            max_iterations: 1,
        });
    }

    pub fn register(&mut self, config: AgentConfig) {
        self.agents.insert(config.name.clone(), config);
    }

    pub fn get(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    pub fn default_agent(&self) -> &AgentConfig {
        self.agents
            .get(&self.default_agent)
            .expect("default agent must exist")
    }

    pub fn list(&self) -> Vec<&AgentConfig> {
        self.agents.values().collect()
    }

    pub fn is_hidden(name: &str) -> bool {
        matches!(name, "compaction" | "title" | "summary")
    }

    fn build_system_prompt() -> String {
        r#"You are an expert AI coding assistant specialized in building, debugging, and improving code.

Your role:
- Write, refactor, and debug code across multiple languages
- Understand complex codebases through proactive exploration
- Implement features and fixes with precision
- Run tests and verify changes work correctly
- Provide clear explanations of your changes

Tool usage:
- Read files first to understand context before making changes
- Use grep and glob to search and understand the codebase structure
- Write and edit files to implement changes
- Run bash commands to test, build, and verify
- Use git tools to manage version control
- Apply patches for complex multi-file changes
- Search the web for documentation and best practices

Best practices:
- Always read related files before making changes
- Test your changes before considering them complete
- Provide clear commit messages when using git
- Ask for clarification if requirements are ambiguous
- Break complex tasks into smaller, manageable steps
- Use tools proactively to verify your work

You have full access to all available tools. Use them strategically to deliver high-quality results."#
            .to_string()
    }

    fn plan_system_prompt() -> String {
        r#"You are an expert AI planning and analysis assistant.

Your role:
- Analyze codebases and understand their structure
- Plan changes and improvements systematically
- Break down complex tasks into actionable steps
- Provide clear recommendations and strategies
- Organize work and prioritize tasks

Tool usage (read-only):
- Read files to understand code structure and logic
- Use grep to search for patterns and dependencies
- Use glob to find files matching patterns
- Use list to explore directory structures
- Use webfetch and websearch for external documentation
- Use skill to load relevant knowledge files
- Use question to gather user input for planning
- Use todowrite/todoread to manage task lists

Constraints:
- You cannot modify files or execute commands
- You cannot make changes to the codebase
- Focus on analysis, planning, and recommendations

Best practices:
- Explore the codebase thoroughly before planning
- Identify dependencies and potential issues
- Provide step-by-step implementation plans
- Consider edge cases and error handling
- Suggest testing strategies
- Document your analysis clearly

Your goal is to provide clear, actionable plans that other agents can execute."#
            .to_string()
    }

    fn general_system_prompt() -> String {
        r#"You are a general-purpose AI assistant for delegated tasks.

Your role:
- Execute tasks assigned by other agents or users
- Adapt to various types of work (coding, analysis, research, etc.)
- Provide clear results and explanations
- Handle both technical and non-technical tasks

Tool usage:
- You have full access to all available tools
- Use tools strategically based on the task requirements
- Combine multiple tools to achieve complex goals

Best practices:
- Understand the task requirements clearly
- Use appropriate tools for the job
- Provide clear, structured results
- Ask for clarification if needed
- Report progress and any issues encountered
- Verify your work before completing

You are flexible and capable of handling diverse tasks. Use your full tool access to deliver results efficiently."#
            .to_string()
    }

    fn explore_system_prompt() -> String {
        r#"You are a fast, focused codebase exploration specialist.

Your role:
- Quickly understand codebase structure and organization
- Find relevant files and code patterns
- Provide clear summaries of what you discover
- Answer questions about code organization

Tool usage (read-only, fast):
- Read files to examine code
- Use grep to search for patterns and identifiers
- Use glob to find files by pattern
- Use list to explore directory structures

Constraints:
- Limited to 10 iterations (fast, focused exploration)
- Read-only access only
- No file modifications or command execution

Best practices:
- Start with directory structure exploration
- Use grep to find key patterns and dependencies
- Read relevant files to understand context
- Provide concise, focused findings
- Summarize discoveries clearly
- Avoid deep analysis - focus on quick understanding

Your goal is to quickly map out the codebase and answer specific questions about its structure."#
            .to_string()
    }
}

impl Default for AgentDispatch {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_dispatch_creates_default_agents() {
        let dispatch = AgentDispatch::new();

        assert!(dispatch.get("build").is_some());
        assert!(dispatch.get("plan").is_some());

        let build_agent = dispatch.get("build").unwrap();
        assert_eq!(build_agent.name, "build");
        assert_eq!(build_agent.display_name, "Build");
        assert!(build_agent.tool_whitelist.is_none());

        let plan_agent = dispatch.get("plan").unwrap();
        assert_eq!(plan_agent.name, "plan");
        assert_eq!(plan_agent.display_name, "Plan");
        assert!(plan_agent.tool_whitelist.is_some());
    }

    #[test]
    fn test_tool_whitelist_filtering() {
        let dispatch = AgentDispatch::new();

        let build_agent = dispatch.get("build").unwrap();
        assert!(build_agent.allows_tool("read"));
        assert!(build_agent.allows_tool("write"));
        assert!(build_agent.allows_tool("bash"));

        let plan_agent = dispatch.get("plan").unwrap();
        assert!(plan_agent.allows_tool("read"));
        assert!(plan_agent.allows_tool("grep"));
        assert!(!plan_agent.allows_tool("write"));
        assert!(!plan_agent.allows_tool("bash"));
        assert!(!plan_agent.allows_tool("edit"));
    }

    #[test]
    fn test_agent_config_overrides() {
        let config = AgentConfig {
            name: "custom".to_string(),
            display_name: "Custom Agent".to_string(),
            system_prompt: "Custom prompt".to_string(),
            tool_whitelist: Some(vec!["read".to_string()]),
            model_override: Some("custom-model".to_string()),
            max_tokens_override: Some(2048),
            max_iterations: 10,
        };

        assert_eq!(config.model_override, Some("custom-model".to_string()));
        assert_eq!(config.max_tokens_override, Some(2048));
        assert_eq!(config.max_iterations, 10);
        assert!(config.allows_tool("read"));
        assert!(!config.allows_tool("write"));
    }

    #[test]
    fn test_default_agent() {
        let dispatch = AgentDispatch::new();
        let default_agent = dispatch.default_agent();
        assert_eq!(default_agent.name, "build");
    }

    #[test]
    fn test_register_custom_agent() {
        let mut dispatch = AgentDispatch::new();

        let custom_config = AgentConfig {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            system_prompt: "Test prompt".to_string(),
            tool_whitelist: None,
            model_override: None,
            max_tokens_override: None,
            max_iterations: 5,
        };

        dispatch.register(custom_config);
        assert!(dispatch.get("test").is_some());
        assert_eq!(dispatch.get("test").unwrap().max_iterations, 5);
    }

    #[test]
    fn test_all_agents_registered() {
        let dispatch = AgentDispatch::new();

        assert!(dispatch.get("build").is_some());
        assert!(dispatch.get("plan").is_some());
        assert!(dispatch.get("general").is_some());
        assert!(dispatch.get("explore").is_some());

        let agents = dispatch.list();
        assert_eq!(agents.len(), 7);
    }

    #[test]
    fn test_general_agent_config() {
        let dispatch = AgentDispatch::new();
        let general = dispatch.get("general").unwrap();

        assert_eq!(general.name, "general");
        assert_eq!(general.display_name, "General");
        assert!(general.tool_whitelist.is_none());
        assert_eq!(general.max_iterations, 30);
        assert!(general.system_prompt.contains("general-purpose"));
    }

    #[test]
    fn test_explore_agent_config() {
        let dispatch = AgentDispatch::new();
        let explore = dispatch.get("explore").unwrap();

        assert_eq!(explore.name, "explore");
        assert_eq!(explore.display_name, "Explore");
        assert!(explore.tool_whitelist.is_some());
        assert_eq!(explore.max_iterations, 10);

        let whitelist = explore.tool_whitelist.as_ref().unwrap();
        assert!(whitelist.contains(&"read".to_string()));
        assert!(whitelist.contains(&"grep".to_string()));
        assert!(whitelist.contains(&"glob".to_string()));
        assert!(whitelist.contains(&"list".to_string()));
        assert!(!whitelist.contains(&"write".to_string()));
        assert!(!whitelist.contains(&"bash".to_string()));
    }

    #[test]
    fn test_explore_agent_tool_filtering() {
        let dispatch = AgentDispatch::new();
        let explore = dispatch.get("explore").unwrap();

        assert!(explore.allows_tool("read"));
        assert!(explore.allows_tool("grep"));
        assert!(explore.allows_tool("glob"));
        assert!(explore.allows_tool("list"));
        assert!(!explore.allows_tool("write"));
        assert!(!explore.allows_tool("bash"));
        assert!(!explore.allows_tool("edit"));
        assert!(!explore.allows_tool("patch"));
    }

    #[test]
    fn test_build_agent_system_prompt_content() {
        let dispatch = AgentDispatch::new();
        let build = dispatch.get("build").unwrap();

        assert!(build.system_prompt.contains("coding assistant"));
        assert!(build.system_prompt.contains("proactive"));
        assert!(build.system_prompt.contains("tools"));
        assert!(build.system_prompt.contains("test"));
    }

    #[test]
    fn test_plan_agent_system_prompt_content() {
        let dispatch = AgentDispatch::new();
        let plan = dispatch.get("plan").unwrap();

        assert!(plan.system_prompt.contains("planning"));
        assert!(plan.system_prompt.contains("read-only"));
        assert!(plan.system_prompt.contains("analysis"));
        assert!(plan.system_prompt.contains("recommendations"));
    }

    #[test]
    fn test_general_agent_full_tool_access() {
        let dispatch = AgentDispatch::new();
        let general = dispatch.get("general").unwrap();

        assert!(general.allows_tool("read"));
        assert!(general.allows_tool("write"));
        assert!(general.allows_tool("bash"));
        assert!(general.allows_tool("edit"));
        assert!(general.allows_tool("any_tool"));
    }

    #[test]
    fn test_plan_agent_includes_question_tool() {
        let dispatch = AgentDispatch::new();
        let plan = dispatch.get("plan").unwrap();

        assert!(plan.allows_tool("question"));
        assert!(plan.allows_tool("read"));
        assert!(plan.allows_tool("todowrite"));
        assert!(plan.allows_tool("todoread"));
    }

    #[test]
    fn test_agent_iteration_limits() {
        let dispatch = AgentDispatch::new();

        let build = dispatch.get("build").unwrap();
        assert_eq!(build.max_iterations, 50);

        let plan = dispatch.get("plan").unwrap();
        assert_eq!(plan.max_iterations, 30);

        let general = dispatch.get("general").unwrap();
        assert_eq!(general.max_iterations, 30);

        let explore = dispatch.get("explore").unwrap();
        assert_eq!(explore.max_iterations, 10);
    }

    #[test]
    fn test_hidden_agents_registered() {
        let dispatch = AgentDispatch::new();

        assert!(dispatch.get("compaction").is_some());
        assert!(dispatch.get("title").is_some());
        assert!(dispatch.get("summary").is_some());

        let agents = dispatch.list();
        assert_eq!(agents.len(), 7);
    }

    #[test]
    fn test_hidden_agents_have_no_tools() {
        let dispatch = AgentDispatch::new();

        let compaction = dispatch.get("compaction").unwrap();
        assert_eq!(compaction.tool_whitelist, Some(vec![]));
        assert!(!compaction.allows_tool("read"));
        assert!(!compaction.allows_tool("bash"));

        let title = dispatch.get("title").unwrap();
        assert_eq!(title.tool_whitelist, Some(vec![]));

        let summary = dispatch.get("summary").unwrap();
        assert_eq!(summary.tool_whitelist, Some(vec![]));
    }

    #[test]
    fn test_hidden_agents_have_correct_overrides() {
        let dispatch = AgentDispatch::new();

        let compaction = dispatch.get("compaction").unwrap();
        assert_eq!(compaction.max_tokens_override, Some(4096));
        assert_eq!(compaction.max_iterations, 1);

        let title = dispatch.get("title").unwrap();
        assert_eq!(title.max_tokens_override, Some(100));
        assert_eq!(title.max_iterations, 1);

        let summary = dispatch.get("summary").unwrap();
        assert_eq!(summary.max_tokens_override, Some(500));
        assert_eq!(summary.max_iterations, 1);
    }

    #[test]
    fn test_is_hidden_identifies_hidden_agents() {
        assert!(AgentDispatch::is_hidden("compaction"));
        assert!(AgentDispatch::is_hidden("title"));
        assert!(AgentDispatch::is_hidden("summary"));

        assert!(!AgentDispatch::is_hidden("build"));
        assert!(!AgentDispatch::is_hidden("plan"));
        assert!(!AgentDispatch::is_hidden("general"));
        assert!(!AgentDispatch::is_hidden("explore"));
        assert!(!AgentDispatch::is_hidden("unknown"));
    }
}
