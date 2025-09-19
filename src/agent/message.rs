use serde::{Deserialize, Serialize};

/// 代表從應用程式發送給代理的請求內容。
/// 這份資料結構包含了代理提供協助所需的完整上下文。
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentRequest {
    /// 當前作用中檔案的絕對路徑（可選）。
    pub file_path: Option<String>,
    /// 編輯器中文件的完整內容。
    pub content: String,
    /// 游標所在的行號（從 0 開始）。
    pub cursor_line: usize,
    /// 游標所在的欄位（從 0 開始，以字元為單位）。
    pub cursor_col: usize,
    /// 文件的語言識別碼（例如 "rust", "python"）（可選）。
    #[serde(default)]
    pub language: Option<String>,
    /// 使用者在編輯器中選取的文字內容（可選）。
    #[serde(default)]
    pub selection: Option<String>,
    /// 用於傳遞額外、非結構化資料的 JSON 值。
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl AgentRequest {
    /// `AgentRequest` 的建構函式。
    pub fn new(
        file_path: Option<String>,
        content: String,
        cursor_line: usize,
        cursor_col: usize,
    ) -> Self {
        Self {
            file_path,
            content,
            cursor_line,
            cursor_col,
            language: None,
            selection: None,
            metadata: serde_json::Value::Null,
        }
    }
}

/// 代表從代理回傳給應用程式的回應。
/// 這可以是建議、狀態資訊或程式碼修改。
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct AgentResponse {
    /// 回應的簡短標題，用於在 UI 中顯示。
    pub title: String,
    /// 回應的詳細文字內容。
    pub detail: String,
    /// 此回應相關的檔案路徑（可選）。
    #[serde(default)]
    pub file: Option<String>,
    /// 此回應相關的行號（可選）。
    #[serde(default)]
    pub line: Option<usize>,
    /// 以 diff/patch 格式提供的程式碼修改建議（可選）。
    /// 如果存在，UI 可以提供一鍵應用的功能。
    #[serde(default)]
    pub patch: Option<String>,
    /// 原始的、未經處理的代理回應，以 JSON 格式儲存（可選）。
    /// 用於除錯或未來擴充。
    #[serde(default)]
    pub raw: Option<serde_json::Value>,
}

impl AgentResponse {
    /// 一個輔助建構函式，用於快速建立一個只包含文字內容的回應。
    pub fn from_text(title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            detail: detail.into(),
            file: None,
            line: None,
            patch: None,
            raw: None,
        }
    }
}
