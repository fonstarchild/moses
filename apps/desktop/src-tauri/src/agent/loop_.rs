use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::sync::oneshot;
use crate::llm::client::LlmClient;
use crate::agent::task::AgentTask;
use crate::memory::short_term::ConversationMemory;
use crate::workspace::file_tree::list_files;
use crate::workspace::vector_store::VectorStore;

// ── Events ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum AgentEvent {
    Thinking      { content: String },
    StreamToken   { token: String },
    ConfirmWrite  { id: String, path: String, preview: String },
    FileWritten   { path: String },
    Done          { summary: String },
    Error         { message: String },
}

// ── Permission gate ───────────────────────────────────────────────────────────

type PermMap = Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>;

fn perm_map() -> PermMap {
    use once_cell::sync::OnceCell;
    static MAP: OnceCell<PermMap> = OnceCell::new();
    MAP.get_or_init(|| Arc::new(Mutex::new(HashMap::new()))).clone()
}

pub fn resolve_confirm(id: &str, approved: bool) {
    if let Ok(mut map) = perm_map().lock() {
        if let Some(tx) = map.remove(id) { let _ = tx.send(approved); }
    }
}

async fn ask_write_permission(app: &AppHandle, id: &str, path: &str, preview: &str) -> bool {
    let (tx, rx) = oneshot::channel::<bool>();
    { perm_map().lock().unwrap().insert(id.to_string(), tx); }
    app.emit_all("agent-event", AgentEvent::ConfirmWrite {
        id: id.to_string(),
        path: path.to_string(),
        preview: preview.chars().take(400).collect(),
    }).ok();
    match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
        Ok(Ok(v)) => v,
        _ => { perm_map().lock().unwrap().remove(id); false }
    }
}

// ── Agent loop ────────────────────────────────────────────────────────────────

pub struct AgentLoop {
    llm: LlmClient,
    memory: ConversationMemory,
    app: AppHandle,
}

impl AgentLoop {
    pub fn new(llm: LlmClient, app: AppHandle) -> Self {
        Self { llm, memory: ConversationMemory::new(128_000), app }
    }

    pub async fn run(&mut self, task: AgentTask) -> Result<(), anyhow::Error> {
        self.emit(AgentEvent::Thinking { content: "Reading project…".into() });

        // ── 1. Gather context ─────────────────────────────────────────────────
        let file_tree  = build_file_tree(&task.workspace_root);
        let open_file  = read_open_file(&task).await;
        let search_ctx = semantic_search(&task.workspace_root, &task.prompt).await;

        // ── 2. Build user message — context + request, nothing else ───────────
        let user_message = build_user_message(&task, &file_tree, &open_file, &search_ctx);
        self.memory.add_user_message(&user_message);

        // ── 3. Call model ─────────────────────────────────────────────────────
        let system = build_system();
        let messages = self.memory.to_messages();
        let app_clone = self.app.clone();
        let on_token = move |token: String| {
            app_clone.emit_all("agent-event", AgentEvent::StreamToken { token }).ok();
        };

        match self.llm.stream_text(&system, &messages, on_token).await {
            Ok(response) => {
                self.memory.add_assistant_message(&response);

                // ── 4. Find file-worthy code blocks and offer to save them ────
                let candidates = find_file_candidates(&response, &task, &open_file);
                for (path, content) in candidates {
                    let id = unique_id();
                    let approved = ask_write_permission(&self.app, &id, &path, &content).await;
                    if approved {
                        if let Some(parent) = Path::new(&path).parent() {
                            tokio::fs::create_dir_all(parent).await.ok();
                        }
                        if tokio::fs::write(&path, &content).await.is_ok() {
                            self.emit(AgentEvent::FileWritten { path });
                        }
                    }
                }

                self.emit(AgentEvent::Done { summary: String::new() });
            }
            Err(e) => self.emit(AgentEvent::Error { message: e.to_string() }),
        }

        Ok(())
    }

    fn emit(&self, event: AgentEvent) {
        self.app.emit_all("agent-event", &event).ok();
    }
}

// ── System prompt ─────────────────────────────────────────────────────────────

fn build_system() -> String {
    // Keep it short — deepseek-coder largely ignores long system prompts.
    // The personality lives here; the context lives in the user message.
    "You are Moses, a brilliant coding assistant and close collaborator. \
     You are knowledgeable, direct, and friendly. \
     You explain your thinking, ask clarifying questions when needed, \
     and write complete, working code. \
     When you write a file, output its full content in a fenced code block. \
     Never truncate code with comments like '// ... rest unchanged'."
    .to_string()
}

// ── User message builder ──────────────────────────────────────────────────────

struct OpenFile { path: String, display: String, lang: &'static str, content: String }

fn build_user_message(
    task: &AgentTask,
    file_tree: &str,
    open_file: &Option<OpenFile>,
    search_ctx: &str,
) -> String {
    let mut msg = String::new();

    // File tree — gives Moses project awareness
    if !file_tree.is_empty() {
        msg.push_str(&format!("Project: {}\n```\n{}\n```\n\n", task.workspace_root, file_tree));
    }

    // Relevant code snippets from the index
    if !search_ctx.is_empty() {
        msg.push_str(&format!("Relevant code in this project:\n```\n{}\n```\n\n", search_ctx));
    }

    // The file the user is looking at
    if let Some(f) = open_file {
        msg.push_str(&format!(
            "Currently open: {}\n```{}\n{}\n```\n\n",
            f.display, f.lang, f.content.trim()
        ));
    }

    // The actual request — plain, no formatting instructions
    msg.push_str(&task.prompt);

    msg
}

// ── Context readers ───────────────────────────────────────────────────────────

fn build_file_tree(workspace_root: &str) -> String {
    match list_files(workspace_root, 2) {
        Ok(nodes) => {
            let tree = render_tree(&nodes, 0);
            // Cap at 1500 chars so it doesn't eat the context window
            tree.chars().take(1500).collect()
        }
        Err(_) => String::new(),
    }
}

fn render_tree(nodes: &[crate::workspace::file_tree::FileNode], depth: usize) -> String {
    let mut out = String::new();
    let indent = "  ".repeat(depth);
    for node in nodes {
        if node.is_dir {
            out.push_str(&format!("{}{}/\n", indent, node.name));
            if let Some(ch) = &node.children { out.push_str(&render_tree(ch, depth + 1)); }
        } else {
            out.push_str(&format!("{}{}\n", indent, node.name));
        }
    }
    out
}

async fn read_open_file(task: &AgentTask) -> Option<OpenFile> {
    let path = task.open_files.first()?;
    let content = tokio::fs::read_to_string(path).await.ok()?;
    let display = Path::new(path)
        .strip_prefix(&task.workspace_root)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.clone());
    let ext = path.rsplit('.').next().unwrap_or("");
    Some(OpenFile {
        path: path.clone(),
        display,
        lang: ext_to_lang(ext),
        content: content.chars().take(14_000).collect(),
    })
}

async fn semantic_search(workspace_root: &str, query: &str) -> String {
    let store = match VectorStore::open(workspace_root) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    match store.search(query, 5) {
        Ok(results) if !results.is_empty() => results
            .iter()
            .map(|r| format!("// {}:{}\n{}", r.file, r.line, r.snippet))
            .collect::<Vec<_>>()
            .join("\n---\n"),
        _ => String::new(),
    }
}

// ── File candidate detection ──────────────────────────────────────────────────
//
// We scan the model's natural response for code blocks that look like complete
// files (not tiny snippets), then infer where to save them.
// The model doesn't need to follow any special format.

fn find_file_candidates(
    response: &str,
    task: &AgentTask,
    open_file: &Option<OpenFile>,
) -> Vec<(String, String)> {
    let blocks = extract_all_code_blocks(response);
    if blocks.is_empty() { return vec![]; }

    let p = task.prompt.to_lowercase();
    let wants_new_file = p.contains("create") || p.contains("new file")
        || p.contains("test file") || p.contains("generate") || p.contains("write a")
        || p.contains("add a file") || p.contains("make a");
    let wants_edit = p.contains("edit") || p.contains("fix") || p.contains("refactor")
        || p.contains("update") || p.contains("add") || p.contains("change")
        || p.contains("remove") || p.contains("implement");

    let mut results = Vec::new();

    for (lang, content) in &blocks {
        // Skip tiny snippets — if it's less than 3 lines it's probably an example
        if content.lines().count() < 3 { continue; }

        if wants_new_file {
            let path = infer_new_file_path(open_file, &task.workspace_root, &task.prompt, lang);
            results.push((path, content.clone()));
            break; // one file per request is enough
        } else if wants_edit {
            if let Some(f) = open_file {
                // Only offer to overwrite the open file if the lang matches
                if lang.is_empty() || f.lang == lang || content.lines().count() > 10 {
                    results.push((f.path.clone(), content.clone()));
                    break;
                }
            }
        }
        // For other requests (explain, question, etc.) — don't write anything
    }

    results
}

/// Decide where a new file should be saved.
fn infer_new_file_path(
    open_file: &Option<OpenFile>,
    workspace_root: &str,
    prompt: &str,
    lang: &str,
) -> String {
    let p = prompt.to_lowercase();

    // Did the user mention a filename directly? e.g. "create utils.ts"
    for word in prompt.split_whitespace() {
        let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-');
        if w.contains('.') && !w.starts_with('.') && w.len() > 2 {
            let dir = open_file.as_ref()
                .and_then(|f| Path::new(&f.path).parent())
                .map(|d| d.to_string_lossy().to_string())
                .unwrap_or_else(|| workspace_root.to_string());
            return format!("{}/{}", dir, w);
        }
    }

    // Derive from open file + request type
    if let Some(f) = open_file {
        let stem = Path::new(&f.path).file_stem()
            .and_then(|s| s.to_str()).unwrap_or("file");
        let ext = Path::new(&f.path).extension()
            .and_then(|s| s.to_str()).unwrap_or(lang);
        let dir = Path::new(&f.path).parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_else(|| workspace_root.to_string());

        if p.contains("test") {
            return match ext {
                "rs"            => format!("{}/{}_test.rs", dir, stem),
                "ts" | "tsx"    => format!("{}/{}.test.ts", dir, stem),
                "js" | "jsx"    => format!("{}/{}.test.js", dir, stem),
                "py"            => format!("{}/test_{}.py", dir, stem),
                "go"            => format!("{}/{}_test.go", dir, stem),
                _               => format!("{}/{}_test.{}", dir, stem, ext),
            };
        }

        // Generic new file next to the open one
        let new_ext = if lang.is_empty() { ext } else { lang_to_ext(lang) };
        return format!("{}/new_{}.{}", dir, stem, new_ext);
    }

    // No open file — put it at workspace root
    let ext = if lang.is_empty() { "txt" } else { lang_to_ext(lang) };
    format!("{}/new_file.{}", workspace_root, ext)
}

// ── Code block extraction ─────────────────────────────────────────────────────

/// Returns Vec<(lang, content)> for every complete fenced code block in text.
/// Incomplete blocks (no closing ```) are skipped — truncated output should not be saved.
fn extract_all_code_blocks(text: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let (lang, content_start) = match after.find('\n') {
            Some(nl) => (after[..nl].trim().to_string(), nl + 1),
            None => break,
        };
        let content = &after[content_start..];
        match content.find("\n```") {
            Some(end) => {
                let body = content[..end].to_string();
                // Only keep blocks that look like real files (3+ non-empty lines)
                let real_lines = body.lines().filter(|l| !l.trim().is_empty()).count();
                if real_lines >= 3 {
                    results.push((lang, body));
                }
                rest = &content[end + 4..];
            }
            // No closing fence — model was cut off, skip this block entirely
            None => break,
        }
    }
    results
}

// ── Misc ──────────────────────────────────────────────────────────────────────

fn ext_to_lang(ext: &str) -> &'static str {
    match ext {
        "rs" => "rust", "ts"|"tsx" => "typescript", "js"|"jsx" => "javascript",
        "py" => "python", "go" => "go", "cpp"|"cc"|"cxx" => "cpp", "c" => "c",
        "cs" => "csharp", "java" => "java", "sh" => "bash", "toml" => "toml",
        "json" => "json", "yaml"|"yml" => "yaml", "md" => "markdown",
        "html" => "html", "css" => "css", "sql" => "sql", _ => "",
    }
}

fn lang_to_ext(lang: &str) -> &'static str {
    match lang {
        "rust"       => "rs",
        "typescript" => "ts",
        "javascript" => "js",
        "python"     => "py",
        "go"         => "go",
        "cpp"|"c++"  => "cpp",
        "c"          => "c",
        "csharp"     => "cs",
        "java"       => "java",
        "bash"|"sh"  => "sh",
        "toml"       => "toml",
        "json"       => "json",
        "yaml"       => "yaml",
        "markdown"   => "md",
        "html"       => "html",
        "css"        => "css",
        "sql"        => "sql",
        _            => "txt",
    }
}

fn unique_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".into())
}
