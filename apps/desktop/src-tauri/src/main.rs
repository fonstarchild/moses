// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![allow(dead_code, unused_variables)]

mod agent;
mod bridge;
mod llm;
mod memory;
mod patch;
mod security;
mod settings;
mod setup;
mod workspace;

use agent::loop_::{resolve_confirm, AgentLoop};
use agent::task::AgentTask;
use llm::client::LlmClient;
use memory::long_term::{LongTermMemory, ProjectFact};
use once_cell::sync::OnceCell;
use patch::apply::PatchEngine;
use std::sync::Arc;
use tauri::Manager;
use tokio::sync::Mutex;
use workspace::file_tree::{list_files, FileNode};
use workspace::indexer::WorkspaceIndexer;
use workspace::vector_store::{SearchResult, VectorStore};
use workspace::watcher::{start_watcher, WatcherHandle};

static WORKSPACE: OnceCell<Arc<Mutex<String>>> = OnceCell::new();
static MODEL: OnceCell<Arc<Mutex<String>>> = OnceCell::new();
static WATCHER: OnceCell<Arc<Mutex<Option<WatcherHandle>>>> = OnceCell::new();
static SETUP_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn get_watcher_slot() -> Arc<Mutex<Option<WatcherHandle>>> {
    WATCHER.get_or_init(|| Arc::new(Mutex::new(None))).clone()
}

fn get_workspace() -> Arc<Mutex<String>> {
    WORKSPACE
        .get_or_init(|| Arc::new(Mutex::new(String::new())))
        .clone()
}

fn get_model() -> Arc<Mutex<String>> {
    let saved = settings::load()
        .model
        .unwrap_or_else(|| "deepseek-coder:6.7b".to_string());
    MODEL.get_or_init(|| Arc::new(Mutex::new(saved))).clone()
}

#[tauri::command]
async fn set_workspace(path: String, app: tauri::AppHandle) {
    *get_workspace().lock().await = path.clone();
    // Start file watcher for the new workspace
    match start_watcher(path, app).await {
        Ok(handle) => {
            *get_watcher_slot().lock().await = Some(handle);
        }
        Err(e) => eprintln!("Watcher error: {}", e),
    }
}

#[tauri::command]
async fn set_model(model: String) {
    *get_model().lock().await = model.clone();
    let mut s = settings::load();
    s.model = Some(model);
    settings::save(&s);
}

#[tauri::command]
async fn load_settings() -> settings::Settings {
    settings::load()
}

#[tauri::command]
async fn save_workspace_setting(path: String) {
    let mut s = settings::load();
    s.workspace = Some(path);
    settings::save(&s);
}

#[tauri::command]
async fn read_file(path: String) -> Result<String, String> {
    tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_ollama() -> Result<(), String> {
    let client = reqwest::Client::new();
    client
        .get("http://localhost:11434/api/tags")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_models() -> Result<Vec<String>, String> {
    let model = get_model().lock().await.clone();
    let llm = LlmClient::new("http://localhost:11434", &model);
    llm.list_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_workspace_files(root: String) -> Result<Vec<FileNode>, String> {
    list_files(&root, 4).map_err(|e| e.to_string())
}

#[tauri::command]
async fn run_agent(task: AgentTask, app: tauri::AppHandle) -> Result<(), String> {
    let model = get_model().lock().await.clone();
    let llm = LlmClient::new("http://localhost:11434", &model);
    let mut agent = AgentLoop::new(llm, app);
    agent.run(task).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn apply_patch_cmd(diff: String, workspace_root: String) -> Result<Vec<String>, String> {
    let engine = PatchEngine::new(&workspace_root);
    engine.apply(&diff).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn index_workspace(root: String) -> Result<usize, String> {
    let indexer = WorkspaceIndexer::new(&root);
    let mut store = VectorStore::open(&root).map_err(|e| e.to_string())?;
    indexer
        .index_all(&mut store)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn search_workspace(
    root: String,
    query: String,
    top_k: usize,
) -> Result<Vec<SearchResult>, String> {
    let store = VectorStore::open(&root).map_err(|e| e.to_string())?;
    store.search(&query, top_k).map_err(|e| e.to_string())
}

#[tauri::command]
async fn index_stats(root: String) -> Result<usize, String> {
    let store = VectorStore::open(&root).map_err(|e| e.to_string())?;
    store.stats().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_project_facts(root: String, category: String) -> Result<Vec<ProjectFact>, String> {
    let mem = LongTermMemory::open(&root).map_err(|e| e.to_string())?;
    mem.facts_by_category(&category).map_err(|e| e.to_string())
}

#[tauri::command]
async fn store_project_fact(
    root: String,
    key: String,
    value: String,
    category: String,
) -> Result<(), String> {
    let mem = LongTermMemory::open(&root).map_err(|e| e.to_string())?;
    mem.store_fact(&key, &value, &category)
        .map_err(|e| e.to_string())
}

/// Called by the frontend when user approves or denies a ConfirmAction request.
#[tauri::command]
fn confirm_action(id: String, approved: bool) {
    resolve_confirm(&id, approved);
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();
            // 1. Auto-setup: wait for frontend to signal it's ready, then run ONCE
            let setup_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
                let tx = std::sync::Arc::new(std::sync::Mutex::new(Some(tx)));
                let _unlisten = setup_handle.listen_global("setup-ready", move |_| {
                    if let Ok(mut guard) = tx.lock() {
                        if let Some(tx) = guard.take() {
                            let _ = tx.send(());
                        }
                    }
                });
                tokio::select! {
                    _ = &mut rx => {}
                    _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {}
                }
                // Only run setup once per app lifetime — skip on frontend reload
                if !SETUP_DONE.load(std::sync::atomic::Ordering::SeqCst) {
                    setup::run(setup_handle.clone()).await;
                    SETUP_DONE.store(true, std::sync::atomic::Ordering::SeqCst);
                } else {
                    // Frontend reloaded — just tell it setup is already done
                    setup_handle
                        .emit_all(
                            "setup-progress",
                            serde_json::json!({
                                "step": "ready", "detail": "", "done": true, "error": null
                            }),
                        )
                        .ok();
                }
            });
            // 2. VSCode WebSocket bridge
            let bridge_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = bridge::websocket::start_bridge(43210, bridge_handle).await {
                    eprintln!("Bridge error: {}", e);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            set_workspace,
            set_model,
            read_file,
            check_ollama,
            get_models,
            list_workspace_files,
            run_agent,
            apply_patch_cmd,
            index_workspace,
            search_workspace,
            index_stats,
            get_project_facts,
            store_project_fact,
            load_settings,
            save_workspace_setting,
            confirm_action,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Moses");
}
