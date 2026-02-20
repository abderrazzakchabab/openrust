use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub content: String,
    pub size: u64,
    pub lines: usize,
}

pub struct FileTool {
    working_dir: PathBuf,
}

impl FileTool {
    pub fn new(working_dir: PathBuf) -> Self {
        FileTool { working_dir }
    }

    fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }

    pub fn read_file(&self, path: &str) -> Result<FileInfo> {
        let full_path = self.resolve_path(path);
        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file: {}", full_path.display()))?;

        let metadata = std::fs::metadata(&full_path)
            .with_context(|| format!("Failed to get metadata for: {}", full_path.display()))?;

        let lines = content.lines().count();

        Ok(FileInfo {
            path: full_path,
            size: metadata.len(),
            lines,
            content,
        })
    }

    pub fn write_file(&self, path: &str, content: &str) -> Result<()> {
        let full_path = self.resolve_path(path);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }

        std::fs::write(&full_path, content)
            .with_context(|| format!("Failed to write file: {}", full_path.display()))?;

        Ok(())
    }

    pub fn list_directory(&self, path: &str) -> Result<Vec<DirEntry>> {
        let full_path = self.resolve_path(path);
        let mut entries: Vec<DirEntry> = std::fs::read_dir(&full_path)
            .with_context(|| format!("Failed to read directory: {}", full_path.display()))?
            .filter_map(|e| e.ok())
            .map(|e| {
                let is_dir = e.file_type().map_or(false, |t| t.is_dir());
                let size = e.metadata().map_or(0, |m| m.len());
                DirEntry {
                    name: e.file_name().to_string_lossy().to_string(),
                    path: e.path(),
                    is_dir,
                    size,
                }
            })
            .collect();

        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });

        Ok(entries)
    }

    pub fn search_files(&self, pattern: &str, path: &str) -> Result<Vec<SearchResult>> {
        let full_path = self.resolve_path(path);
        let regex = regex::Regex::new(pattern)
            .with_context(|| format!("Invalid search pattern: {}", pattern))?;

        let mut results = Vec::new();
        self.search_recursive(&full_path, &regex, &mut results)?;
        Ok(results)
    }

    fn search_recursive(
        &self,
        dir: &Path,
        pattern: &regex::Regex,
        results: &mut Vec<SearchResult>,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip hidden files and common ignored directories
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }

            if path.is_dir() {
                self.search_recursive(&path, pattern, results)?;
            } else if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_num, line) in content.lines().enumerate() {
                    if pattern.is_match(line) {
                        results.push(SearchResult {
                            path: path.clone(),
                            line_number: line_num + 1,
                            line: line.to_string(),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    pub fn apply_patch(&self, path: &str, old_content: &str, new_content: &str) -> Result<String> {
        let full_path = self.resolve_path(path);
        let file_content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read file for patching: {}", full_path.display()))?;

        if !file_content.contains(old_content) {
            anyhow::bail!(
                "Could not find the specified content in file: {}",
                full_path.display()
            );
        }

        let updated = file_content.replacen(old_content, new_content, 1);
        std::fs::write(&full_path, &updated)
            .with_context(|| format!("Failed to write patched file: {}", full_path.display()))?;

        Ok(updated)
    }
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub line_number: usize,
    pub line: String,
}
