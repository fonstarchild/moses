use crate::patch::apply::PatchEngine;
use crate::security::sandbox::Sandbox;
use crate::workspace::vector_store::VectorStore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolResult {
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    /// If a file was written, its absolute path (for FileWritten event).
    #[serde(skip)]
    pub written_path: Option<String>,
}

impl ToolResult {
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: output.into(),
            error: None,
            written_path: None,
        }
    }
    pub fn err(e: impl Into<String>) -> Self {
        Self {
            success: false,
            output: String::new(),
            error: Some(e.into()),
            written_path: None,
        }
    }
    pub fn written(output: impl Into<String>, path: impl Into<String>) -> Self {
        let p = path.into();
        Self {
            success: true,
            output: output.into(),
            error: None,
            written_path: Some(p),
        }
    }
}

impl std::fmt::Display for ToolResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.success {
            write!(f, "{}", self.output)
        } else {
            write!(f, "ERROR: {}", self.error.as_deref().unwrap_or("unknown"))
        }
    }
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "read_file",
            "description": "Read the content of a file. Use relative paths from workspace root.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative path from workspace root, e.g. src/main.rs" }
                },
                "required": ["path"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Write content to a file, creating it and any missing parent directories. REQUIRES USER APPROVAL.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative path from workspace root" },
                    "content": { "type": "string", "description": "Full file content to write" }
                },
                "required": ["path", "content"]
            }
        }),
        json!({
            "name": "list_files",
            "description": "List files and directories at a path (one level deep).",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Relative path, defaults to workspace root", "default": "." }
                }
            }
        }),
        json!({
            "name": "search_code",
            "description": "Search for text patterns across all code files in the workspace using the indexed database. Returns matching snippets.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query (text, function name, keyword, etc.)" },
                    "top_k": { "type": "integer", "description": "Max results to return", "default": 10 }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "grep",
            "description": "Grep for an exact pattern in file contents using ripgrep. Good for finding specific strings or symbols.",
            "parameters": {
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "Literal text or regex pattern to search for" },
                    "file_glob": { "type": "string", "description": "Optional glob pattern like '*.rs' or '*.ts'", "default": "" }
                },
                "required": ["pattern"]
            }
        }),
        json!({
            "name": "run_command",
            "description": "Run a shell command in the workspace directory. Use for builds, tests, installs. REQUIRES USER APPROVAL.",
            "parameters": {
                "type": "object",
                "properties": {
                    "cmd": { "type": "string", "description": "Shell command to run" },
                    "timeout_secs": { "type": "integer", "description": "Timeout in seconds", "default": 60 }
                },
                "required": ["cmd"]
            }
        }),
        json!({
            "name": "apply_patch",
            "description": "Apply a unified diff patch to modify one or more files. REQUIRES USER APPROVAL.",
            "parameters": {
                "type": "object",
                "properties": {
                    "diff": { "type": "string", "description": "Unified diff format (--- a/file ... +++ b/file ...)" }
                },
                "required": ["diff"]
            }
        }),
        json!({
            "name": "git_diff",
            "description": "Get current git diff to see what changes have been made.",
            "parameters": { "type": "object", "properties": {} }
        }),
        json!({
            "name": "git_commit",
            "description": "Stage all changes and create a git commit. REQUIRES USER APPROVAL.",
            "parameters": {
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "Commit message" }
                },
                "required": ["message"]
            }
        }),
    ]
}

pub async fn execute_tool(
    name: &str,
    args: Value,
    workspace_root: &str,
) -> Result<ToolResult, anyhow::Error> {
    let root = PathBuf::from(workspace_root);

    match name {
        "read_file" => {
            let path = args["path"].as_str().unwrap_or_default();
            let full = resolve_path(&root, path);
            match tokio::fs::read_to_string(&full).await {
                Ok(content) => {
                    // Cap at 20k chars
                    let capped: String = content.chars().take(20_000).collect();
                    let note = if content.len() > 20_000 {
                        "\n[...truncated at 20k chars]"
                    } else {
                        ""
                    };
                    Ok(ToolResult::ok(format!("{}{}", capped, note)))
                }
                Err(e) => Ok(ToolResult::err(e.to_string())),
            }
        }

        "write_file" => {
            let path = args["path"].as_str().unwrap_or_default();
            let content = args["content"].as_str().unwrap_or_default();
            let full = resolve_path(&root, path);
            if let Some(parent) = full.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&full, content).await?;
            let abs = full.to_string_lossy().to_string();
            Ok(ToolResult::written(
                format!("Written {} bytes to {}", content.len(), path),
                abs,
            ))
        }

        "list_files" => {
            let path = args["path"].as_str().unwrap_or(".");
            let full = resolve_path(&root, path);
            let mut entries = vec![];
            if let Ok(mut dir) = tokio::fs::read_dir(&full).await {
                while let Ok(Some(entry)) = dir.next_entry().await {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                    entries.push(if is_dir { format!("{}/", name) } else { name });
                }
            }
            entries.sort();
            Ok(ToolResult::ok(entries.join("\n")))
        }

        "search_code" => {
            let query = args["query"].as_str().unwrap_or_default();
            let top_k = args["top_k"].as_u64().unwrap_or(10) as usize;
            match VectorStore::open(workspace_root) {
                Ok(store) => match store.search(query, top_k) {
                    Ok(results) if !results.is_empty() => {
                        let text = results
                            .iter()
                            .map(|r| format!("{}:{}\n{}", r.file, r.line, r.snippet))
                            .collect::<Vec<_>>()
                            .join("\n---\n");
                        Ok(ToolResult::ok(text))
                    }
                    Ok(_) => Ok(ToolResult::ok("No results found.")),
                    Err(e) => Ok(ToolResult::err(e.to_string())),
                },
                Err(e) => Ok(ToolResult::err(format!("Index not available: {}", e))),
            }
        }

        "grep" => {
            let pattern = args["pattern"].as_str().unwrap_or_default();
            let file_glob = args["file_glob"].as_str().unwrap_or("");

            let mut cmd_args = vec!["-rn", "--max-count=5"];
            let include_arg;
            if !file_glob.is_empty() {
                include_arg = format!("--include={}", file_glob);
                cmd_args.push(&include_arg);
            }
            cmd_args.push(pattern);
            cmd_args.push(".");

            let output = Command::new("grep")
                .args(&cmd_args)
                .current_dir(&root)
                .output()
                .await?;
            let result = String::from_utf8_lossy(&output.stdout).to_string();
            let capped: String = result.chars().take(3000).collect();
            Ok(ToolResult::ok(if capped.is_empty() {
                "No matches.".into()
            } else {
                capped
            }))
        }

        "run_command" => {
            let cmd = args["cmd"].as_str().unwrap_or_default();
            let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(60);
            let sandbox = Sandbox::new(workspace_root);
            sandbox.run(cmd, timeout_secs).await
        }

        "apply_patch" => {
            let diff = args["diff"].as_str().unwrap_or_default();
            let engine = PatchEngine::new(workspace_root);
            match engine.apply(diff).await {
                Ok(files) => Ok(ToolResult::ok(format!("Patched: {}", files.join(", ")))),
                Err(e) => Ok(ToolResult::err(e.to_string())),
            }
        }

        "git_diff" => {
            let output = Command::new("git")
                .args(["diff", "HEAD"])
                .current_dir(&root)
                .output()
                .await?;
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            let capped: String = text.chars().take(4000).collect();
            Ok(ToolResult::ok(if capped.is_empty() {
                "No changes.".into()
            } else {
                capped
            }))
        }

        "git_commit" => {
            let message = args["message"].as_str().unwrap_or("Auto-commit by Moses");
            Command::new("git")
                .args(["add", "-A"])
                .current_dir(&root)
                .output()
                .await?;
            let output = Command::new("git")
                .args(["commit", "-m", message])
                .current_dir(&root)
                .output()
                .await?;
            Ok(ToolResult::ok(String::from_utf8_lossy(&output.stdout)))
        }

        _ => Ok(ToolResult::err(format!("Unknown tool: {}", name))),
    }
}

/// Resolve a relative path against the workspace root.
/// If the path is already absolute, use it directly.
fn resolve_path(root: &std::path::Path, path: &str) -> PathBuf {
    let p = PathBuf::from(path);
    if p.is_absolute() {
        p
    } else {
        root.join(path)
    }
}
