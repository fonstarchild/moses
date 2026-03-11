use crate::workspace::indexer::WorkspaceIndexer;
use crate::workspace::vector_store::VectorStore;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::{mpsc, Mutex};

const DEBOUNCE_MS: u64 = 800; // wait 800ms after last change before re-indexing

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    pub kind: String, // "modified" | "created" | "deleted"
    pub path: String,
    pub chunks: usize,
}

/// Starts a file watcher for `workspace_root`.
/// Re-indexes changed files and emits "file-changed" events to the frontend.
/// Returns a handle that keeps the watcher alive.
pub async fn start_watcher(
    workspace_root: String,
    app: AppHandle,
) -> Result<WatcherHandle, anyhow::Error> {
    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(256);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.blocking_send(res).ok();
        },
        notify::Config::default().with_poll_interval(Duration::from_secs(1)),
    )?;

    watcher.watch(Path::new(&workspace_root), RecursiveMode::Recursive)?;

    let root = workspace_root.clone();
    let app_clone = app.clone();

    // Debounce state: path -> last seen timestamp
    let pending: Arc<Mutex<HashMap<PathBuf, Instant>>> = Arc::new(Mutex::new(HashMap::new()));
    let pending_clone = pending.clone();

    // Debounce flusher task
    let root_flush = root.clone();
    let app_flush = app_clone.clone();
    let pending_flush = pending.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;

            let now = Instant::now();
            let mut guard = pending_flush.lock().await;
            let ready: Vec<PathBuf> = guard
                .iter()
                .filter(|(_, t)| now.duration_since(**t) >= Duration::from_millis(DEBOUNCE_MS))
                .map(|(p, _)| p.clone())
                .collect();

            for path in ready {
                guard.remove(&path);
                drop(guard); // release lock while doing async work

                reindex_file(&path, &root_flush, &app_flush).await;

                guard = pending_flush.lock().await;
            }
        }
    });

    // Event receiver task
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let event = match event {
                Ok(e) => e,
                Err(_) => continue,
            };

            for path in event.paths {
                // Skip ignored dirs and unsupported extensions
                if is_ignored(&path) {
                    continue;
                }
                if !is_supported(&path) {
                    continue;
                }

                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        let mut guard = pending_clone.lock().await;
                        guard.insert(path, Instant::now());
                    }
                    EventKind::Remove(_) => {
                        // Remove from index immediately
                        if let Ok(store) = VectorStore::open(&root) {
                            let rel = path
                                .strip_prefix(&root)
                                .unwrap_or(&path)
                                .to_string_lossy()
                                .to_string();
                            store.clear_file(&rel).ok();
                            app_clone
                                .emit_all(
                                    "file-changed",
                                    WatchEvent {
                                        kind: "deleted".into(),
                                        path: rel,
                                        chunks: 0,
                                    },
                                )
                                .ok();
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(WatcherHandle { _watcher: watcher })
}

async fn reindex_file(path: &std::path::Path, root: &str, app: &AppHandle) {
    let indexer = WorkspaceIndexer::new(root);
    let mut store = match VectorStore::open(root) {
        Ok(s) => s,
        Err(_) => return,
    };

    let chunks = indexer.index_file(path, &mut store).await.unwrap_or(0);
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();

    let kind = if path.exists() { "modified" } else { "created" };

    app.emit_all(
        "file-changed",
        WatchEvent {
            kind: kind.into(),
            path: rel,
            chunks,
        },
    )
    .ok();
}

fn is_ignored(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_str().unwrap_or("");
        IGNORED_DIRS.contains(&s)
    })
}

fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTS.contains(&e))
        .unwrap_or(false)
}

/// Keeps the watcher alive as long as this handle is held.
pub struct WatcherHandle {
    _watcher: RecommendedWatcher,
}
