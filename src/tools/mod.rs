pub mod file;
pub mod git;
pub mod shell;

use std::path::PathBuf;

use file::FileTool;
use git::GitTool;
use shell::ShellTool;

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
