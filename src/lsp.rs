use anyhow::Result;
use lsp_types::{
    notification::{Notification, DidOpenTextDocument, PublishDiagnostics},
    request::{Initialize, Request},
    ClientCapabilities, DidOpenTextDocumentParams, InitializeParams, PublishDiagnosticsParams,
    TextDocumentItem, Uri, WorkspaceFolder,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::str::FromStr;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
};

#[derive(Debug, Deserialize)]
struct LspNotification {
    method: String,
    params: Value,
}

#[derive(Debug)]
pub enum LspMessage {
    Notification(String, Value),
}

pub struct LspClient {
    #[allow(dead_code)]
    server: Child,
    writer: UnboundedSender<Value>,
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

        let mut server = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = server.stdin.take().unwrap();
        let stdout = server.stdout.take().unwrap();
        let stderr = server.stderr.take().unwrap();

        tokio::spawn(Self::writer_task(stdin, writer_rx));
        tokio::spawn(Self::reader_task(stdout, msg_tx.clone()));
        // Also consume stderr to prevent it from blocking, but we don't need to parse it
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut buffer = Vec::new();
            while let Ok(_) = reader.read_to_end(&mut buffer).await {}
        });

        let client = Self { server, writer: writer_tx };

        Ok((client, msg_rx))
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
        let uri_string = format!("file://{}", root_path.to_string_lossy());
        let root_uri = Uri::from_str(&uri_string)
            .map_err(|_| anyhow::anyhow!("Failed to create root URI"))?;
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
        let uri_string = format!("file://{}", path.to_string_lossy());
        let uri = Uri::from_str(&uri_string)
            .map_err(|_| anyhow::anyhow!("Failed to create file URI"))?;
        let text = std::fs::read_to_string(path)?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem::new(uri, "rust".to_string(), 0, text),
        };
        let notification = create_lsp_notification::<DidOpenTextDocument>(params);
        self.writer.send(notification)?;
        Ok(())
    }
}
