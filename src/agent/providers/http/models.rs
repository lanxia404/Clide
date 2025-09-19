//! This module contains the request and response structures for various HTTP-based AI providers.
//！此模組包含各個基於 HTTP 的 AI 提供商的請求和回應結構。

// --- OpenAI ---
#[derive(serde::Serialize)]
pub struct OpenAiMessage<'a> {
    pub role: &'a str,
    pub content: &'a str,
}

#[derive(serde::Serialize)]
pub struct OpenAiChatPayload<'a> {
    pub model: &'a str,
    pub messages: Vec<OpenAiMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct OpenAiChatResponse {
    pub choices: Vec<OpenAiChatChoice>,
}

#[derive(serde::Deserialize)]
pub struct OpenAiChatChoice {
    pub message: OpenAiChatMessage,
}

#[derive(serde::Deserialize)]
pub struct OpenAiChatMessage {
    pub content: String,
}

// --- Gemini ---
#[derive(serde::Serialize)]
pub struct GeminiPart<'a> {
    pub text: &'a str,
}

#[derive(serde::Serialize)]
pub struct GeminiContent<'a> {
    pub role: &'a str,
    pub parts: Vec<GeminiPart<'a>>,
}

#[derive(serde::Serialize)]
pub struct GeminiPayload<'a> {
    pub contents: Vec<GeminiContent<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiContent<'a>>,
}

#[derive(serde::Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<GeminiCandidate>,
}

#[derive(serde::Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiCandidateContent,
}

#[derive(serde::Deserialize)]
pub struct GeminiCandidateContent {
    pub parts: Vec<GeminiPartOwned>,
}

#[derive(serde::Deserialize)]
pub struct GeminiPartOwned {
    pub text: Option<String>,
}

// --- Anthropic ---
#[derive(serde::Serialize)]
pub struct AnthropicMessage<'a> {
    pub role: &'a str,
    pub content: Vec<AnthropicPart<'a>>,
}

#[derive(serde::Serialize)]
pub struct AnthropicPart<'a> {
    pub r#type: &'a str,
    pub text: &'a str,
}

#[derive(serde::Serialize)]
pub struct AnthropicPayload<'a> {
    pub model: &'a str,
    pub max_tokens: u32,
    pub messages: Vec<AnthropicMessage<'a>>,
}

#[derive(serde::Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<AnthropicPartOwned>,
}

#[derive(serde::Deserialize)]
pub struct AnthropicPartOwned {
    pub text: String,
}

// --- Ollama ---
#[derive(serde::Serialize)]
pub struct OllamaPayload<'a> {
    pub model: &'a str,
    pub prompt: &'a str,
    pub stream: bool,
}

#[derive(serde::Deserialize)]
pub struct OllamaResponse {
    pub response: String,
}

// --- Llama.cpp ---
#[derive(serde::Serialize)]
pub struct LlamaPayload<'a> {
    pub prompt: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

#[derive(serde::Deserialize)]
pub struct LlamaResponse {
    pub content: String,
}
