use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::process::Command;

pub struct GitTool {
    working_dir: PathBuf,
}

impl GitTool {
    pub fn new(working_dir: PathBuf) -> Self {
        GitTool { working_dir }
    }

    async fn run_git(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.working_dir)
            .output()
            .await
            .with_context(|| format!("Failed to run git {:?}", args))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git error: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    pub async fn status(&self) -> Result<String> {
        self.run_git(&["status", "--short"]).await
    }

    pub async fn diff(&self, staged: bool) -> Result<String> {
        if staged {
            self.run_git(&["diff", "--cached"]).await
        } else {
            self.run_git(&["diff"]).await
        }
    }

    pub async fn log(&self, count: usize) -> Result<String> {
        let count_str = count.to_string();
        self.run_git(&[
            "log",
            "--oneline",
            "--graph",
            "--decorate",
            &format!("-{}", count_str),
        ])
        .await
    }

    pub async fn add(&self, files: &[&str]) -> Result<String> {
        let mut args = vec!["add"];
        args.extend_from_slice(files);
        self.run_git(&args).await
    }

    pub async fn commit(&self, message: &str) -> Result<String> {
        self.run_git(&["commit", "-m", message]).await
    }

    pub async fn branch_list(&self) -> Result<String> {
        self.run_git(&["branch", "-a"]).await
    }

    pub async fn current_branch(&self) -> Result<String> {
        let output = self.run_git(&["rev-parse", "--abbrev-ref", "HEAD"]).await?;
        Ok(output.trim().to_string())
    }

    pub async fn is_git_repo(&self) -> bool {
        self.run_git(&["rev-parse", "--git-dir"]).await.is_ok()
    }

    pub async fn stash(&self) -> Result<String> {
        self.run_git(&["stash"]).await
    }

    pub async fn stash_pop(&self) -> Result<String> {
        self.run_git(&["stash", "pop"]).await
    }
}
