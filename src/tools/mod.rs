pub mod bash;
pub mod custom;
pub mod edit;
pub mod file;
pub mod git;
pub mod glob;
pub mod grep;
pub mod list;
pub mod patch;
pub mod question;
pub mod read;
pub mod shell;
pub mod skill;
pub mod subagent;
pub mod todo;
pub mod traits;
pub mod webfetch;
pub mod websearch;
pub mod write;

use std::path::PathBuf;

use file::FileTool;
use git::GitTool;
use shell::ShellTool;

pub use traits::{Tool, ToolOutput, ToolRegistry};

pub fn create_tool_registry(working_dir: PathBuf) -> ToolRegistry {
    create_tool_registry_with_custom(working_dir, &std::collections::HashMap::new())
}

pub fn create_tool_registry_with_custom(
    working_dir: PathBuf,
    custom_tools: &std::collections::HashMap<String, crate::config::CustomToolConfig>,
) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(bash::BashTool::new(working_dir.clone())));
    registry.register(Box::new(read::ReadTool::new(working_dir.clone())));
    registry.register(Box::new(write::WriteTool::new(working_dir.clone())));
    registry.register(Box::new(edit::EditTool::new(working_dir.clone())));
    registry.register(Box::new(list::ListTool::new(working_dir.clone())));
    registry.register(Box::new(glob::GlobTool::new(working_dir.clone())));
    registry.register(Box::new(grep::GrepTool::new(working_dir.clone())));
    registry.register(Box::new(patch::PatchTool::new(working_dir.clone())));
    registry.register(Box::new(skill::SkillTool::new(working_dir.clone())));
    registry.register(Box::new(question::QuestionTool::new()));
    registry.register(Box::new(webfetch::WebFetchTool::new()));
    registry.register(Box::new(websearch::WebSearchTool::new()));
    registry.register(Box::new(subagent::SubagentTool::new()));
    let (todo_write, todo_read) = todo::create_todo_tools();
    registry.register(Box::new(todo_write));
    registry.register(Box::new(todo_read));

    for (name, cfg) in custom_tools {
        if cfg.enabled {
            registry.register(Box::new(custom::CustomTool::new(
                name.clone(),
                cfg.clone(),
                working_dir.clone(),
            )));
        }
    }

    registry
}

pub struct ToolSet {
    pub file: FileTool,
    pub shell: ShellTool,
    pub git: GitTool,
}

impl ToolSet {
    pub fn new(working_dir: PathBuf) -> Self {
        ToolSet {
            file: FileTool::new(working_dir.clone()),
            shell: ShellTool::new(working_dir.clone()),
            git: GitTool::new(working_dir),
        }
    }
}

/// Format a list of available tool descriptions for the AI system prompt
pub fn tools_description() -> &'static str {
    r#"You have access to the following tools. Use them when appropriate:

## File Tools
- `read_file(path)`: Read the contents of a file
- `write_file(path, content)`: Write content to a file (creates if not exists)
- `list_directory(path)`: List contents of a directory
- `search_files(pattern, path)`: Search for a regex pattern in files recursively
- `apply_patch(path, old_content, new_content)`: Replace specific content in a file

## Shell Tools
- `run_command(command)`: Execute a shell command and return output

## Git Tools
- `git_status()`: Show working tree status
- `git_diff(staged)`: Show changes (pass staged=true for staged changes)
- `git_log(count)`: Show recent commit history
- `git_add(files)`: Stage files for commit
- `git_commit(message)`: Create a commit with the given message
- `git_branch_list()`: List all branches

When you need to use a tool, describe what you're doing and show the results clearly.
Always ask for confirmation before making destructive changes (deleting files, force operations, etc.)."#
}
