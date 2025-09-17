pub struct AgentMessage {
    pub title: String,
    pub detail: String,
}

pub struct AgentPanel {
    messages: Vec<AgentMessage>,
    selected: usize,
}

impl AgentPanel {
    pub fn with_placeholder() -> Self {
        let messages = vec![
            AgentMessage {
                title: String::from("代理狀態"),
                detail: String::from("等待 AI 編輯建議"),
            },
            AgentMessage {
                title: String::from("變更預覽"),
                detail: String::from("main.rs 第 10 行新增 println!"),
            },
        ];
        Self {
            messages,
            selected: 0,
        }
    }

    pub fn messages(&self) -> &[AgentMessage] {
        &self.messages
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.messages.is_empty() {
            return;
        }
        let len = self.messages.len() as isize;
        let mut new_index = self.selected as isize + delta;
        if new_index < 0 {
            new_index = 0;
        }
        if new_index >= len {
            new_index = len - 1;
        }
        self.selected = new_index as usize;
    }

    pub fn selected_message(&self) -> Option<&AgentMessage> {
        self.messages.get(self.selected)
    }

    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn set_selection(&mut self, index: usize) {
        if self.messages.is_empty() {
            return;
        }
        self.selected = index.min(self.messages.len().saturating_sub(1));
    }
}
