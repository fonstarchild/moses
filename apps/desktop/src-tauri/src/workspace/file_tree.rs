use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<FileNode>>,
}

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

pub fn list_files(root: &str, max_depth: usize) -> Result<Vec<FileNode>, anyhow::Error> {
    list_dir(Path::new(root), root, 0, max_depth)
}

fn list_dir(
    dir: &Path,
    root: &str,
    depth: usize,
    max_depth: usize,
) -> Result<Vec<FileNode>, anyhow::Error> {
    if depth > max_depth {
        return Ok(vec![]);
    }

    let mut entries: Vec<FileNode> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name_str = name.to_str().unwrap_or("");
            !IGNORED.contains(&name_str) && !name_str.starts_with('.')
        })
        .filter_map(|e| {
            let path = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            let rel_path = path.strip_prefix(root).ok()?.to_string_lossy().to_string();
            let is_dir = path.is_dir();

            let children = if is_dir {
                list_dir(&path, root, depth + 1, max_depth).ok()
            } else {
                None
            };

            Some(FileNode {
                name,
                path: format!("{}/{}", root, rel_path),
                is_dir,
                children,
            })
        })
        .collect();

    entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });

    Ok(entries)
}
