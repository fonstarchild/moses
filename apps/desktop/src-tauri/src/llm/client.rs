use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use crate::memory::short_term::ChatMessage;

pub struct LlmClient {
    client: Client,
    pub base_url: String,
    pub model: String,
}

#[derive(Debug)]
pub enum LlmResponse {
    Text(String),
    ToolCall { name: String, args: Value },
}

impl LlmClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .unwrap(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    /// Prompt-based tool calling — works with any model.
    /// Tool definitions are injected into the system prompt.
    /// The model is expected to emit tool calls as:
    ///   <tool_call>{"name": "...", "args": {...}}</tool_call>
    pub async fn chat_with_tools(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: &[Value],
        on_token: impl Fn(String) + Send + 'static,
    ) -> Result<LlmResponse, anyhow::Error> {
        let system_with_tools = build_system_with_tools(system, tools);

        // Stream the response, collecting full text
        let text = self.stream_text(&system_with_tools, messages, on_token).await?;

        // Check if the model emitted a tool call block
        if let Some(tc) = parse_tool_call(&text) {
            return Ok(tc);
        }

        Ok(LlmResponse::Text(text))
    }

    /// Stream a text response, calling on_token for each chunk.
    /// Returns the full accumulated text.
    pub async fn stream_text(
        &self,
        system: &str,
        messages: &[ChatMessage],
        on_token: impl Fn(String),
    ) -> Result<String, anyhow::Error> {
        let body = json!({
            "model": self.model,
            "system": system,
            "messages": messages,
            "stream": true,
            "options": {
                "num_ctx": 32768,
                "temperature": 0.1,
                "top_p": 0.9,
                "num_predict": 8192,
            }
        });

        let response = self.client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let err = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama error {}: {}", status, err);
        }

        let mut stream = response.bytes_stream();
        let mut full_text = String::new();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].trim().to_string();
                buf = buf[pos + 1..].to_string();
                if line.is_empty() { continue; }

                if let Ok(event) = serde_json::from_str::<StreamEvent>(&line) {
                    let token = strip_special_tokens(event.message.content);
                    if !token.is_empty() {
                        on_token(token.clone());
                        full_text.push_str(&token);
                    }
                    if event.done {
                        return Ok(full_text);
                    }
                }
            }
        }

        Ok(full_text)
    }

    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, anyhow::Error> {
        let body = json!({
            "model": "nomic-embed-text",
            "prompt": text,
        });

        let resp = self.client
            .post(format!("{}/api/embeddings", self.base_url))
            .json(&body)
            .send()
            .await?
            .json::<EmbedResponse>()
            .await?;

        Ok(resp.embedding)
    }

    pub async fn list_models(&self) -> Result<Vec<String>, anyhow::Error> {
        #[derive(Deserialize)]
        struct TagsResponse { models: Vec<ModelEntry> }
        #[derive(Deserialize)]
        struct ModelEntry { name: String }

        let resp = self.client
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?
            .json::<TagsResponse>()
            .await?;

        Ok(resp.models.into_iter().map(|m| m.name).collect())
    }
}

/// Inject tool definitions into the system prompt so any model can use them.
fn build_system_with_tools(system: &str, tools: &[Value]) -> String {
    if tools.is_empty() {
        return system.to_string();
    }

    let tools_desc: Vec<String> = tools.iter().map(|t| {
        let name = t["function"]["name"].as_str().unwrap_or("unknown");
        let desc = t["function"]["description"].as_str().unwrap_or("");
        let params = &t["function"]["parameters"];
        format!("- {}: {}\n  Parameters: {}", name, desc, params)
    }).collect();

    format!(
        "{}\n\n## Available Tools\nYou can call tools by emitting EXACTLY this format (nothing before or after on the same line):\n<tool_call>{{\"name\": \"tool_name\", \"args\": {{...}}}}</tool_call>\n\nTools:\n{}\n\nOnly call ONE tool at a time. After receiving the result, continue reasoning.",
        system,
        tools_desc.join("\n")
    )
}

/// Parse a <tool_call>{...}</tool_call> block from model output.
fn parse_tool_call(text: &str) -> Option<LlmResponse> {
    let start = text.find("<tool_call>")?;
    let rest = &text[start + 11..];
    let end = rest.find("</tool_call>")?;
    let json_str = rest[..end].trim();

    let v: Value = serde_json::from_str(json_str).ok()?;
    let name = v["name"].as_str()?.to_string();
    let args = v["args"].clone();

    Some(LlmResponse::ToolCall { name, args })
}

/// Remove DeepSeek special tokens that sometimes leak into output.
fn strip_special_tokens(s: String) -> String {
    // These are DeepSeek tokenizer boundary markers
    const JUNK: &[&str] = &[
        "<｜begin▁of▁sentence｜>",
        "<｜end▁of▁sentence｜>",
        "<｜fim▁begin｜>",
        "<｜fim▁end｜>",
        "<｜fim▁hole｜>",
        "<|begin_of_sentence|>",
        "<|end_of_sentence|>",
    ];
    let mut out = s;
    for pat in JUNK {
        if out.contains(pat) {
            out = out.replace(pat, "");
        }
    }
    out
}

#[derive(Deserialize)]
struct StreamEvent {
    message: StreamMessage,
    #[serde(default)]
    done: bool,
}

#[derive(Deserialize)]
struct StreamMessage {
    content: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: Vec<f32>,
}
