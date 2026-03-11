use std::path::PathBuf;
use std::collections::HashMap;
use anyhow::{Context, Result, anyhow};

pub struct PatchEngine {
    workspace_root: PathBuf,
}

impl PatchEngine {
    pub fn new(workspace_root: &str) -> Self {
        Self { workspace_root: PathBuf::from(workspace_root) }
    }

    pub async fn apply(&self, diff: &str) -> Result<Vec<String>> {
        let file_hunks = parse_unified_diff(diff)?;
        let mut modified = vec![];

        for (file_path, hunks) in &file_hunks {
            let full = self.workspace_root.join(file_path);
            let original = if full.exists() {
                tokio::fs::read_to_string(&full).await?
            } else {
                String::new()
            };

            let patched = apply_hunks(&original, hunks)
                .with_context(|| format!("Failed to patch {}", file_path))?;

            if let Some(parent) = full.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&full, &patched).await?;
            modified.push(file_path.clone());
        }

        Ok(modified)
    }
}

#[derive(Debug, Clone)]
struct Hunk {
    old_start: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Added(String),
    Removed(String),
}

fn parse_unified_diff(diff: &str) -> Result<HashMap<String, Vec<Hunk>>> {
    let mut files: HashMap<String, Vec<Hunk>> = HashMap::new();
    let mut current_file: Option<String> = None;
    let mut current_hunks: Vec<Hunk> = vec![];
    let mut current_hunk: Option<Hunk> = None;

    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            if let Some(f) = current_file.take() {
                if let Some(h) = current_hunk.take() { current_hunks.push(h); }
                files.insert(f, std::mem::take(&mut current_hunks));
            }
            current_file = Some(rest.to_string());
        } else if line.starts_with("@@ ") {
            if let Some(h) = current_hunk.take() { current_hunks.push(h); }
            if let Ok(hunk) = parse_hunk_header(line) {
                current_hunk = Some(hunk);
            }
        } else if let Some(ref mut hunk) = current_hunk {
            if let Some(rest) = line.strip_prefix('+') {
                hunk.lines.push(HunkLine::Added(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix('-') {
                hunk.lines.push(HunkLine::Removed(rest.to_string()));
            } else if let Some(rest) = line.strip_prefix(' ') {
                hunk.lines.push(HunkLine::Context(rest.to_string()));
            }
        }
    }

    if let Some(f) = current_file {
        if let Some(h) = current_hunk { current_hunks.push(h); }
        files.insert(f, current_hunks);
    }

    Ok(files)
}

fn parse_hunk_header(line: &str) -> Result<Hunk> {
    // @@ -old_start,old_count +new_start,new_count @@
    let inner = line.trim_start_matches("@@ ").trim_end_matches(" @@").trim_end();
    let parts: Vec<&str> = inner.split(' ').collect();
    if parts.len() < 2 {
        return Err(anyhow!("Invalid hunk header: {}", line));
    }
    let old_part = parts[0].trim_start_matches('-');
    let old_start: usize = old_part.split(',').next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    Ok(Hunk { old_start, lines: vec![] })
}

fn apply_hunks(original: &str, hunks: &[Hunk]) -> Result<String> {
    let orig_lines: Vec<&str> = original.lines().collect();
    let mut result: Vec<String> = vec![];
    let mut orig_pos = 0usize;

    for hunk in hunks {
        let hunk_start = hunk.old_start.saturating_sub(1);

        while orig_pos < hunk_start && orig_pos < orig_lines.len() {
            result.push(orig_lines[orig_pos].to_string());
            orig_pos += 1;
        }

        for hl in &hunk.lines {
            match hl {
                HunkLine::Context(s) => {
                    result.push(s.clone());
                    orig_pos += 1;
                }
                HunkLine::Added(s) => {
                    result.push(s.clone());
                }
                HunkLine::Removed(_) => {
                    orig_pos += 1;
                }
            }
        }
    }

    while orig_pos < orig_lines.len() {
        result.push(orig_lines[orig_pos].to_string());
        orig_pos += 1;
    }

    let mut out = result.join("\n");
    if original.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}
