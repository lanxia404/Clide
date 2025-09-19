//! `providers` 模組負責提供與不同類型代理後端進行通訊的具體實作。
//!
//! 每個子模組（如 `http`, `local_process`）都實現了 `AgentBackend` trait，
//! 抽象化了與特定代理通訊的細節。

// --- 子模組宣告 ---

/// `http` 模組：提供透過 HTTP/HTTPS API 與遠端代理（如 OpenAI, Ollama）通訊的後端實作。
pub mod http;
/// `local_process` 模組：提供與本地執行的子程序（透過 stdin/stdout）進行通訊的後端實作。
pub mod local_process;
/// `mcp` 模組：提供透過 MCP 協定與代理通訊的後端實作。
pub mod mcp;

use anyhow::Result;
use async_trait::async_trait;

use crate::agent::{AgentEvent, AgentRequest};

/// 定義了所有代理後端都必須遵守的通用行為介面。
///
/// 這個 trait 使用了 `#[async_trait]` 宏，允許在 trait 中定義非同步函式。
/// `Send` marker trait 約束表示實現此 trait 的類型可以安全地在執行緒之間傳遞。
#[async_trait]
pub trait AgentBackend: Send {
    /// 回傳此後端的名稱，用於 UI 顯示或日誌記錄。
    fn name(&self) -> &str;

    /// 非同步地將一個 `AgentRequest` 傳送給代理。
    ///
    /// # Arguments
    /// * `request` - 要傳送給代理的請求物件。
    ///
    /// # Returns
    /// * `Result<()>` - 如果傳送成功則回傳 `Ok(())`，否則回傳錯誤。
    async fn send(&mut self, request: AgentRequest) -> Result<()>;

    /// 從後端輪詢一個 `AgentEvent`。
    ///
    /// 這是一個非阻塞函式。如果後端的事件佇列中有事件，則應回傳 `Some(AgentEvent)`。
    /// 如果沒有事件，則應立即回傳 `None`。
    ///
    /// # Returns
    /// * `Option<AgentEvent>` - 如果有可用的事件則回傳 `Some`，否則回傳 `None`。
    fn poll_event(&mut self) -> Option<AgentEvent>;
}
