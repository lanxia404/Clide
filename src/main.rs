// src/main.rs
pub mod app;
pub mod components;
pub mod core;
pub mod event;
pub mod features;
pub mod tui;
pub mod ui;

use anyhow::Result;
use app::App;
use core::lsp;
use crossterm::event::{Event as CrosstermEvent, EventStream};
use event::Event;
use lsp_types::{
    notification::{Notification, PublishDiagnostics},
    PublishDiagnosticsParams,
};
use std::{panic, time::Duration};
use tokio_stream::StreamExt;
use tui::{init, restore};
use ui::layout::render;
use url::Url;

#[tokio::main]
async fn main() -> Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));

    let mut tui = init()?;
    let mut app = App::new()?;

    let mut stream = EventStream::new();
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    while app.running {
        tui.draw(|frame| render(&mut app, frame))?;

        let event = tokio::select! {
            _ = interval.tick() => Event::Tick,

            maybe_event = stream.next() => {
                match maybe_event {
                    Some(Ok(CrosstermEvent::Key(key))) => Event::Key(key),
                    Some(Ok(CrosstermEvent::Mouse(mouse))) => Event::Mouse(mouse),
                    Some(Ok(CrosstermEvent::Resize(_, _))) => {
                        app.clear_editor_cache();
                        continue;
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => break,
                }
            },

            Some(lsp_message) = app.lsp_receiver.recv() => {
                match lsp_message {
                    lsp::LspMessage::Notification(method, params) => {
                        if method == PublishDiagnostics::METHOD {
                            if let Ok(diagnostics) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                                if let Ok(url) = Url::parse(diagnostics.uri.as_str()) {
                                    app.diagnostics.insert(url, diagnostics.diagnostics);
                                }
                            }
                        }
                    }
                    lsp::LspMessage::Response(id, result) => {
                        match id {
                            1 => { // Initialize
                                app.lsp_status = app::LspStatus::Ready;
                            }
                            2 => { // Completion
                                if let Ok(Some(lsp_types::CompletionResponse::Array(items))) = serde_json::from_value(result.clone()) {
                                    if !items.is_empty() {
                                        app.completion_selection = Some(0);
                                    }
                                    app.completion_list = Some(items);
                                } else {
                                    eprintln!("[Clide DEBUG] Failed to deserialize completion response or got empty response: {:?}", result);
                                }
                            }
                            3 => { // Hover
                                if let Ok(Some(hover)) = serde_json::from_value(result) {
                                    app.hover_info = Some(hover);
                                }
                            }
                            4 => { // Go to Definition
                                if let Ok(Some(lsp_types::GotoDefinitionResponse::Scalar(location))) = serde_json::from_value(result) {
                                    if let Ok(url) = url::Url::parse(location.uri.as_str()) {
                                        if let Ok(path) = url.to_file_path() {
                                            let _ = app.open_file(path);
                                            app.editor.move_cursor_to(location.range.start.line as usize, location.range.start.character as usize);
                                        }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    lsp::LspMessage::Error(id, error) => {
                        eprintln!("[Clide LSP ERROR] ID: {}, Body: {:?}", id, error);
                        if id == 1 { // Initialization failed
                            app.lsp_status = app::LspStatus::Failed;
                        }
                    }
                    lsp::LspMessage::Stderr(msg) => {
                        app.lsp_message = Some(msg);
                    }
                }
                continue;
            }
        };

        match event {
            Event::Tick => app.tick(),
            _ => app.handle_event(event),
        }
    }

    restore()?;
    Ok(())
}