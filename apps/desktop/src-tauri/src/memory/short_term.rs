use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub struct ConversationMemory {
    messages: Vec<ChatMessage>,
    max_tokens: usize,
}

impl ConversationMemory {
    pub fn new(max_tokens: usize) -> Self {
        Self { messages: vec![], max_tokens }
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "user".into(),
            content: content.to_string(),
        });
        self.trim();
    }

    pub fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(ChatMessage {
            role: "assistant".into(),
            content: content.to_string(),
        });
    }

    pub fn add_tool_result(&mut self, tool_name: &str, result: &str) {
        self.messages.push(ChatMessage {
            role: "user".into(),
            content: format!("[Tool result: {}]\n{}", tool_name, result),
        });
        self.trim();
    }

    pub fn to_messages(&self) -> Vec<ChatMessage> {
        self.messages.clone()
    }

    fn trim(&mut self) {
        // Rough token estimate: 4 chars per token
        let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();
        let approx_tokens = total_chars / 4;

        if approx_tokens > self.max_tokens {
            // Keep the last N messages
            let to_remove = self.messages.len() / 4;
            self.messages.drain(0..to_remove);
        }
    }
}
