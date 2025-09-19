use std::collections::BTreeMap;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use reqwest::Client;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::agent::config::McpConfig;
use crate::agent::{AgentEvent, AgentRequest, AgentResponse};

use super::AgentBackend;

/// `AgentBackend` 的 MCP (Model Communication Protocol) 實作。
///
/// MCP 是一個基於 HTTP 的自訂協定，用於與特定的代理後端進行結構化通訊。
/// 這個後端的架構與 `HttpBackend` 非常相似，同樣使用 MPSC channel 來處理非同步通訊。
pub struct McpBackend {
    client: Client,
    info: McpBackendInfo,
    tx: UnboundedSender<AgentEvent>,
    rx: UnboundedReceiver<AgentEvent>,
}

/// 儲存 MCP 後端連線所需的設定資訊。
#[derive(Clone)]
struct McpBackendInfo {
    endpoint: String,
    tool: Option<String>,
    headers: BTreeMap<String, String>,
    api_key: Option<String>,
}

impl McpBackend {
    /// 根據提供的設定建立一個新的 `McpBackend`。
    pub fn new(config: McpConfig) -> Result<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let api_key = config.resolved_api_key();
        let headers = config.headers.clone();
        Ok(Self {
            client: Client::new(),
            info: McpBackendInfo {
                endpoint: config.endpoint,
                tool: config.tool,
                headers,
                api_key,
            },
            tx,
            rx,
        })
    }
}

#[async_trait]
impl AgentBackend for McpBackend {
    fn name(&self) -> &str {
        "MCP"
    }

    /// 非同步地將請求傳送給 MCP 後端。
    ///
    /// 此方法會 `tokio::spawn` 一個新任務來處理 HTTP 請求，並透過 MPSC channel 傳回結果。
    async fn send(&mut self, request: AgentRequest) -> Result<()> {
        let info = self.info.clone();
        let client = self.client.clone();
        let tx = self.tx.clone();
        tokio::spawn(async move {
            if let Err(err) = dispatch_mcp(info, client, request, tx.clone()).await {
                let _ = tx.send(AgentEvent::Error(format!("MCP 呼叫失敗: {err}")));
            }
        });
        Ok(())
    }

    /// 從事件 channel 中輪詢一個事件。
    fn poll_event(&mut self) -> Option<AgentEvent> {
        self.rx.try_recv().ok()
    }
}

/// 處理 MCP 請求的分派。
async fn dispatch_mcp(
    info: McpBackendInfo,
    client: Client,
    request: AgentRequest,
    tx: UnboundedSender<AgentEvent>,
) -> Result<()> {
    // 根據 MCP 協定，將 AgentRequest 包裝在一個更大的 JSON 物件中。
    let mut body = serde_json::json!({
        "input": request,
    });
    if let Some(tool) = &info.tool {
        body["tool"] = serde_json::Value::String(tool.clone());
    }

    // 建構 HTTP 標頭。
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if let Some(key) = &info.api_key {
        let value = format!("Bearer {}", key);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&value)?);
    }
    for (key, value) in &info.headers {
        let header_name = HeaderName::from_bytes(key.as_bytes())?;
        headers.insert(header_name, HeaderValue::from_str(value)?);
    }

    // 發送 HTTP POST 請求。
    let response = client
        .post(&info.endpoint)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .context("無法呼叫 MCP 端點")?;

    if !response.status().is_success() {
        let text = response.text().await.unwrap_or_default();
        return Err(anyhow!("MCP 端點回傳錯誤: {}", text));
    }

    let payload: serde_json::Value = response.json().await.context("解析 MCP 回應失敗")?;

    // 解析 MCP 回應並將事件發送到 channel。
    parse_mcp_response(payload, tx);

    Ok(())
}

/// 解析來自 MCP 後端的回應 (payload)。
///
/// MCP 的回應格式比較靈活，此函式負責處理以下幾種主要情況：
/// 1. 包含 "responses" 陣列的物件。
/// 2. 代表工具輸出的物件。
/// 3. 單一的 `AgentResponse` 物件。
/// 4. `AgentResponse` 物件的陣列。
/// 5. 無法識別的格式，將被當作純文字處理。
fn parse_mcp_response(payload: serde_json::Value, tx: UnboundedSender<AgentEvent>) {
    if let Some(messages) = payload.get("responses").and_then(|v| v.as_array()) {
        // 情況 1：回應是一個包含 "responses" 陣列的物件。
        for message in messages {
            match serde_json::from_value::<AgentResponse>(message.clone()) {
                Ok(resp) => {
                    let _ = tx.send(AgentEvent::Response(resp));
                }
                Err(_) => {
                    let _ = tx.send(AgentEvent::Error(format!(
                        "MCP 回應格式不正確: {}",
                        message
                    )));
                }
            }
        }
    } else if let Some(tool) = payload.get("tool").and_then(|v| v.as_str()) {
        // 情況 2：回應是一個工具輸出。
        let detail = payload
            .get("detail")
            .or_else(|| payload.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("(無內容)");
        let _ = tx.send(AgentEvent::ToolOutput {
            tool: tool.to_string(),
            detail: detail.to_string(),
        });
    } else if payload.get("title").is_some() {
        // 情況 3：回應本身就是一個 AgentResponse 物件。
        let response: AgentResponse = serde_json::from_value(payload.clone())
            .unwrap_or_else(|_| AgentResponse::from_text("MCP 回應", payload.to_string()));
        let _ = tx.send(AgentEvent::Response(response));
    } else if let Some(items) = payload.as_array() {
        // 情況 4：回應是一個 AgentResponse 物件的陣列。
        for item in items {
            match serde_json::from_value::<AgentResponse>(item.clone()) {
                Ok(resp) => {
                    let _ = tx.send(AgentEvent::Response(resp));
                }
                Err(_) => {
                    let _ = tx.send(AgentEvent::Error(format!("MCP 回應格式不正確: {}", item)));
                }
            }
        }
    } else {
        // 情況 5：無法識別的格式，將整個回應作為純文字處理。
        let text = payload.to_string();
        let _ = tx.send(AgentEvent::Response(AgentResponse::from_text(
            "MCP 回應",
            text,
        )));
    }
}
