use crate::workspace::vector_store::VectorStore;
use std::path::PathBuf;

const CHARS_PER_TOKEN: usize = 4;
const IGNORED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".venv",
    "venv",
    ".moses",
];

pub struct ContextBuilder {
    workspace_root: String,
    open_files: Vec<String>,
    semantic_query: Option<String>,
    semantic_k: usize,
}

impl ContextBuilder {
    pub fn new(workspace_root: &str) -> Self {
        Self {
            workspace_root: workspace_root.to_string(),
            open_files: vec![],
            semantic_query: None,
            semantic_k: 8,
        }
    }

    pub fn with_open_files(mut self, files: &[String]) -> Self {
        self.open_files = files.to_vec();
        self
    }

    pub fn with_semantic_search(mut self, query: &str, k: usize) -> Self {
        self.semantic_query = Some(query.to_string());
        self.semantic_k = k;
        self
    }

    pub async fn build(self, max_tokens: usize) -> Result<String, anyhow::Error> {
        let budget = max_tokens * CHARS_PER_TOKEN;
        let mut context = String::new();
        let mut used = 0usize;

        // Priority 1: open files — always include these in full, they're what the user is looking at
        for file in &self.open_files {
            let path = PathBuf::from(file);
            let path = if path.is_absolute() {
                path
            } else {
                PathBuf::from(&self.workspace_root).join(file)
            };
            if let Ok(content) = tokio::fs::read_to_string(&path).await {
                let display = path
                    .strip_prefix(&self.workspace_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file.clone());
                // Cap a single file at 12k chars (~3k tokens) to leave room for other context
                let text: String = content.chars().take(12_000).collect();
                let section = format!("## Open File: {}\n```\n{}\n```\n\n", display, text);
                context.push_str(&section);
                used += section.len();
            }
        }

        // Priority 2: file tree (capped at 1500 chars)
        if used < budget {
            let tree = self.build_file_tree();
            let tree_section = format!(
                "## Project Structure\n```\n{}\n```\n\n",
                &tree.chars().take(1500).collect::<String>()
            );
            context.push_str(&tree_section);
            used += tree_section.len();
        }

        // Priority 3: semantic search results
        if used < budget {
            if let Some(ref query) = self.semantic_query {
                if let Ok(store) = VectorStore::open(&self.workspace_root) {
                    if let Ok(results) = store.search(query, self.semantic_k) {
                        for r in &results {
                            if used >= budget {
                                break;
                            }
                            let section = format!(
                                "## Related: {}:{}\n```\n{}\n```\n\n",
                                r.file, r.line, r.snippet
                            );
                            context.push_str(&section);
                            used += section.len();
                        }
                    }
                }
            }
        }

        Ok(context)
    }

    fn build_file_tree(&self) -> String {
        use walkdir::WalkDir;
        WalkDir::new(&self.workspace_root)
            .max_depth(4)
            .into_iter()
            .filter_entry(|e| !IGNORED.contains(&e.file_name().to_str().unwrap_or("")))
            .filter_map(|e| e.ok())
            .map(|e| {
                let depth = e.depth();
                let name = e.file_name().to_string_lossy().to_string();
                format!("{}{}", "  ".repeat(depth), name)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
