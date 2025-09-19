use std::collections::BTreeMap;

use crate::agent::config::{
    AgentCapabilities, AgentProfile, AgentSettings, AgentTransport, HttpApiConfig, HttpProvider,
};
use crate::agent::{AgentManager, AgentRequest, ApiKeyPrompt};
use crate::agent::manager::AgentError;
use anyhow::Result;
use log::{debug, error, info, warn};

use super::state::AgentSetupRequest;
use super::{
    AgentSwitcherState, App, FocusArea, InputPromptState, OverlayState, PendingInputAction,
};

// Implementation block for agent-related logic in the App.
impl App {
    /// Checks if the current agent prefers to receive input via the dedicated agent panel.
    pub(crate) fn agent_prefers_panel_input(&self) -> bool {
        self.agent_capabilities
            .as_ref()
            .map(|caps| caps.prefers_panel_input)
            .unwrap_or(false)
    }

    /// Determines if the application should automatically send context to the agent
    /// (e.g., when a file is opened).
    pub(crate) fn should_auto_send_agent(&self) -> bool {
        !self.agent_prefers_panel_input()
    }

    /// Manages the agent panel, ensuring it's visible and focused if needed.
    pub(crate) fn manage_agent_panel(&mut self) {
        if self.agent_manager.is_none() {
            debug!("Agent manager not initialized, bootstrapping now.");
            self.initialize_agent_runtime();
        }
        self.layout.agent_visible = true;
        self.focus = FocusArea::Agent;
        if let Some(manager) = self.agent_manager.as_ref()
            && let Some(profile) = manager.active_profile() {
                let backend = manager.backend_name().unwrap_or("Unknown Agent");
                if self.agent_prefers_panel_input() {
                    self.status_message =
                        format!("Agent panel focused: {} ({})", profile.label, backend);
                    info!("Agent panel focused: {} ({})", profile.label, backend);
                } else {
                    self.status_message =
                        format!("Agent panel shown: {} ({})", profile.label, backend);
                    info!("Agent panel shown: {} ({})", profile.label, backend);
                }
                return;
            }
        if self.agent_prefers_panel_input() {
            self.status_message = String::from("Agent panel focused, awaiting input");
        } else {
            self.status_message = String::from("Agent panel is visible");
        }
    }

    /// Opens the agent switcher overlay to allow the user to select a different agent profile.
    pub(crate) fn open_agent_switcher(&mut self) {
        if self.agent_manager.is_none() {
            self.initialize_agent_runtime();
        }
        let (profiles, active_id) = if let Some(manager) = self.agent_manager.as_ref() {
            (
                manager.profiles().to_vec(),
                manager.active_profile().map(|profile| profile.id.clone()),
            )
        } else {
            warn!("Agent manager not available, cannot switch profiles.");
            self.status_message = String::from("Agent not configured, cannot switch.");
            return;
        };

        if profiles.is_empty() {
            warn!("No agent profiles defined, cannot switch.");
            self.status_message = String::from("No agent profiles are defined.");
            return;
        }

        let selected_index = active_id
            .as_ref()
            .and_then(|id| profiles.iter().position(|profile| &profile.id == id))
            .unwrap_or(0);
        let state = AgentSwitcherState::new(profiles, selected_index);
        let preview = state
            .selected_profile()
            .map(|profile| profile.label.clone());
        self.overlay = Some(OverlayState::AgentSwitcher(state));
        if let Some(label) = preview {
            self.status_message = format!("Select Agent: {}", label);
        } else {
            self.status_message = String::from("Select Agent: No profiles available");
        }
    }

    /// Switches the active agent profile.
    pub(crate) fn switch_agent_profile(&mut self, profile: AgentProfile) {
        if self.agent_manager.is_none() {
            debug!("Agent manager not running, initializing before switching.");
            self.initialize_agent_runtime();
        }
        if let Some(manager) = self.agent_manager.as_mut() {
            let target_id = profile.id.clone();
            let target_label = profile.label.clone();
            match manager.activate_profile(target_id) {
                Ok(()) => {
                    let backend_name = manager.backend_name().unwrap_or("Unknown Agent").to_string();
                    if let Some(active) = manager.active_profile().cloned() {
                        let capabilities = active.capabilities.clone();
                        self.agent_capabilities = Some(capabilities.clone());
                        self.agent
                            .push_info("Agent Switched", format!("Switched to {}", active.label));
                        info!("Switched agent to {} ({})", active.label, backend_name);
                        if capabilities.prefers_panel_input {
                            self.layout.agent_visible = true;
                            self.focus = FocusArea::Agent;
                            self.status_message = format!(
                                "Agent switched: {} ({}), input focused",
                                active.label, backend_name
                            );
                        } else {
                            self.status_message =
                                format!("Agent switched: {} ({})", active.label, backend_name);
                        }
                    } else {
                        self.agent_capabilities = None;
                        self.status_message =
                            format!("Agent switched: {} ({})", target_label, backend_name);
                    }
                }
                Err(err) => {
                    let message = err.to_string();
                    warn!("Failed to switch agent: {}", message);
                    self.agent.push_error("Agent Switch Failed", message.clone());
                    self.status_message = format!("Failed to switch agent: {}", message);
                }
            }
        } else {
            warn!("Agent manager not running, cannot switch.");
            self.status_message = String::from("Agent not running, cannot switch.");
        }
    }

    /// Initializes the agent runtime, loading configurations and starting the manager.
    pub(crate) fn initialize_agent_runtime(&mut self) {
        match AgentManager::bootstrap(self.workspace_root.clone()) {
            Ok(mut manager) => {
                let message = manager.bootstrap_message().map(|s| s.to_string());
                match manager.ensure_active() {
                    Ok(()) => {
                        let backend_name =
                            manager.backend_name().unwrap_or("Unknown Agent").to_string();
                        let capabilities = manager
                            .active_profile()
                            .map(|profile| profile.capabilities.clone());
                        let prefers_panel_input = capabilities
                            .as_ref()
                            .map(|caps| caps.prefers_panel_input)
                            .unwrap_or(false);
                        self.agent_manager = Some(manager);
                        self.agent_capabilities = capabilities;
                        // Display available profiles if there are more than one.
                        if let Some(labels) = self.agent_manager.as_ref().map(|m| {
                            m.profiles()
                                .iter()
                                .map(|p| p.label.clone())
                                .collect::<Vec<_>>()
                        })
                            && labels.len() > 1 {
                                let listing = labels
                                    .iter()
                                    .enumerate()
                                    .map(|(idx, label)| format!("{}. {}", idx + 1, label))
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                self.agent.push_info("Available Agents", listing);
                            }
                        if let Some(info) = message {
                            info!("Agent bootstrap complete: {}", info);
                            self.agent.push_info("Agent Ready", info);
                        } else {
                            self.agent.push_info(
                                "Agent Ready",
                                format!("Loaded backend {}", backend_name),
                            );
                            info!("Agent backend {} bootstrapped successfully", backend_name);
                        }
                        self.pending_agent_setup = None;
                        if prefers_panel_input {
                            self.layout.agent_visible = true;
                            self.focus = FocusArea::Agent;
                            self.status_message =
                                format!("Agent ready: {} (input focused)", backend_name);
                            info!(
                                "Agent backend {} prefers panel input, focusing panel.",
                                backend_name
                            );
                        } else {
                            self.status_message = format!("Agent ready: {}", backend_name);
                        }
                    }
                    Err(err) => {
                        self.agent_manager = None;
                        self.agent_capabilities = None;
                        let detail = err.to_string();
                        self.agent.push_error("Agent Failed to Start", detail.clone());
                        error!("Failed to start agent: {}", detail);
                        self.status_message = format!("Agent failed to start: {}", err);
                    }
                }
            }
            Err(AgentError::NeedsApiKey(prompt)) => {
                self.agent_manager = None;
                self.agent_capabilities = None;
                info!(
                    "Agent requires API key for provider: {}",
                    prompt.provider.display_name()
                );
                self.handle_agent_setup_prompt(prompt);
            }
            Err(err) => {
                self.agent_manager = None;
                self.agent_capabilities = None;
                let detail = err.to_string();
                self.agent
                    .push_error("Failed to Load Agent Config", detail.clone());
                error!("Failed to load agent config: {}", detail);
                self.status_message = format!("Failed to load agent config: {}", err);
            }
        }
    }
    /// Handles the case where an agent requires an API key by showing an input prompt.
    pub(crate) fn handle_agent_setup_prompt(&mut self, prompt: ApiKeyPrompt) {
        self.agent
            .push_info("Agent Setup Required", prompt.instructions.clone());
        info!("Agent needs API key: {}", prompt.provider.display_name());
        self.pending_agent_setup = Some(AgentSetupRequest {
            provider: prompt.provider.clone(),
            _instructions: prompt.instructions.clone(),
        });
        self.menu_bar.close();
        let mut state = InputPromptState::new(
            format!("Set {} API Key", prompt.provider.display_name()),
            "Enter the agent API key",
            PendingInputAction::SetAgentApiKey,
            None,
        );
        state.placeholder = prompt.instructions.clone();
        self.overlay = Some(OverlayState::InputPrompt(state));
        self.status_message = format!("Agent Setup: {}", prompt.instructions);
    }

    /// Applies and saves an API key provided by the user.
    pub(crate) fn apply_agent_api_key(&mut self, api_key: &str) -> Result<(), String> {
        let request = self
            .pending_agent_setup
            .clone()
            .ok_or_else(|| String::from("No pending agent setup request"))?;
        let AgentSetupRequest { provider, .. } = request;
        let trimmed = api_key.trim();
        if trimmed.is_empty() {
            return Err(String::from("API key cannot be empty"));
        }
        // Create a new user-specific profile with the provided key.
        let profile_id = format!("{}-user", provider_slug(&provider));
        let profile = AgentProfile {
            id: profile_id.clone(),
            label: format!("{} (User)", provider.display_name()),
            description: Some(String::from("Connection using user-provided API key")),
            transport: AgentTransport::HttpApi(HttpApiConfig {
                provider: provider.clone(),
                base_url: None,
                api_key: Some(trimmed.to_string()),
                api_key_env: None,
                model: provider.default_model().map(|m| m.to_string()),
                system_prompt: None,
                extra_headers: BTreeMap::new(),
            }),
            capabilities: AgentCapabilities {
                supports_apply: true,
                supports_tools: true,
                stream_responses: true,
                prefers_panel_input: false,
            },
        };
        let settings = AgentSettings {
            profiles: vec![profile],
            default_profile: Some(profile_id),
        };
        // Save the new settings to the workspace config.
        // 注意：此文件包含敏感的 API 密钥。应确保 `config/agents.toml` 被添加到 `.gitignore` 中，
        // 以防止意外提交到版本控制系统。
        settings
            .save_to_file(&self.workspace_root)
            .map_err(|err| err.to_string())?;
        self.pending_agent_setup = None;
        self.agent.push_info(
            "Agent Config Updated",
            format!("Saved {} API key.", provider.display_name()),
        );
        info!(
            "Saved {} API key to config/agents.toml",
            provider.display_name()
        );
        self.status_message = format!(
            "Set {} API key (in config/agents.toml)",
            provider.display_name()
        );
        Ok(())
    }

    /// Sends the current editor content and context to the agent.
    pub(crate) async fn send_to_agent(&mut self, user_prompt: Option<String>) {
        if let Some(manager) = &mut self.agent_manager {
            let (line, col) = self.editor.cursor();
            let file_path = self
                .editor
                .file_path()
                .map(|p| p.to_string_lossy().to_string());
            let mut request =
                AgentRequest::new(file_path.clone(), self.editor.buffer_content(), line, col);

            let user_prompt_cloned = user_prompt.clone(); // 优化：只克隆一次

            // Construct metadata to send with the request.
            let mut metadata = serde_json::json!({
                "workspace": self.workspace_root.to_string_lossy(),
                "cursor": {
                    "line": line,
                    "column": col,
                },
            });
            if let serde_json::Value::Object(ref mut map) = metadata {
                if let Some(prompt) = user_prompt_cloned {
                    map.insert(String::from("prompt"), serde_json::Value::String(prompt));
                    map.insert(
                        String::from("source"),
                        serde_json::Value::String(String::from("user")),
                    );
                } else {
                    map.insert(
                        String::from("source"),
                        serde_json::Value::String(String::from("system")),
                    );
                }
            }
            request.metadata = metadata;
            if request.language.is_none() {
                request.language = file_path.as_ref().and_then(|path| guess_language(path));
            }

            match manager.send(request).await {
                Ok(_) => {
                    if let Some(path) = file_path.as_ref() {
                        if user_prompt.is_none() {
                            self.agent
                                .push_info("Request Sent", format!("Target file: {}", path));
                            debug!("Sent file {} to agent", path);
                        }
                    } else if user_prompt.is_none() {
                        self.agent
                            .push_info("Request Sent", String::from("Current buffer"));
                        debug!("Sent current buffer to agent");
                    }
                    if user_prompt.is_some() {
                        self.status_message = String::from("Agent request sent");
                    } else {
                        self.status_message = String::from("Sent request to agent");
                    }
                }
                Err(err) => {
                    let detail = err.to_string();
                    self.agent.push_error("Agent Request Failed", detail.clone());
                    error!("Agent request failed: {}", detail);
                    self.status_message = format!("Failed to send to agent: {}", err);
                }
            }
        } else {
            warn!("Agent not running, cannot send request.");
            self.status_message = "Agent not running".into();
        }
    }

    /// Submits the text from the agent input composer as a prompt.
    pub(crate) async fn submit_agent_prompt(&mut self) {
        let prompt = self.agent_input.take();
        if prompt.trim().is_empty() {
            self.status_message = String::from("Agent input is empty, not sending.");
            return;
        }
        if self.agent_manager.is_none() {
            self.initialize_agent_runtime();
        }
        self.agent.push_user_prompt(prompt.clone());
        info!("Submitting agent prompt: {}", prompt.lines().next().unwrap_or(""));
        self.status_message = String::from("Agent request sent");
        self.send_to_agent(Some(prompt)).await;
    }
}

/// Returns a short slug for a given HTTP provider.
fn provider_slug(provider: &HttpProvider) -> &'static str {
    match provider {
        HttpProvider::OpenAi | HttpProvider::Codex | HttpProvider::AzureOpenAi => "openai",
        HttpProvider::Gemini => "gemini",
        HttpProvider::Anthropic => "anthropic",
        HttpProvider::Ollama => "ollama",
        HttpProvider::Vllm => "vllm",
        HttpProvider::LlamaCpp => "llamacpp",
        HttpProvider::Custom => "custom",
    }
}

/// Guesses the programming language from a file path based on its extension。
///
/// 此函数通过硬编码的 `match` 语句将文件扩展名映射到编程语言。
/// 对于需要支持大量语言或需要灵活配置语言映射的场景，
/// 可以考虑将其重构为从配置文件加载映射，以提高可扩展性和可维护性。
fn guess_language(path: &str) -> Option<String> {
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())?;
    let language = match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "tsx" => "typescript",
        "jsx" => "javascript",
        "json" => "json",
        "toml" => "toml",
        "yml" | "yaml" => "yaml",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "go" => "go",
        "java" => "java",
        "cs" => "csharp",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "c" | "h" => "c",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        _ => return None,
    };
    Some(language.into())
}