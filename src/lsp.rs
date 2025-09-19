use anyhow::Result;
use lsp_types::notification::{Notification, DidOpenTextDocument};
use lsp_types::request::{Initialize, Request};
use lsp_types::{
    ClientCapabilities, DidOpenTextDocumentParams, InitializeParams,
    TextDocumentItem,
    Uri,
};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

pub struct LspClient {
    server: Child,
    writer: UnboundedSender<Value>,
}

fn create_lsp_request<R: Request>(id: u64, params: R::Params) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": R::METHOD,
        "params": params,
    })
}

fn create_lsp_notification<N: Notification>(params: N::Params) -> Value {
     json!({
        "jsonrpc": "2.0",
        "method": N::METHOD,
        "params": params,
    })
}

impl LspClient {
    pub fn new() -> Result<Self> {
        let mut server = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = server.stdin.take().unwrap();
        let stdout = server.stdout.take().unwrap();

        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(Self::writer_task(stdin, rx));
        tokio::spawn(Self::reader_task(stdout));

        Ok(Self { server, writer: tx })
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

    async fn reader_task(stdout: ChildStdout) {
        let mut reader = BufReader::new(stdout);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                break;
            }
        }
    }

    pub fn initialize(&self, root_path: &Path) -> Result<()> {
        let uri_string = format!("file://{}", root_path.to_string_lossy());
        let root_uri = Uri::from_str(&uri_string)
            .map_err(|_| anyhow::anyhow!("Failed to create root URI"))?;
        let params = InitializeParams {
            process_id: Some(std::process::id()),
            root_uri: Some(root_uri),
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