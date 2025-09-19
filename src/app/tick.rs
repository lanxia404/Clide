use super::App;
use crate::agent::AgentEvent;

// Implementation block for tick-related logic in the App.
impl App {
    /// This function is called on every "tick" of the application loop.
    ///
    /// It's responsible for handling time-based events and background tasks, such as
    /// polling for events from the agent manager.
    pub(crate) fn on_tick(&mut self) {
        // Update the last_tick timestamp.
        if self.last_tick.elapsed() >= self.tick_rate {
            self.last_tick = std::time::Instant::now();
        }

        let mut runtime_terminated = false;
        if let Some(manager) = self.agent_manager.as_mut() {
            // Poll for any events that have come from the agent in the background.
            while let Some(event) = manager.poll_event() {
                match event {
                    AgentEvent::Connected(name) => {
                        self.agent.push_info("Agent Connected", format!("Connected to {}", name));
                        self.status_message = format!("Agent connected: {}", name);
                    }
                    AgentEvent::Response(response) => {
                        self.agent.push_response(response);
                        self.status_message = String::from("Received suggestion from agent");
                    }
                    AgentEvent::ToolOutput { tool, detail } => {
                        self.agent.push_tool_output(tool.clone(), detail.clone());
                        self.status_message = format!("Tool output: {}", tool);
                    }
                    AgentEvent::Error(message) => {
                        self.agent.push_error("Agent Error", message.clone());
                        self.status_message = format!("Agent error: {}", message);
                    }
                    AgentEvent::Terminated => {
                        self.agent
                            .push_error("Agent Terminated", "Connection to backend was lost".to_string());
                        runtime_terminated = true;
                        self.status_message = String::from("Agent terminated");
                    }
                }
            }
        }

        // If the agent runtime terminated, clean up the manager and capabilities.
        if runtime_terminated {
            self.agent_manager = None;
            self.agent_capabilities = None;
        }
    }
}