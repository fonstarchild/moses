use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AgentTask {
    pub prompt: String,
    pub workspace_root: String,
    pub open_files: Vec<String>,
    pub mode: String,
}
