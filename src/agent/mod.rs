//! 代理執行與資料結構骨架。
//!
//! 此模組負責描述代理流程的核心組件，包括請求/回應訊息、
//! 對話狀態管理、後端設定與連線管理等抽象層。

// --- 子模組宣告 ---
// `pub mod` 關鍵字會將指定的檔案或目錄作為一個公共的子模組引入。

/// `config` 模組：負責定義代理的設定結構，例如 `agents.toml` 的解析格式，
/// 包括代理的個人資料 (profile)、能力 (capabilities) 和提供者 (provider) 設定。
pub mod config;

/// `manager` 模組：提供 `AgentManager`，這是代理生命週期的核心管理者。
/// 它負責啟動、停止代理程序，並處理與代理之間的事件流。
pub mod manager;

/// `message` 模組：定義了應用程式與代理之間通訊的標準訊息格式，
/// 如 `AgentRequest` (發送給代理) 和 `AgentResponse` (從代理接收)。
pub mod message;

/// `providers` 模組：包含與不同類型代理後端（如 HTTP API、本地程序）
/// 進行通訊的具體實作邏輯。
pub mod providers;

/// `session` 模組：管理代理的對話狀態，包括對話歷史 (`AgentConversation`)
/// 和在 UI 中顯示的條目 (`AgentPanelEntry`)。
pub mod session;


// --- 公共 API 重新導出 ---
// `pub use` 關鍵字將子模組中的特定項目提升到 `agent` 模組的頂層命名空間，
// 這樣外部模組就可以直接透過 `crate::agent::Item` 的方式來存取，
// 而不是更長的 `crate::agent::submodule::Item` 路徑，簡化了引用。

// 從 `manager` 模組導出，方便外部直接存取代理管理相關的核心功能。
pub use manager::{AgentEvent, AgentManager, ApiKeyPrompt};
// 從 `message` 模組導出，提供標準的請求與回應類型。
pub use message::{AgentRequest, AgentResponse};
// 從 `session` 模組導出，用於存取和管理對話內容。
pub use session::{AgentConversation, AgentPanelEntry};
