use serde::{Deserialize, Serialize};
/// Automatic setup — runs on app launch.
/// Ensures Ollama is installed, running, and has at least one model.
/// Emits "setup-progress" events to the frontend throughout.
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupProgress {
    pub step: String,   // human-readable current step
    pub detail: String, // sub-detail or progress line
    pub done: bool,     // true = setup complete
    pub error: Option<String>,
}

impl SetupProgress {
    fn emit(app: &AppHandle, step: &str, detail: &str) {
        app.emit_all(
            "setup-progress",
            Self {
                step: step.into(),
                detail: detail.into(),
                done: false,
                error: None,
            },
        )
        .ok();
    }

    fn complete(app: &AppHandle) {
        app.emit_all(
            "setup-progress",
            Self {
                step: "Ready".into(),
                detail: "Moses is ready".into(),
                done: true,
                error: None,
            },
        )
        .ok();
    }

    fn fail(app: &AppHandle, step: &str, err: &str) {
        app.emit_all(
            "setup-progress",
            Self {
                step: step.into(),
                detail: String::new(),
                done: false,
                error: Some(err.into()),
            },
        )
        .ok();
    }
}

/// Entry point — call this from main `setup` closure.
pub async fn run(app: AppHandle) {
    // Step 1: ensure Ollama binary exists
    SetupProgress::emit(
        &app,
        "Checking Ollama",
        "Looking for Ollama on your system…",
    );
    let ollama = match find_or_install_ollama(&app).await {
        Ok(p) => p,
        Err(e) => {
            SetupProgress::fail(&app, "Ollama install failed", &e.to_string());
            return;
        }
    };

    // Step 2: ensure Ollama server is running
    SetupProgress::emit(&app, "Starting Ollama", "Starting the local model server…");
    if let Err(e) = ensure_ollama_running(&ollama).await {
        SetupProgress::fail(&app, "Ollama server failed", &e.to_string());
        return;
    }

    // Step 3: ensure at least one usable model is present
    SetupProgress::emit(&app, "Checking models", "Looking for installed models…");
    if let Err(e) = ensure_model(&app, &ollama).await {
        SetupProgress::fail(&app, "Model download failed", &e.to_string());
        return;
    }

    // Step 4: ensure embedding model
    SetupProgress::emit(&app, "Checking embed model", "Checking nomic-embed-text…");
    pull_if_missing(&app, &ollama, "nomic-embed-text", "Embedding model").await;

    SetupProgress::complete(&app);
}

// ── Ollama location ────────────────────────────────────────────────────────────

fn ollama_bin_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "ollama.exe"
    } else {
        "ollama"
    }
}

async fn find_or_install_ollama(app: &AppHandle) -> anyhow::Result<PathBuf> {
    // Our own downloaded copy takes priority
    let our_bin = crate::settings::moses_data_dir()
        .join("bin")
        .join(ollama_bin_name());
    if our_bin.exists() {
        return Ok(our_bin);
    }

    // Check well-known system locations
    #[cfg(target_os = "windows")]
    let candidates: &[&str] = &[
        r"C:\Program Files\Ollama\ollama.exe",
        r"C:\Users\Public\ollama\ollama.exe",
    ];
    #[cfg(target_os = "macos")]
    let candidates: &[&str] = &[
        "/usr/local/bin/ollama",
        "/usr/bin/ollama",
        "/opt/homebrew/bin/ollama",
        "/Applications/Ollama.app/Contents/Resources/ollama",
    ];
    #[cfg(target_os = "linux")]
    let candidates: &[&str] = &["/usr/local/bin/ollama", "/usr/bin/ollama"];

    for c in candidates {
        if Path::new(c).exists() {
            return Ok(PathBuf::from(c));
        }
    }

    // Check PATH (Unix: `which`, Windows: `where`)
    #[cfg(not(target_os = "windows"))]
    let which_cmd = ("which", "ollama");
    #[cfg(target_os = "windows")]
    let which_cmd = ("where", "ollama");

    if let Ok(out) = Command::new(which_cmd.0).arg(which_cmd.1).output().await {
        let p = String::from_utf8_lossy(&out.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !p.is_empty() && Path::new(&p).exists() {
            return Ok(PathBuf::from(p));
        }
    }

    // Not found — download it
    SetupProgress::emit(
        app,
        "Installing Ollama",
        "Downloading Ollama (one-time setup)…",
    );
    install_ollama(app).await
}

async fn install_ollama(app: &AppHandle) -> anyhow::Result<PathBuf> {
    let dest_dir = crate::settings::moses_data_dir().join("bin");
    tokio::fs::create_dir_all(&dest_dir).await?;
    let dest = dest_dir.join(ollama_bin_name());

    #[cfg(target_os = "macos")]
    let url = if cfg!(target_arch = "aarch64") {
        "https://github.com/ollama/ollama/releases/latest/download/ollama-darwin-arm64"
    } else {
        "https://github.com/ollama/ollama/releases/latest/download/ollama-darwin-amd64"
    };
    #[cfg(target_os = "linux")]
    let url = if cfg!(target_arch = "aarch64") {
        "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-arm64"
    } else {
        "https://github.com/ollama/ollama/releases/latest/download/ollama-linux-amd64"
    };
    #[cfg(target_os = "windows")]
    let url = "https://github.com/ollama/ollama/releases/latest/download/ollama-windows-amd64.exe";

    SetupProgress::emit(
        app,
        "Installing Ollama",
        &format!("Downloading from {url}…"),
    );

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    let bytes = client.get(url).send().await?.bytes().await?;
    tokio::fs::write(&dest, &bytes).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(&dest).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(&dest, perms).await?;
    }

    SetupProgress::emit(app, "Installing Ollama", "Ollama installed ✓");
    Ok(dest)
}

// ── Server ─────────────────────────────────────────────────────────────────────

async fn ensure_ollama_running(ollama: &Path) -> anyhow::Result<()> {
    // Check if already responding
    if ollama_responding().await {
        return Ok(());
    }

    // Spawn `ollama serve` detached
    Command::new(ollama)
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    // Wait up to 10s for it to come up
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if ollama_responding().await {
            return Ok(());
        }
    }

    anyhow::bail!("Ollama server did not start in time. Try running `ollama serve` manually.")
}

async fn ollama_responding() -> bool {
    reqwest::Client::new()
        .get("http://localhost:11434/api/tags")
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .is_ok()
}

// ── Models ─────────────────────────────────────────────────────────────────────

const PREFERRED_MODELS: &[&str] = &[
    "deepseek-coder-v2:16b",
    "deepseek-coder:6.7b",
    "deepseek-r1:14b",
    "deepseek-r1:7b",
    "codellama:13b",
    "llama3:8b",
];

const DEFAULT_MODEL: &str = "deepseek-coder:6.7b";

async fn ensure_model(app: &AppHandle, ollama: &Path) -> anyhow::Result<()> {
    let installed = list_installed_models().await;

    // Check if any preferred model is already present
    let has_preferred = installed.iter().any(|m| {
        PREFERRED_MODELS
            .iter()
            .any(|p| m.starts_with(p) || p.starts_with(m.as_str()))
    });

    if has_preferred {
        SetupProgress::emit(
            app,
            "Checking models",
            &format!("Found: {}", installed.join(", ")),
        );
        return Ok(());
    }

    // Nothing installed — pull the default
    SetupProgress::emit(
        app,
        "Downloading model",
        &format!("Pulling {DEFAULT_MODEL} (~3.8 GB, one-time download)…"),
    );

    pull_model_streaming(app, ollama, DEFAULT_MODEL, "AI model").await
}

async fn pull_if_missing(app: &AppHandle, ollama: &Path, model: &str, label: &str) {
    let installed = list_installed_models().await;
    if installed.iter().any(|m| m.starts_with(model)) {
        return;
    }
    pull_model_streaming(app, ollama, model, label).await.ok();
}

async fn pull_model_streaming(
    app: &AppHandle,
    ollama: &Path,
    model: &str,
    label: &str,
) -> anyhow::Result<()> {
    use futures_util::StreamExt;

    let url = "http://localhost:11434/api/pull";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3600))
        .build()?;

    let mut stream = client
        .post(url)
        .json(&serde_json::json!({ "name": model, "stream": true }))
        .send()
        .await?
        .bytes_stream();

    let mut buf = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buf.find('\n') {
            let line = buf[..pos].trim().to_string();
            buf = buf[pos + 1..].to_string();
            if line.is_empty() {
                continue;
            }

            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                let status = v["status"].as_str().unwrap_or("").to_string();
                let detail =
                    if let (Some(c), Some(t)) = (v["completed"].as_u64(), v["total"].as_u64()) {
                        if t > 0 {
                            let pct = (c as f64 / t as f64 * 100.0) as u64;
                            let mb_done = c / 1_048_576;
                            let mb_total = t / 1_048_576;
                            format!("{status} — {pct}% ({mb_done} / {mb_total} MB)")
                        } else {
                            status
                        }
                    } else {
                        status
                    };

                SetupProgress::emit(app, &format!("Downloading {label}"), &detail);
            }
        }
    }

    Ok(())
}

async fn list_installed_models() -> Vec<String> {
    #[derive(Deserialize)]
    struct TagsResp {
        models: Vec<ModelEntry>,
    }
    #[derive(Deserialize)]
    struct ModelEntry {
        name: String,
    }

    let Ok(resp) = reqwest::Client::new()
        .get("http://localhost:11434/api/tags")
        .timeout(Duration::from_secs(5))
        .send()
        .await
    else {
        return vec![];
    };

    resp.json::<TagsResp>()
        .await
        .map(|r| r.models.into_iter().map(|m| m.name).collect())
        .unwrap_or_default()
}
