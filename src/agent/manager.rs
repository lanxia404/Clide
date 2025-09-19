use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::agent::config::{
    AgentProfile, AgentSettings, AgentSettingsBootstrap, AgentTransport, HttpProvider,
};
use crate::agent::message::{AgentRequest, AgentResponse};
use crate::agent::providers::{self, AgentBackend};

/// Represents the various state events emitted from an agent backend.
/// The UI layer listens to these events to update the interface.
#[derive(Debug)]
pub enum AgentEvent {
    /// The agent has successfully connected, returning a descriptive message.
    Connected(String),
    /// The agent has returned a standard response.
    Response(AgentResponse),
    /// The agent has requested to run a tool and returns the tool's output details.
    ToolOutput { tool: String, detail: String },
    /// An error occurred during the agent's execution.
    Error(String),
    /// The agent process or connection has terminated.
    Terminated,
}

/// Custom error types for the agent manager.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("API key required: {0}")]
    NeedsApiKey(#[from] ApiKeyPrompt),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// The core manager for the agent lifecycle.
///
/// `AgentManager` is responsible for:
/// - Loading and managing `AgentSettings`.
/// - Starting and switching the active agent backend based on `AgentProfile`.
/// - Acting as the unified interface for communication between the rest of the application and the active agent.
pub struct AgentManager {
    workspace_root: PathBuf,
    settings: AgentSettings,
    active: Option<ActiveBackend>,
    bootstrap_message: Option<String>,
}

/// Holds the currently active agent backend instance and its associated settings.
struct ActiveBackend {
    /// A trait object representing the concrete backend implementation (e.g., local process, HTTP API).
    /// This allows `AgentManager` to interact polymorphically with any backend that implements the `AgentBackend` trait.
    backend: Box<dyn AgentBackend>,
    /// The profile associated with this backend.
    profile: AgentProfile,
}

impl AgentManager {
    /// Initializes the agent system.
    ///
    /// This calls `AgentSettings::bootstrap` to auto-detect available agents.
    /// Depending on the detection result, it returns a ready `AgentManager` or an error indicating an API key is needed.
    pub fn bootstrap(workspace_root: PathBuf) -> Result<Self, AgentError> {
        match AgentSettings::bootstrap(&workspace_root)? {
            AgentSettingsBootstrap::Ready { settings, message } => {
                let manager = Self {
                    workspace_root,
                    settings,
                    active: None,
                    bootstrap_message: message,
                };
                Ok(manager)
            }
            AgentSettingsBootstrap::NeedsApiKey {
                provider,
                instructions,
            } => Err(AgentError::NeedsApiKey(ApiKeyPrompt {
                provider,
                instructions,
            })),
        }
    }

    /// Returns the initial bootstrap message, if any.
    pub fn bootstrap_message(&self) -> Option<&str> {
        self.bootstrap_message.as_deref()
    }

    /// Ensures that there is an active agent.
    ///
    /// If no agent is currently active, this function attempts to activate the default profile.
    /// This implements lazy loading for the agent.
    pub fn ensure_active(&mut self) -> Result<()> {
        if self.active.is_some() {
            return Ok(());
        }
        let profile = self
            .settings
            .default_profile()
            .cloned()
            .ok_or_else(|| anyhow!("No agent profiles defined in settings"))?;
        self.activate_profile(profile.id.clone())
    }

    /// Activates an agent by its profile ID.
    ///
    /// This terminates any existing agent connection and creates a new backend instance based on the new profile.
    pub fn activate_profile(&mut self, profile_id: String) -> Result<()> {
        let profile = self
            .settings
            .profile(&profile_id)
            .cloned()
            .ok_or_else(|| anyhow!("Agent profile not found: {}", profile_id))?;
        let backend = self.instantiate_backend(&profile)?;
        self.settings.default_profile = Some(profile.id.clone());
        self.active = Some(ActiveBackend { backend, profile });
        Ok(())
    }

    /// Instantiates the corresponding backend based on the `transport` type in the profile.
    /// This is a factory function that maps settings to concrete `AgentBackend` implementations.
    fn instantiate_backend(&self, profile: &AgentProfile) -> Result<Box<dyn AgentBackend>> {
        match &profile.transport {
            AgentTransport::LocalProcess(config) => {
                let backend = providers::local_process::LocalProcessBackend::start(
                    config,
                    &self.workspace_root,
                )?;
                Ok(Box::new(backend))
            }
            AgentTransport::HttpApi(config) => {
                let backend = providers::http::HttpBackend::new(config.clone())?;
                Ok(Box::new(backend))
            }
            AgentTransport::Mcp(config) => {
                let backend = providers::mcp::McpBackend::new(config.clone())?;
                Ok(Box::new(backend))
            }
        }
    }

    /// Returns the name of the currently active backend.
    pub fn backend_name(&self) -> Option<&str> {
        self.active.as_ref().map(|active| active.backend.name())
    }

    /// Returns the currently active agent profile.
    pub fn active_profile(&self) -> Option<&AgentProfile> {
        self.active.as_ref().map(|active| &active.profile)
    }

    /// Returns a list of all available agent profiles.
    pub fn profiles(&self) -> &[AgentProfile] {
        &self.settings.profiles
    }

    /// Sends a request asynchronously to the active agent.
    /// Returns an error if no agent is active.
    pub async fn send(&mut self, request: AgentRequest) -> Result<()> {
        if let Some(active) = self.active.as_mut() {
            active.backend.send(request).await
        } else {
            Err(anyhow!("Agent not started"))
        }
    }

    /// Polls for an event from the active agent.
    ///
    /// This is a non-blocking function that retrieves an event from the backend's event queue.
    /// Returns `None` if no agent is active or if there are no events.
    pub fn poll_event(&mut self) -> Option<AgentEvent> {
        if let Some(active) = self.active.as_mut() {
            active.backend.poll_event()
        } else {
            None
        }
    }
}

/// This struct carries the information needed when `AgentManager::bootstrap` returns that an API key is required.
#[derive(Debug, thiserror::Error)]
#[error("{instructions}")]
pub struct ApiKeyPrompt {
    /// The API provider that needs a key.
    pub provider: HttpProvider,
    /// The instructions to display to the user.
    pub instructions: String,
}
