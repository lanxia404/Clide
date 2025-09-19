use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 代理設定的頂層結構，通常從 `config/agents.toml` 載入。
/// 用於描述所有可用的代理後端來源。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// 一個包含所有已定義代理設定檔的列表。
    #[serde(default)]
    pub profiles: Vec<AgentProfile>,
    /// 預設使用的代理設定檔的 ID。如果為 `None`，則使用列表中的第一個。
    #[serde(default)]
    pub default_profile: Option<String>,
}

impl AgentSettings {
    /// 從指定的工作區目錄讀取設定。
    /// 如果 `config/agents.toml` 不存在，則提供一個預設的本地範例代理作為備用。
    pub fn load(workspace_root: &Path) -> Result<Self> {
        let config_path = workspace_root.join("config/agents.toml");
        if config_path.exists() {
            // 如果設定檔存在，讀取並解析它。
            let raw = fs::read_to_string(&config_path)
                .with_context(|| format!("讀取代理設定失敗: {}", config_path.display()))?;
            let parsed: AgentSettings = toml::from_str(&raw)
                .with_context(|| format!("解析代理設定失敗: {}", config_path.display()))?;
            // 標準化路徑，將相對路徑轉換為絕對路徑。
            Ok(parsed.normalize(workspace_root))
        } else {
            // 如果設定檔不存在，回傳一個指向內建 Python 腳本的預設設定。
            Ok(Self::default_stub(workspace_root))
        }
    }

    /// 啟動函式，用於智慧偵測和設定代理。
    ///
    /// 此函式會按以下順序嘗試尋找可用的代理設定：
    /// 1. 檢查 `config/agents.toml` 是否存在。
    /// 2. 偵測已知的 CLI 代理程式 (如 `ollama`, `gemini-cli`)。
    /// 3. 偵測本地後端服務 (如 Ollama)。
    /// 4. 偵測環境變數中的 API 金鑰 (如 `OPENAI_API_KEY`)。
    /// 5. 如果以上皆未成功，則回傳 `NeedsApiKey` 狀態，提示使用者輸入金鑰。
    pub fn bootstrap(workspace_root: &Path) -> Result<AgentSettingsBootstrap> {
        let config_path = workspace_root.join("config/agents.toml");
        if config_path.exists() {
            let settings = Self::load(workspace_root)?;
            return Ok(AgentSettingsBootstrap::Ready {
                settings,
                message: Some(String::from("已載入 config/agents.toml 設定")),
            });
        }

        if let Some((settings, message)) = Self::detect_cli_agents(workspace_root) {
            return Ok(AgentSettingsBootstrap::Ready {
                settings,
                message: Some(message),
            });
        }

        if let Some((settings, message)) = Self::detect_local_backend(workspace_root) {
            return Ok(AgentSettingsBootstrap::Ready {
                settings,
                message: Some(message),
            });
        }

        if let Some((settings, message)) = Self::detect_env_provider() {
            return Ok(AgentSettingsBootstrap::Ready {
                settings,
                message: Some(message),
            });
        }

        // 如果所有自動偵測都失敗了，提示使用者輸入 OpenAI API 金鑰。
        Ok(AgentSettingsBootstrap::NeedsApiKey {
            provider: HttpProvider::OpenAi,
            instructions: String::from(
                "未偵測到本地代理或 API 設定。請安裝 Ollama/Llama.cpp 等本地服務，或輸入 OpenAI API 金鑰。",
            ),
        })
    }

    /// 標準化設定中的所有路徑。
    /// 這確保了在設定中使用的相對路徑（如 `working_dir`）能被正確地解析為相對於工作區根目錄的絕對路徑。
    fn normalize(mut self, workspace_root: &Path) -> Self {
        for profile in &mut self.profiles {
            profile.transport.normalize_paths(workspace_root);
        }
        self
    }

    /// 產生一個預設的、指向內建 Python 代理腳本的設定。
    /// 這在使用者沒有提供任何自訂設定時作為一個開箱即用的範例。
    fn default_stub(workspace_root: &Path) -> Self {
        let script = workspace_root.join("python/agent_stub.py");
        Self {
            profiles: vec![AgentProfile {
                id: "local-stub".into(),
                label: "本地範例代理".into(),
                description: Some("透過 python 腳本示範 Clide IPC".into()),
                transport: AgentTransport::LocalProcess(LocalProcessConfig {
                    program: "python3".into(),
                    args: vec![script.to_string_lossy().into()],
                    working_dir: None,
                    env: BTreeMap::new(),
                    stdin_initial: None,
                }),
                capabilities: AgentCapabilities {
                    prefers_panel_input: true,
                    ..AgentCapabilities::default()
                },
            }],
            default_profile: Some("local-stub".into()),
        }
    }

    /// 取得預設的代理設定檔。
    /// 如果 `default_profile` 有指定，則回傳對應的設定檔；否則回傳列表中的第一個。
    pub fn default_profile(&self) -> Option<&AgentProfile> {
        if let Some(id) = &self.default_profile {
            self.profiles.iter().find(|profile| &profile.id == id)
        } else {
            self.profiles.first()
        }
    }

    /// 根據 ID 尋找並回傳一個代理設定檔。
    pub fn profile(&self, id: &str) -> Option<&AgentProfile> {
        self.profiles.iter().find(|profile| profile.id == id)
    }

    /// 將當前的代理設定儲存到 `config/agents.toml` 檔案中。
    pub fn save_to_file(&self, workspace_root: &Path) -> Result<()> {
        let config_dir = workspace_root.join("config");
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir)
                .with_context(|| format!("建立設定目錄失敗: {}", config_dir.display()))?;
        }
        let config_path = config_dir.join("agents.toml");
        let serialized = toml::to_string_pretty(self).context("序列化代理設定失敗")?;
        fs::write(&config_path, serialized)
            .with_context(|| format!("寫入代理設定失敗: {}", config_path.display()))?;
        Ok(())
    }

    /// 偵測本地執行的後端服務，目前主要是 Ollama。
    fn detect_local_backend(workspace_root: &Path) -> Option<(AgentSettings, String)> {
        if let Some(result) = Self::detect_workspace_agent(workspace_root) {
            return Some(result);
        }
        // 檢查 `ollama` 指令是否存在於系統 PATH 中。
        if let Some(command) = find_command("ollama") {
            let mut profiles = Vec::new();
            profiles.push(AgentProfile {
                id: String::from("ollama-local"),
                label: String::from("Ollama 本地模型"),
                description: Some(format!(
                    "偵測到 Ollama 指令於 {}，將使用本地語言模型服務",
                    command.display()
                )),
                transport: AgentTransport::HttpApi(HttpApiConfig {
                    provider: HttpProvider::Ollama,
                    base_url: Some(String::from("http://localhost:11434/api/generate")),
                    api_key: None,
                    api_key_env: None,
                    model: None,
                    system_prompt: None,
                    extra_headers: BTreeMap::new(),
                }),
                capabilities: AgentCapabilities::default(),
            });
            return Some((
                AgentSettings {
                    profiles,
                    default_profile: Some(String::from("ollama-local")),
                },
                String::from("已偵測到本地 Ollama 服務並完成連線設定"),
            ));
        }

        None
    }

    /// 偵測已知的、透過 CLI 互動的代理程式。
    fn detect_cli_agents(_workspace_root: &Path) -> Option<(AgentSettings, String)> {
        #[derive(Debug)]
        struct CliAgentDescriptor {
            id: &'static str,
            label: &'static str,
            commands: &'static [&'static str],
            args: &'static [&'static str],
            description: &'static str,
            prefers_panel_input: bool,
        }

        // 這些 CLI 程式對應官方提供的 npm / Python 套件或安裝工具，
        // 常見於 OpenAI Codex、Google Gemini 及 Anthropic Claude 等代理。
        // 偵測方式以 PATH 中可執行檔為主，涵蓋 npm、pip 或系統套件管理員安裝後的常見路徑。
        const KNOWN_CLI_AGENTS: &[CliAgentDescriptor] = &[
            CliAgentDescriptor {
                id: "openai-codex-cli",
                label: "OpenAI Codex CLI",
                commands: &["openai-codex", "openai-codex-cli"],
                args: &["--stdio"],
                description: "偵測到透過 npm 安裝的 openai-codex 指令，將以 stdio 管道溝通。",
                prefers_panel_input: true,
            },
            CliAgentDescriptor {
                id: "gemini-cli",
                label: "Google Gemini CLI",
                commands: &["gemini", "gemini-cli"],
                args: &["chat", "--stdio"],
                description: "偵測到 gemini/gemini-cli 指令，啟用 Gemini 代理互動。",
                prefers_panel_input: true,
            },
            CliAgentDescriptor {
                id: "anthropic-cli",
                label: "Anthropic Claude CLI",
                commands: &["claude", "anthropic"],
                args: &["--stdio"],
                description: "偵測到 Claude CLI，將以 stdio 模式連線。",
                prefers_panel_input: true,
            },
        ];

        let mut profiles = Vec::new();
        let mut labels = Vec::new();

        for descriptor in KNOWN_CLI_AGENTS {
            let found = descriptor
                .commands
                .iter()
                .find_map(|cmd| find_command(cmd).map(|path| (cmd, path)));
            let (command_label, command_path) = match found {
                Some(result) => result,
                None => continue,
            };

            let program = command_path.to_string_lossy().into_owned();
            profiles.push(AgentProfile {
                id: descriptor.id.to_string(),
                label: descriptor.label.to_string(),
                description: Some(format!(
                    "{} (於 {} 偵測)",
                    descriptor.description,
                    command_path.display()
                )),
                transport: AgentTransport::LocalProcess(LocalProcessConfig {
                    program,
                    args: descriptor
                        .args
                        .iter()
                        .map(|arg| (*arg).to_string())
                        .collect(),
                    working_dir: None,
                    env: BTreeMap::new(),
                    stdin_initial: None,
                }),
                capabilities: AgentCapabilities {
                    prefers_panel_input: descriptor.prefers_panel_input,
                    ..AgentCapabilities::default()
                },
            });
            labels.push(format!("{} ({})", descriptor.label, command_label));
        }

        if profiles.is_empty() {
            return None;
        }

        let default_profile = profiles.first().map(|profile| profile.id.clone());

        let message = if labels.len() > 1 {
            format!(
                "偵測到多個 CLI 代理：{}，請於代理面板選擇使用。",
                labels.join("、")
            )
        } else {
            format!("偵測到 CLI 代理：{}。", labels.join("、"))
        };

        Some((
            AgentSettings {
                profiles,
                default_profile,
            },
            message,
        ))
    }

    /// 偵測工作區中是否存在本地代理腳本。
    fn detect_workspace_agent(workspace_root: &Path) -> Option<(AgentSettings, String)> {
        let script = workspace_root.join("python/agent_stub.py");
        if script.is_file() {
            let settings = Self::default_stub(workspace_root);
            let message = script
                .strip_prefix(workspace_root)
                .map(|path| format!("偵測到本地代理腳本：{}", path.display()))
                .unwrap_or_else(|_| format!("偵測到本地代理腳本：{}", script.display()));
            return Some((settings, message));
        }
        None
    }

    /// 偵測環境變數中設定的 API 供應商金鑰。
    fn detect_env_provider() -> Option<(AgentSettings, String)> {
        let env_map = [
            (
                "OPENAI_API_KEY",
                HttpProvider::OpenAi,
                "openai",
                "gpt-4o-mini",
            ),
            (
                "ANTHROPIC_API_KEY",
                HttpProvider::Anthropic,
                "anthropic",
                "claude-3-5-sonnet-20240620",
            ),
            (
                "GEMINI_API_KEY",
                HttpProvider::Gemini,
                "gemini",
                "gemini-1.5-flash",
            ),
        ];

        for (var, provider, id_suffix, default_model) in env_map {
            let provider_kind = provider.clone();
            if let Ok(value) = env::var(var) {
                if value.trim().is_empty() {
                    continue;
                }
                let profile_id = format!("{}-env", id_suffix);
                let label = format!("{} 雲端代理", provider_kind.display_name());
                let mut profiles = Vec::new();
                profiles.push(AgentProfile {
                    id: profile_id.clone(),
                    label,
                    description: Some(format!("偵測到環境變數 {}，將使用雲端服務", var)),
                    transport: AgentTransport::HttpApi(HttpApiConfig {
                        provider: provider_kind.clone(),
                        base_url: None,
                        api_key: None,
                        api_key_env: Some(var.to_string()),
                        model: Some(default_model.to_string()),
                        system_prompt: None,
                        extra_headers: BTreeMap::new(),
                    }),
                    capabilities: AgentCapabilities {
                        supports_apply: true,
                        supports_tools: true,
                        stream_responses: true,
                        prefers_panel_input: false,
                    },
                });
                let settings = AgentSettings {
                    profiles,
                    default_profile: Some(profile_id),
                };
                let message = format!(
                    "偵測到 {} 環境金鑰，預設使用 {} 後端",
                    var,
                    provider_kind.display_name()
                );
                return Some((settings, message));
            }
        }

        None
    }
}

/// 描述一個代理執行個體（Profile）的完整設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentProfile {
    /// 唯一的識別碼。
    pub id: String,
    /// 顯示在 UI 上的名稱。
    pub label: String,
    /// 對此代理的詳細描述（可選）。
    #[serde(default)]
    pub description: Option<String>,
    /// 與此代理通訊的方式和設定。
    pub transport: AgentTransport,
    /// 此代理支援的功能旗標。
    #[serde(default)]
    pub capabilities: AgentCapabilities,
}

/// 代理可提供的功能旗標，供 UI 根據這些能力調整互動流程。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct AgentCapabilities {
    /// 代理是否支援直接將變更應用於程式碼。
    #[serde(default)]
    pub supports_apply: bool,
    /// 代理是否支援使用工具（Tool Use）。
    #[serde(default)]
    pub supports_tools: bool,
    /// 代理是否支援以串流方式回傳回應。
    #[serde(default)]
    pub stream_responses: bool,
    /// 指示此代理是否偏好使用獨立的輸入面板，而不是行內編輯器。
    #[serde(default)]
    pub prefers_panel_input: bool,
}

/// 定義了與代理通訊的不同方式（Transport）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentTransport {
    /// 透過本地子程序（IPC/stdio）進行通訊。
    LocalProcess(LocalProcessConfig),
    /// 透過標準的 HTTP API 進行通訊。
    HttpApi(HttpApiConfig),
    /// 透過 MCP（一個特定的通訊協定）進行通訊。
    Mcp(McpConfig),
}

impl AgentTransport {
    /// 對 Transport 設定中的路徑進行標準化。
    fn normalize_paths(&mut self, workspace_root: &Path) {
        match self {
            AgentTransport::LocalProcess(config) => config.normalize(workspace_root),
            AgentTransport::HttpApi(config) => config.normalize(workspace_root),
            AgentTransport::Mcp(config) => config.normalize(workspace_root),
        }
    }
}

/// 本地程序代理的設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProcessConfig {
    /// 要執行的程式（例如 `python3` 或 `/path/to/agent`）。
    pub program: String,
    /// 傳遞給程式的命令列參數。
    #[serde(default)]
    pub args: Vec<String>,
    /// 程式的工作目錄。如果為相對路徑，會被解析為相對於工作區根目錄。
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    /// 為子程序設定的額外環境變數。
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// 在程序啟動時，要寫入其 stdin 的初始字串（可選）。
    #[serde(default)]
    pub stdin_initial: Option<String>,
}

impl LocalProcessConfig {
    /// 標準化 `working_dir` 和 `args` 中的路徑。
    fn normalize(&mut self, workspace_root: &Path) {
        if let Some(dir) = self.working_dir.as_mut()
            && dir.is_relative() {
                *dir = workspace_root.join(&dir);
            }
        // 檢查參數是否為指向工作區內檔案的相對路徑，如果是，則轉換為絕對路徑。
        self.args = self
            .args
            .iter()
            .map(|arg| {
                let path = Path::new(arg);
                if path.is_relative() {
                    let candidate = workspace_root.join(path);
                    if candidate.exists() {
                        candidate.to_string_lossy().into()
                    } else {
                        arg.clone()
                    }
                } else {
                    arg.clone()
                }
            })
            .collect();
    }
}

/// HTTP API 代理的設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpApiConfig {
    /// API 供應商的類型。
    pub provider: HttpProvider,
    /// API 的基礎 URL。
    #[serde(default)]
    pub base_url: Option<String>,
    /// 直接在設定中指定的 API 金鑰。
    #[serde(default)]
    pub api_key: Option<String>,
    /// 用於讀取 API 金鑰的環境變數名稱。
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// 要使用的模型名稱（例如 `gpt-4o-mini`）。
    #[serde(default)]
    pub model: Option<String>,
    /// 系統提示詞（System Prompt）。
    #[serde(default)]
    pub system_prompt: Option<String>,
    /// 附加到請求中的額外 HTTP 標頭。
    #[serde(default)]
    pub extra_headers: BTreeMap<String, String>,
}

impl HttpApiConfig {
    fn normalize(&mut self, _workspace_root: &Path) {
        // 目前無需路徑處理，但保留此函式以便未來擴充。
    }

    /// 解析並回傳最終的 API 金鑰。
    /// 優先順序：`api_key` 欄位 > `api_key_env` 環境變數。
    pub fn resolved_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            Some(key.clone())
        } else if let Some(var) = &self.api_key_env {
            std::env::var(var).ok()
        } else {
            None
        }
    }
}

/// 支援的 HTTP API 供應商枚舉。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpProvider {
    OpenAi,
    Codex,
    Gemini,
    Anthropic,
    AzureOpenAi,
    Ollama,
    Vllm,
    LlamaCpp,
    Custom,
}

impl HttpProvider {
    /// 回傳供應商的顯示名稱。
    pub fn display_name(&self) -> &'static str {
        match self {
            HttpProvider::OpenAi | HttpProvider::Codex | HttpProvider::AzureOpenAi => "OpenAI",
            HttpProvider::Gemini => "Google Gemini",
            HttpProvider::Anthropic => "Anthropic Claude",
            HttpProvider::Ollama => "Ollama",
            HttpProvider::Vllm => "vLLM",
            HttpProvider::LlamaCpp => "llama.cpp",
            HttpProvider::Custom => "自訂 HTTP",
        }
    }

    /// 回傳此供應商的建議預設模型。
    pub fn default_model(&self) -> Option<&'static str> {
        match self {
            HttpProvider::OpenAi | HttpProvider::Codex | HttpProvider::AzureOpenAi => {
                Some("gpt-4o-mini")
            }
            HttpProvider::Gemini => Some("gemini-1.5-flash"),
            HttpProvider::Anthropic => Some("claude-3-5-sonnet-20240620"),
            _ => None,
        }
    }
}

/// MCP 代理的設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    pub endpoint: String,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub invoke_path: Option<String>,
}

impl McpConfig {
    /// 標準化 `invoke_path`。
    fn normalize(&mut self, workspace_root: &Path) {
        if let Some(path) = self.invoke_path.as_mut() {
            let candidate = Path::new(path);
            if candidate.is_relative() {
                *path = workspace_root.join(candidate).to_string_lossy().into();
            }
        }
    }

    /// 解析並回傳最終的 API 金鑰。
    pub fn resolved_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            Some(key.clone())
        } else if let Some(var) = &self.api_key_env {
            std::env::var(var).ok()
        } else {
            None
        }
    }
}

/// 代理設定啟動時的狀態。
#[derive(Debug)]
pub enum AgentSettingsBootstrap {
    /// 代理設定已就緒。
    Ready {
        settings: AgentSettings,
        message: Option<String>,
    },
    /// 需要使用者提供 API 金鑰才能繼續。
    NeedsApiKey {
        provider: HttpProvider,
        instructions: String,
    },
}

/// 在系統 PATH 中尋找指定的命令。
fn find_command(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let mut candidates: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        let exts = [".exe", ".bat", ".cmd"];
        for dir in env::split_paths(&path_var) {
            let direct = dir.join(name);
            if direct.is_file() {
                return Some(direct);
            }
            for ext in &exts {
                let candidate = dir.join(format!("{}{}", name, ext));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        return None;
    }

    #[cfg(not(windows))]
    {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join(name);
            if candidate.is_file() && is_executable(&candidate) {
                return Some(candidate);
            } else if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }

    candidates.into_iter().find(|path| path.is_file())
}

/// 檢查檔案是否具有可執行權限 (僅限非 Windows 系統)。
#[cfg(not(windows))]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// 在 Windows 上，所有檔案都被視為潛在可執行的。
#[cfg(windows)]
fn is_executable(_path: &Path) -> bool {
    true
}
