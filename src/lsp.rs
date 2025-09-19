use anyhow::Result;
use lsp_types::{
    notification::Notification,
    request::{Completion, GotoDefinition, HoverRequest, Initialize, Request},
    ClientCapabilities, InitializeParams, Uri,
    WorkspaceFolder,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};
use url::Url;

#[derive(Debug, Deserialize)]
struct LspNotification {
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum LspResponse {
    Success { id: u64, result: Value },
    Error { id: u64, error: Value },
}

#[derive(Debug)]
pub enum LspMessage {
    Notification(String, Value),
    Response(u64, Value),
}

pub struct LspClient {
    #[allow(dead_code)]
    server: Child,
    writer: UnboundedSender<Value>,
    is_dummy: bool,
}

fn create_lsp_request<R: Request>(id: u64, params: R::Params) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": R::METHOD, "params": params })
}

fn create_lsp_notification<N: Notification>(params: N::Params) -> Value {
    json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params })
}

impl LspClient {
    pub fn new() -> Result<(Self, UnboundedReceiver<LspMessage>)> {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (writer_tx, writer_rx) = mpsc::unbounded_channel();

        let server_process = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match server_process {
            Ok(mut server) => {
                let stdin = server.stdin.take().unwrap();
                let stdout = server.stdout.take().unwrap();
                let stderr = server.stderr.take().unwrap();

                tokio::spawn(Self::writer_task(stdin, writer_rx));
                tokio::spawn(Self::reader_task(stdout, msg_tx.clone()));
                tokio::spawn(async move {
                    let mut reader = BufReader::new(stderr);
                    let mut buffer = Vec::new();
                    while let Ok(bytes_read) = reader.read_until(b'\n', &mut buffer).await {
                        if bytes_read == 0 { break; }
                    }
                });

                let client = Self { server, writer: writer_tx, is_dummy: false };
                Ok((client, msg_rx))
            }
            Err(e) => {
                eprintln!("[Clide WARN] Failed to start LSP server 'rust-analyzer': {}. LSP features will be disabled.", e);
                let (_, dummy_rx) = mpsc::unbounded_channel::<LspMessage>();
                let (dummy_tx, _) = mpsc::unbounded_channel::<Value>();
                let dummy_server = Command::new("sleep").arg("infinity").spawn()?;
                let client = Self { server: dummy_server, writer: dummy_tx, is_dummy: true };
                Ok((client, dummy_rx))
            }
        }
    }

    async fn writer_task(mut stdin: ChildStdin, mut rx: UnboundedReceiver<Value>) {
        while let Some(msg) = rx.recv().await {
            let msg_str = msg.to_string();
            let content = format!("Content-Length: {}\r\n\r\n{}", msg_str.len(), msg_str);
            if stdin.write_all(content.as_bytes()).await.is_err() {
                break;
            }
        }
    }

    async fn reader_task<T: AsyncRead + Unpin>(stream: T, msg_tx: UnboundedSender<LspMessage>) {
        let mut reader = BufReader::new(stream);
        let mut buffer = String::new();
        let mut content_length: Option<usize> = None;

        loop {
            buffer.clear();
            if let Ok(bytes_read) = reader.read_line(&mut buffer).await {
                if bytes_read == 0 { break; }
                if buffer.starts_with("Content-Length:") {
                    if let Some(len_str) = buffer.trim().split(':').nth(1) {
                        content_length = len_str.trim().parse::<usize>().ok();
                    }
                }
                if buffer.trim().is_empty() {
                    if let Some(length) = content_length {
                        let mut body_buffer = vec![0; length];
                        if reader.read_exact(&mut body_buffer).await.is_ok() {
                            if let Ok(notification) = serde_json::from_slice::<LspNotification>(&body_buffer) {
                                let _ = msg_tx.send(LspMessage::Notification(notification.method, notification.params));
                            } else if let Ok(response) = serde_json::from_slice::<LspResponse>(&body_buffer) {
                                if let LspResponse::Success { id, result } = response {
                                    let _ = msg_tx.send(LspMessage::Response(id, result));
                                }
                            }
                        }
                        content_length = None;
                    }
                }
            } else {
                break;
            }
        }
    }

    pub fn initialize(&self, root_path: &Path) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let root_uri_url = Url::from_file_path(root_path).map_err(|_| anyhow::anyhow!("Invalid root path"))?;
        let root_uri: Uri = root_uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse root URI: {}", e))?;
        let workspace_folder = WorkspaceFolder {
            uri: root_uri,
            name: root_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        };
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            workspace_folders: Some(vec![workspace_folder]),
            capabilities: ClientCapabilities::default(),
            ..Default::default()
        };
        let request = create_lsp_request::<Initialize>(1, params);
        self.writer.send(request)?;
        Ok(())
    }

    pub fn did_open(&self, path: &Path) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let text = std::fs::read_to_string(path)?;
        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem::new(uri, "rust".to_string(), 0, text),
        };
        let notification = create_lsp_notification::<lsp_types::notification::DidOpenTextDocument>(params);
        self.writer.send(notification)?;
        Ok(())
    }

    pub fn did_change(&self, path: &Path, content: &str, version: i32) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let params = lsp_types::DidChangeTextDocumentParams {
            text_document: lsp_types::VersionedTextDocumentIdentifier::new(uri, version),
            content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: content.to_string(),
            }],
        };
        let notification = create_lsp_notification::<lsp_types::notification::DidChangeTextDocument>(params);
        self.writer.send(notification)?;
        Ok(())
    }

    pub fn completion(&self, path: &Path, line: u32, col: u32) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let params = lsp_types::CompletionParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position { line, character: col },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        };
        let request = create_lsp_request::<Completion>(2, params);
        self.writer.send(request)?;
        Ok(())
    }

    pub fn hover(&self, path: &Path, line: u32, col: u32) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position { line, character: col },
            },
            work_done_progress_params: Default::default(),
        };
        let request = create_lsp_request::<HoverRequest>(3, params);
        self.writer.send(request)?;
        Ok(())
    }

    pub fn goto_definition(&self, path: &Path, line: u32, col: u32) -> Result<()> {
        if self.is_dummy { return Ok(()); }
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let params = lsp_types::GotoDefinitionParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri },
                position: lsp_types::Position { line, character: col },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let request = create_lsp_request::<GotoDefinition>(4, params);
        self.writer.send(request)?;
        Ok(())
    }
}