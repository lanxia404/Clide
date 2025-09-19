use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::agent::config::{HttpApiConfig, HttpProvider};
use crate::agent::{AgentEvent, AgentRequest, AgentResponse};

use super::AgentBackend;

mod models;

/// `AgentBackend` 的 HTTP 實作，用於與遠端 AI 服務進行通訊。
pub struct HttpBackend {
    /// 存放後端設定資訊，方便在非同步任務中複製和傳遞。
    info: HttpBackendInfo,
    /// `reqwest` 的非同步 HTTP 客戶端。
    client: Client,
    /// 一個 MPSC (多生產者，單消費者) channel 的發送端，用於將非同步任務的結果傳回主執行緒。
    events_tx: UnboundedSender<AgentEvent>,
    /// MPSC channel 的接收端，`poll_event` 會從這裡接收事件。
    events_rx: UnboundedReceiver<AgentEvent>,
}

/// 一個輔助結構，用於儲存特定後端的設定，方便在非同步任務中安全地傳遞。
#[derive(Clone)]
struct HttpBackendInfo {
    provider: HttpProvider,
    base_url: String,
    model: Option<String>,
    api_key: Option<String>,
    system_prompt: Option<String>,
    headers: BTreeMap<String, String>,
}

impl HttpBackend {
    /// 根據提供的設定建立一個新的 `HttpBackend`。
    pub fn new(config: HttpApiConfig) -> Result<Self> {
        let api_key = config.resolved_api_key();
        let base_url = if let Some(url) = config.base_url.clone() {
            url
        } else {
            default_base_url(&config.provider, config.model.as_deref())?
        };

        if api_key.is_none() && requires_api_key(&config.provider) {
            return Err(anyhow!("未提供必要的 API 金鑰"));
        }

        // 建立一個無邊界的 MPSC channel，用於在非同步任務和主執行緒之間傳遞事件。
        let (tx, rx) = mpsc::unbounded_channel();
        Ok(Self {
            info: HttpBackendInfo {
                provider: config.provider,
                base_url,
                model: config.model,
                api_key,
                system_prompt: config.system_prompt,
                headers: config.extra_headers,
            },
            client: Client::new(),
            events_tx: tx,
            events_rx: rx,
        })
    }
}

#[async_trait]
impl AgentBackend for HttpBackend {
    fn name(&self) -> &str {
        self.info.provider.display_name()
    }

    /// 非同步地將請求傳送給後端。
    ///
    /// 這個方法不會阻塞。它會立即 `tokio::spawn` 一個新的非同步任務來處理實際的 HTTP 請求。
    /// 該任務完成後，會透過 MPSC channel 將結果（成功或失敗）傳送回來。
    async fn send(&mut self, request: AgentRequest) -> Result<()> {
        let tx = self.events_tx.clone();
        let info = self.info.clone();
        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(err) = dispatch_request(info, client, request, tx.clone()).await {
                let _ = tx.send(AgentEvent::Error(format!("HTTP 後端錯誤: {err}")));
            }
        });
        Ok(())
    }

    /// 從事件 channel 中輪詢一個事件。
    ///
    /// 這是一個非阻塞操作，它嘗試從 MPSC channel 接收一個由非同步任務傳回的事件。
    /// 如果沒有事件，它會立即回傳 `None`。
    fn poll_event(&mut self) -> Option<AgentEvent> {
        self.events_rx.try_recv().ok()
    }
}

/// 檢查指定的供應商是否需要 API 金鑰。
fn requires_api_key(provider: &HttpProvider) -> bool {
    matches!(
        provider,
        HttpProvider::OpenAi
            | HttpProvider::Codex
            | HttpProvider::Gemini
            | HttpProvider::Anthropic
            | HttpProvider::AzureOpenAi
    )
}

/// 根據供應商類型回傳預設的 API 基礎 URL。
fn default_base_url(provider: &HttpProvider, model: Option<&str>) -> Result<String> {
    Ok(match provider {
        HttpProvider::OpenAi | HttpProvider::Codex | HttpProvider::Vllm => {
            "https://api.openai.com/v1/chat/completions".into()
        }
        HttpProvider::Gemini => {
            let model = model.unwrap_or("gemini-1.5-flash");
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
                model
            )
        }
        HttpProvider::Anthropic => "https://api.anthropic.com/v1/messages".into(),
        HttpProvider::AzureOpenAi => {
            return Err(anyhow!("Azure OpenAI 需在設定中指定 base_url"));
        }
        HttpProvider::Ollama => "http://localhost:11434/api/generate".into(),
        HttpProvider::LlamaCpp => "http://localhost:8080/completion".into(),
        HttpProvider::Custom => {
            return Err(anyhow!("自訂 HTTP 後端需指定 base_url"));
        }
    })
}

/// 根據供應商類型，將請求分派給對應的處理函式。
async fn dispatch_request(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    match info.provider {
        HttpProvider::OpenAi | HttpProvider::Codex | HttpProvider::Vllm => {
            handle_openai_like(info, client, request, tx).await
        }
        HttpProvider::Gemini => handle_gemini(info, client, request, tx).await,
        HttpProvider::Anthropic => handle_anthropic(info, client, request, tx).await,
        HttpProvider::AzureOpenAi => handle_azure_openai(info, client, request, tx).await,
        HttpProvider::Ollama => handle_ollama(info, client, request, tx).await,
        HttpProvider::LlamaCpp => handle_llama_cpp(info, client, request, tx).await,
        HttpProvider::Custom => handle_custom(info, client, request, tx).await,
    }
}

/// 根據 `AgentRequest` 的內容，建構一個通用的提示詞字串。
fn build_prompt(request: &AgentRequest) -> String {
    let mut prompt = String::new();
    if let Some(path) = &request.file_path {
        prompt.push_str(&format!("目標檔案：{}\n", path));
    }
    prompt.push_str(&format!(
        "游標位置：第 {} 行，第 {} 欄。\n",
        request.cursor_line + 1,
        request.cursor_col + 1
    ));
    if let Some(selection) = &request.selection
        && !selection.is_empty() {
            prompt.push_str("目前選取內容：\n");
            prompt.push_str(selection);
            prompt.push_str("\n\n");
        }
    prompt.push_str("請根據下列完整程式內容提供建議：\n");
    prompt.push_str(&request.content);
    prompt
}

/// 根據後端資訊建構 HTTP 標頭。
fn build_headers(info: &HttpBackendInfo) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(api_key) = &info.api_key {
        match info.provider {
            HttpProvider::AzureOpenAi => {
                headers.insert("api-key", HeaderValue::from_str(api_key)?);
            }
            HttpProvider::Gemini => {} // Gemini 的金鑰在 URL 參數中
            _ => {
                let value = format!("Bearer {}", api_key);
                headers.insert(AUTHORIZATION, HeaderValue::from_str(&value)?);
            }
        }
    }
    for (key, value) in info.headers.iter() {
        let header_name = HeaderName::from_bytes(key.as_bytes())?;
        headers.insert(header_name, HeaderValue::from_str(value)?);
    }
    Ok(headers)
}

/// 處理與 OpenAI API 相容的請求（包括 OpenAI, vLLM）。
async fn handle_openai_like(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    use models::{OpenAiChatPayload, OpenAiChatResponse, OpenAiMessage};

    let model = info
        .model
        .as_deref()
        .unwrap_or(match info.provider {
            HttpProvider::Vllm => "gpt-3.5-turbo",
            _ => "gpt-4o-mini",
        });
    let prompt = build_prompt(&request);
    let system_prompt = info
        .system_prompt
        .as_deref()
        .unwrap_or("你是一個協助開發者分析程式碼並提出補強建議的 AI 助理。");

    let payload = OpenAiChatPayload {
        model,
        messages: vec![
            OpenAiMessage {
                role: "system",
                content: system_prompt,
            },
            OpenAiMessage {
                role: "user",
                content: &prompt,
            },
        ],
        response_format: None,
    };

    let headers = build_headers(&info)?;
    let response = client
        .post(info.base_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .context("OpenAI API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("OpenAI 回應錯誤: {}", text));
    }

    let data: OpenAiChatResponse = response.json().await.context("解析 OpenAI 回應失敗")?;
    if let Some(choice) = data.choices.into_iter().next() {
        let mut agent_response =
            AgentResponse::from_text("模型建議", choice.message.content.trim());
        agent_response.raw = Some(serde_json::Value::String("openai".into()));
        let _ = tx.send(AgentEvent::Response(agent_response));
    } else {
        return Err(anyhow!("OpenAI 回應未包含建議"));
    }
    Ok(())
}

/// 處理 Google Gemini API 的請求。
async fn handle_gemini(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    use models::{GeminiContent, GeminiPart, GeminiPayload, GeminiResponse};

    let prompt = build_prompt(&request);
    let system_prompt = info
        .system_prompt
        .clone()
        .unwrap_or_else(|| "你是一個協助開發者分析程式碼的 AI 助理".into());

    let payload = GeminiPayload {
        contents: vec![GeminiContent {
            role: "user",
            parts: vec![GeminiPart { text: &prompt }],
        }],
        system_instruction: Some(GeminiContent {
            role: "system",
            parts: vec![GeminiPart {
                text: &system_prompt,
            }],
        }),
    };

    let mut url = info.base_url.clone();
    if let Some(api_key) = &info.api_key {
        if !url.contains('?') {
            url.push_str("?key=");
            url.push_str(api_key);
        } else {
            url.push_str("&key=");
            url.push_str(api_key);
        }
    }

    let headers = build_headers(&info)?;
    let response = client
        .post(url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .context("Gemini API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Gemini 回應錯誤: {}", text));
    }

    let data: GeminiResponse = response.json().await.context("解析 Gemini 回應失敗")?;
    if let Some(candidate) = data.candidates.into_iter().next() {
        if let Some(part) = candidate
            .content
            .parts
            .into_iter()
            .find_map(|part| part.text)
        {
            let mut agent_response = AgentResponse::from_text("Gemini", part.trim());
            agent_response.raw = Some(serde_json::json!({"provider": "gemini"}));
            let _ = tx.send(AgentEvent::Response(agent_response));
        } else {
            return Err(anyhow!("Gemini 未返回文字建議"));
        }
    } else {
        return Err(anyhow!("Gemini 回應未包含建議"));
    }
    Ok(())
}

/// 處理 Anthropic Claude API 的請求。
async fn handle_anthropic(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    use models::{AnthropicMessage, AnthropicPart, AnthropicPayload, AnthropicResponse};

    let model = info.model.as_deref().unwrap_or("claude-3-haiku-20240307");
    let prompt = build_prompt(&request);
    let system_prompt = info
        .system_prompt
        .as_deref()
        .unwrap_or("你是一個協助分析程式碼的 AI 助理");

    let payload = AnthropicPayload {
        model,
        max_tokens: 1024,
        messages: vec![
            AnthropicMessage {
                role: "system",
                content: vec![AnthropicPart {
                    r#type: "text",
                    text: system_prompt,
                }],
            },
            AnthropicMessage {
                role: "user",
                content: vec![AnthropicPart {
                    r#type: "text",
                    text: &prompt,
                }],
            },
        ],
    };

    let mut headers = build_headers(&info)?;
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

    let response = client
        .post(info.base_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .context("Anthropic API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Anthropic 回應錯誤: {}", text));
    }

    let data: AnthropicResponse = response.json().await.context("解析 Anthropic 回應失敗")?;
    if let Some(part) = data.content.into_iter().next() {
        let response = AgentResponse::from_text("Claude 建議", part.text.trim());
        let _ = tx.send(AgentEvent::Response(response));
    } else {
        return Err(anyhow!("Anthropic 回應未包含建議"));
    }
    Ok(())
}

/// 處理 Azure OpenAI API 的請求。
async fn handle_azure_openai(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    // Azure OpenAI 介面與 OpenAI 相同，但授權標頭不同（已在 `build_headers` 中處理），
    // 並且 base_url 需包含部署名稱。因此可以直接重用 `handle_openai_like`。
    handle_openai_like(info, client, request, tx).await
}

/// 處理本地 Ollama 服務的請求。
async fn handle_ollama(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    use models::{OllamaPayload, OllamaResponse};

    let model = info.model.as_deref().unwrap_or("llama3");
    let prompt = build_prompt(&request);
    let payload = OllamaPayload {
        model,
        prompt: &prompt,
        stream: false,
    };

    let response = client
        .post(info.base_url)
        .json(&payload)
        .send()
        .await
        .context("Ollama API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("Ollama 回應錯誤: {}", text));
    }

    let body: OllamaResponse = response.json().await.context("解析 Ollama 回應失敗")?;
    let response = AgentResponse::from_text("Ollama 建議", body.response.trim());
    let _ = tx.send(AgentEvent::Response(response));
    Ok(())
}

/// 處理本地 llama.cpp 服務的請求。
async fn handle_llama_cpp(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    use models::{LlamaPayload, LlamaResponse};

    let prompt = build_prompt(&request);
    let payload = LlamaPayload {
        prompt: &prompt,
        n_predict: Some(512),
        temperature: Some(0.2),
    };

    let response = client
        .post(info.base_url)
        .json(&payload)
        .send()
        .await
        .context("llama.cpp API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("llama.cpp 回應錯誤: {}", text));
    }

    let body: LlamaResponse = response.json().await.context("解析 llama.cpp 回應失敗")?;
    let response = AgentResponse::from_text("llama.cpp 建議", body.content.trim());
    let _ = tx.send(AgentEvent::Response(response));
    Ok(())
}

/// 處理自訂 HTTP 後端的請求。
async fn handle_custom(
    info: HttpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    let payload = serde_json::to_value(&request).context("序列化請求失敗")?;
    let headers = build_headers(&info)?;

    let response = client
        .post(info.base_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .context("自訂 HTTP API 呼叫失敗")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("自訂 HTTP 回應錯誤: {}", text));
    }

    // 支援串流（ndjson）或單次回應。
    if let Some(content_type) = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        && (content_type.contains("ndjson") || content_type.contains("stream")) {
            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                if let Ok(text) = std::str::from_utf8(&chunk) {
                    for line in text.lines() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<AgentResponse>(line) {
                            Ok(resp) => {
                                let _ = tx.send(AgentEvent::Response(resp));
                            }
                            Err(_) => {
                                let _ = tx.send(AgentEvent::Error(format!(
                                    "自訂 HTTP 無法解析: {}",
                                    line
                                )));
                            }
                        }
                    }
                }
            }
            return Ok(());
        }

    // 處理單一 JSON 回應。
    let json: serde_json::Value = response.json().await.context("解析自訂 HTTP 回應失敗")?;
    let agent_response = if json.get("title").is_some() {
        serde_json::from_value::<AgentResponse>(json.clone()).unwrap_or_else(|_| {
            let detail = json.to_string();
            AgentResponse::from_text("HTTP 回應", detail)
        })
    } else {
        AgentResponse::from_text("HTTP 回應", json.to_string())
    };
    let _ = tx.send(AgentEvent::Response(agent_response));
    Ok(())
}
