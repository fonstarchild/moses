use crate::agent::tools::ToolResult;
use std::path::PathBuf;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

const BLOCKED: &[&str] = &[
    "rm -rf /",
    "sudo rm",
    "mkfs",
    ":(){:|:&};:",
    "dd if=/dev/zero",
];

pub struct Sandbox {
    workspace_root: PathBuf,
}

impl Sandbox {
    pub fn new(workspace_root: &str) -> Self {
        Self {
            workspace_root: PathBuf::from(workspace_root),
        }
    }

    pub async fn run(&self, cmd: &str, timeout_secs: u64) -> Result<ToolResult, anyhow::Error> {
        for blocked in BLOCKED {
            if cmd.contains(blocked) {
                return Ok(ToolResult::err(format!("Blocked: '{}'", blocked)));
            }
        }

        let result = timeout(
            Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.workspace_root)
                .output(),
        )
        .await;

        match result {
            Ok(Ok(output)) => {
                let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if !stderr.is_empty() {
                    combined.push_str("\nSTDERR:\n");
                    combined.push_str(&stderr);
                }
                Ok(ToolResult::ok(combined))
            }
            Ok(Err(e)) => Ok(ToolResult::err(e.to_string())),
            Err(_) => Ok(ToolResult::err(format!(
                "Timed out after {}s",
                timeout_secs
            ))),
        }
    }
}
