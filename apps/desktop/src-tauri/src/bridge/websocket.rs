use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BridgeIn {
    Prompt { text: String, workspace: String, open_files: Vec<String>, mode: Option<String> },
    AcceptPatch { diff: String },
    RejectPatch,
    Cancel,
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BridgeOut {
    Token { content: String },
    PatchProposal { diff: String, files: Vec<String> },
    ToolActivity { name: String, status: String },
    Done,
    Error { message: String },
    Pong,
    Connected { version: String },
}

pub async fn start_bridge(port: u16, app: AppHandle) -> Result<(), anyhow::Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Moses WS bridge: port {} unavailable ({}), skipping bridge", port, e);
            return Ok(()); // non-fatal — desktop app works fine without VSCode bridge
        }
    };
    println!("Moses WS bridge listening on ws://127.0.0.1:{}", port);

    // channel to broadcast agent events to all connected VSCode clients
    let (tx, _) = broadcast::channel::<BridgeOut>(256);
    let tx = Arc::new(tx);

    // subscribe to tauri agent events and forward them to bridge
    let tx_clone = tx.clone();
    let app_clone = app.clone();
    app_clone.listen_global("agent-event", move |event| {
        if let Some(payload) = event.payload() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) {
                let out = match v["type"].as_str() {
                    Some("StreamToken") => Some(BridgeOut::Token {
                        content: v["token"].as_str().unwrap_or_default().to_string(),
                    }),
                    Some("PatchProposed") => {
                        let files = v["files"].as_array()
                            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                            .unwrap_or_default();
                        Some(BridgeOut::PatchProposal {
                            diff: v["diff"].as_str().unwrap_or_default().to_string(),
                            files,
                        })
                    },
                    Some("ToolCall") => Some(BridgeOut::ToolActivity {
                        name: v["name"].as_str().unwrap_or_default().to_string(),
                        status: "running".to_string(),
                    }),
                    Some("Done") => Some(BridgeOut::Done),
                    Some("Error") => Some(BridgeOut::Error {
                        message: v["message"].as_str().unwrap_or_default().to_string(),
                    }),
                    _ => None,
                };
                if let Some(msg) = out {
                    tx_clone.send(msg).ok();
                }
            }
        }
    });

    while let Ok((stream, peer)) = listener.accept().await {
        let tx = tx.clone();
        let app = app.clone();
        tokio::spawn(handle_client(stream, peer, tx, app));
    }

    Ok(())
}

async fn handle_client(
    stream: TcpStream,
    peer: SocketAddr,
    tx: Arc<broadcast::Sender<BridgeOut>>,
    app: AppHandle,
) {
    println!("VSCode connected: {}", peer);

    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("WS handshake error: {}", e);
            return;
        }
    };

    let (mut ws_tx, mut ws_rx) = ws.split();
    let mut rx = tx.subscribe();

    // Send welcome
    let welcome = serde_json::to_string(&BridgeOut::Connected {
        version: "0.1.0".to_string(),
    }).unwrap();
    ws_tx.send(Message::Text(welcome)).await.ok();

    // Forward agent events → VSCode
    let forward = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_tx.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Receive VSCode → dispatch to agent
    while let Some(Ok(msg)) = ws_rx.next().await {
        if let Message::Text(text) = msg {
            if let Ok(bridge_msg) = serde_json::from_str::<BridgeIn>(&text) {
                match bridge_msg {
                    BridgeIn::Prompt { text, workspace, open_files, mode } => {
                        use crate::agent::task::AgentTask;
                        let task = AgentTask {
                            prompt: text,
                            workspace_root: workspace,
                            open_files,
                            mode: mode.unwrap_or_else(|| "Chat".to_string()),
                        };
                        let app_clone = app.clone();
                        tokio::spawn(async move {
                            use crate::llm::client::LlmClient;
                            use crate::agent::loop_::AgentLoop;
                            let llm = LlmClient::new("http://localhost:11434", "deepseek-coder:6.7b");
                            let mut agent = AgentLoop::new(llm, app_clone);
                            agent.run(task).await.ok();
                        });
                    }
                    BridgeIn::AcceptPatch { diff: _ } => {
                        // Patch acceptance is handled by the desktop app UI
                        // The VSCode extension sends this to confirm
                        println!("VSCode accepted patch");
                    }
                    BridgeIn::Ping => {
                        tx.send(BridgeOut::Pong).ok();
                    }
                    _ => {}
                }
            }
        }
    }

    forward.abort();
    println!("VSCode disconnected: {}", peer);
}
