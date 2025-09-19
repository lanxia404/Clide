use crate::agent::{AgentConversation, AgentPanelEntry, AgentResponse};

pub struct AgentPanel {
    conversation: AgentConversation,
}

impl AgentPanel {
    pub fn with_placeholder() -> Self {
        let entry = AgentPanelEntry::Info {
            title: String::from("代理狀態"),
            detail: String::from("代理尚未連線"),
        };
        let conversation = AgentConversation::with_entry(entry);
        Self { conversation }
    }

    pub fn entries(&self) -> &[AgentPanelEntry] {
        self.conversation.entries()
    }

    pub fn push_response(&mut self, response: AgentResponse) {
        self.conversation.push(AgentPanelEntry::Response(response));
    }

    pub fn push_user_prompt(&mut self, prompt: impl Into<String>) {
        self.conversation.push(AgentPanelEntry::UserPrompt {
            prompt: prompt.into(),
        });
    }

    pub fn push_info(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        self.conversation.push(AgentPanelEntry::Info {
            title: title.into(),
            detail: detail.into(),
        });
    }

    pub fn push_error(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        self.conversation.push(AgentPanelEntry::Error {
            title: title.into(),
            detail: detail.into(),
        });
    }

    pub fn push_tool_output(&mut self, tool: impl Into<String>, detail: impl Into<String>) {
        self.conversation.push(AgentPanelEntry::ToolOutput {
            tool: tool.into(),
            detail: detail.into(),
        });
    }

    pub fn move_selection(&mut self, delta: isize) {
        self.conversation.move_selection(delta);
    }

    pub fn selected_entry(&self) -> Option<&AgentPanelEntry> {
        self.conversation.selected()
    }

    pub fn selected_index(&self) -> usize {
        self.conversation.selected_index()
    }

    pub fn set_selection(&mut self, index: usize) {
        self.conversation.set_selection(index);
    }
}
