// src/lsp.rs
use anyhow::Result;
use lsp_types::{
    notification::Notification,
    request::{Completion, GotoDefinition, HoverRequest, Request}, Uri,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;
use tokio::sync::mpsc::UnboundedSender;
use url::Url;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LspResponse {
    Success { id: u64, result: Value },
    #[allow(dead_code)]
    Error { id: u64, error: Value },
}

#[derive(Debug, Deserialize)]
pub struct LspNotification {
    pub method: String,
    pub params: Value,
}

#[derive(Debug)]
pub enum LspMessage {
    Notification(String, Value),
    Response(u64, Value),
    Error(u64, Value),
    Stderr(String),
}

pub struct LspClient {
    writer: UnboundedSender<Value>,
}

fn create_lsp_request<R: Request>(id: u64, params: R::Params) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": R::METHOD, "params": params })
}

fn create_lsp_notification<N: Notification>(params: N::Params) -> Value {
    json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params })
}

impl LspClient {
    pub fn new(writer: UnboundedSender<Value>) -> Self {
        Self { writer }
    }

    pub fn did_open(&self, path: &Path) -> Result<()> {
        let uri_url = Url::from_file_path(path).map_err(|_| anyhow::anyhow!("Invalid path"))?;
        let uri: Uri = uri_url.as_str().parse().map_err(|e| anyhow::anyhow!("Failed to parse URI: {}", e))?;
        let text = std::fs::read_to_string(path)?;
        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem::new(uri, "rust".to_string(), 1, text),
        };
        let notification = create_lsp_notification::<lsp_types::notification::DidOpenTextDocument>(params);
        self.writer.send(notification)?;
        Ok(())
    }

    pub fn did_change(&self, path: &Path, content: &str, version: i32) -> Result<()> {
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
