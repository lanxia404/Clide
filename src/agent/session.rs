use super::AgentResponse;

/// 代表代理面板中單一可顯示的訊息條目。
///
/// 這個枚舉將所有可能出現在對話歷史中的內容（使用者輸入、代理回應、系統資訊等）
/// 統一為單一類型，極大地簡化了 UI 的渲染邏輯。
#[derive(Debug, Clone)]
pub enum AgentPanelEntry {
    /// 使用者輸入的提示。
    UserPrompt { prompt: String },
    /// 來自代理的標準回應。
    Response(AgentResponse),
    /// 系統產生的參考資訊。
    Info { title: String, detail: String },
    /// 系統產生的錯誤訊息。
    Error { title: String, detail: String },
    /// 代理請求執行工具後產生的輸出。
    ToolOutput { tool: String, detail: String },
}

impl AgentPanelEntry {
    /// 一個輔助函式，從不同類型的條目中提取一個統一的標題字串，用於 UI 顯示。
    pub fn title(&self) -> &str {
        match self {
            AgentPanelEntry::UserPrompt { prompt } => prompt,
            AgentPanelEntry::Response(resp) => &resp.title,
            AgentPanelEntry::Info { title, .. } => title,
            AgentPanelEntry::Error { title, .. } => title,
            AgentPanelEntry::ToolOutput { tool, .. } => tool,
        }
    }
}

/// 管理一個完整的代理對話狀態，包括所有訊息列表和當前的選取游標。
#[derive(Default)]
pub struct AgentConversation {
    /// 儲存對話歷史中的所有條目。
    entries: Vec<AgentPanelEntry>,
    /// 當前在 UI 中被選取的條目的索引。
    selected: usize,
}

impl AgentConversation {
    /// 建立一個包含單一初始條目的新對話。
    pub fn with_entry(entry: AgentPanelEntry) -> Self {
        let mut convo = Self {
            entries: vec![entry],
            selected: 0,
        };
        convo.clamp_selection();
        convo
    }

    /// 回傳對話條目列表的不可變切片。
    pub fn entries(&self) -> &[AgentPanelEntry] {
        &self.entries
    }

    /// 向對話歷史中新增一個條目，並自動將選取位置移動到這個新條目上。
    pub fn push(&mut self, entry: AgentPanelEntry) {
        self.entries.push(entry);
        self.selected = self.entries.len().saturating_sub(1);
    }

    /// 回傳當前選取的條目的引用。
    pub fn selected(&self) -> Option<&AgentPanelEntry> {
        self.entries.get(self.selected)
    }

    /// 回傳當前選取的條目的索引。
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// 根據給定的偏移量（`delta`）移動選取位置。
    /// `delta` 可以是正數（向下移動）或負數（向上移動）。
    /// 此方法會確保選取位置不會超出邊界。
    pub fn move_selection(&mut self, delta: isize) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as isize;
        let mut next = self.selected as isize + delta;
        if next < 0 {
            next = 0;
        }
        if next >= len {
            next = len - 1;
        }
        self.selected = next as usize;
    }

    /// 將選取位置直接設定為指定的索引。
    /// 此方法會確保索引值在有效範圍內。
    pub fn set_selection(&mut self, index: usize) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = index.min(self.entries.len().saturating_sub(1));
    }

    /// 一個內部輔助函式，用於確保 `selected` 索引總是在有效範圍內。
    fn clamp_selection(&mut self) {
        if self.entries.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        }
    }
}
