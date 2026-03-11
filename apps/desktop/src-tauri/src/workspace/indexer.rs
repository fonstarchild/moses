use crate::workspace::vector_store::{CodeChunk, VectorStore};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const IGNORED_DIRS: &[&str] = &[
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

const SUPPORTED_EXTS: &[&str] = &[
    "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "cpp", "c", "h", "cs", "rb", "swift", "kt",
    "md", "toml", "yaml", "json", "sh",
];

pub struct WorkspaceIndexer {
    root: PathBuf,
}

impl WorkspaceIndexer {
    pub fn new(root: &str) -> Self {
        Self {
            root: PathBuf::from(root),
        }
    }

    pub async fn index_all(&self, store: &mut VectorStore) -> Result<usize, anyhow::Error> {
        store.clear_all()?;
        let files = self.collect_files();
        let mut total = 0;

        for file in &files {
            if let Ok(chunks) = self.chunk_file(file).await {
                for chunk in chunks {
                    store.insert_text(&chunk)?;
                    total += 1;
                }
            }
        }

        Ok(total)
    }

    pub async fn index_file(
        &self,
        path: &Path,
        store: &mut VectorStore,
    ) -> Result<usize, anyhow::Error> {
        let rel = path.strip_prefix(&self.root)?.to_string_lossy().to_string();
        store.clear_file(&rel)?;
        let chunks = self.chunk_file(path).await?;
        let count = chunks.len();
        for chunk in chunks {
            store.insert_text(&chunk)?;
        }
        Ok(count)
    }

    async fn chunk_file(&self, path: &Path) -> Result<Vec<CodeChunk>, anyhow::Error> {
        let rel = path
            .strip_prefix(&self.root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content = tokio::fs::read_to_string(path).await?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        let chunks = match ext {
            "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "cpp" | "c" | "cs"
            | "swift" | "kt" => chunk_by_declarations(&content, &rel),
            _ => chunk_by_lines(&content, &rel, 60),
        };

        Ok(chunks)
    }

    fn collect_files(&self) -> Vec<PathBuf> {
        WalkDir::new(&self.root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !IGNORED_DIRS.contains(&e.file_name().to_str().unwrap_or("")))
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| SUPPORTED_EXTS.contains(&x))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_owned())
            .collect()
    }
}

/// Splits code at top-level function/class/struct/impl declarations.
fn chunk_by_declarations(content: &str, file: &str) -> Vec<CodeChunk> {
    // Patterns that start a new logical chunk
    let declaration_patterns = [
        "fn ",
        "pub fn ",
        "async fn ",
        "pub async fn ",
        "impl ",
        "pub impl ",
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "pub trait ",
        "class ",
        "def ",
        "func ",
        "function ",
        "export function ",
        "export const ",
        "export default ",
        "export class ",
        "export interface ",
        "interface ",
        "type ",
        "mod ",
    ];

    let lines: Vec<&str> = content.lines().collect();
    let mut chunks: Vec<CodeChunk> = vec![];
    let mut chunk_start = 0usize;
    let mut chunk_lines: Vec<&str> = vec![];
    let mut chunk_kind = "code";

    for (i, &line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let is_decl = declaration_patterns.iter().any(|p| trimmed.starts_with(p))
            && !trimmed.starts_with("//")
            && !trimmed.starts_with('#');

        if is_decl && !chunk_lines.is_empty() && chunk_lines.len() > 2 {
            // flush current chunk
            let text = chunk_lines.join("\n");
            if text.trim().len() > 20 {
                chunks.push(CodeChunk {
                    file: file.to_string(),
                    line: chunk_start + 1,
                    text,
                    node_kind: chunk_kind.to_string(),
                });
            }
            chunk_start = i;
            chunk_lines = vec![line];
            // detect kind
            chunk_kind = if trimmed.contains("fn ")
                || trimmed.contains("function")
                || trimmed.contains("def ")
                || trimmed.contains("func ")
            {
                "function"
            } else if trimmed.contains("struct ")
                || trimmed.contains("class ")
                || trimmed.contains("interface ")
            {
                "type"
            } else if trimmed.contains("impl ") || trimmed.contains("trait ") {
                "impl"
            } else {
                "declaration"
            };
        } else {
            chunk_lines.push(line);
        }

        // also flush on large chunks
        if chunk_lines.len() >= 80 {
            let text = chunk_lines.join("\n");
            if text.trim().len() > 20 {
                chunks.push(CodeChunk {
                    file: file.to_string(),
                    line: chunk_start + 1,
                    text,
                    node_kind: chunk_kind.to_string(),
                });
            }
            chunk_start = i + 1;
            chunk_lines = vec![];
            chunk_kind = "code";
        }
    }

    // flush remaining
    if !chunk_lines.is_empty() {
        let text = chunk_lines.join("\n");
        if text.trim().len() > 20 {
            chunks.push(CodeChunk {
                file: file.to_string(),
                line: chunk_start + 1,
                text,
                node_kind: chunk_kind.to_string(),
            });
        }
    }

    if chunks.is_empty() {
        chunk_by_lines(content, file, 60)
    } else {
        chunks
    }
}

fn chunk_by_lines(content: &str, file: &str, size: usize) -> Vec<CodeChunk> {
    content
        .lines()
        .collect::<Vec<_>>()
        .chunks(size)
        .enumerate()
        .filter_map(|(i, lines)| {
            let text = lines.join("\n");
            if text.trim().len() > 10 {
                Some(CodeChunk {
                    file: file.to_string(),
                    line: i * size + 1,
                    text,
                    node_kind: "lines".to_string(),
                })
            } else {
                None
            }
        })
        .collect()
}
