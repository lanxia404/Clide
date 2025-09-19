pub mod app;
pub mod editor;
pub mod event;
pub mod file_tree;
pub mod i18n;
pub mod lsp;
pub mod syntax;
pub mod tui;
pub mod ui;

use anyhow::Result;
use app::App;
use crossterm::event::{Event as CrosstermEvent, EventStream};
use event::Event;
use lsp_types::{
    notification::{Notification, PublishDiagnostics},
    PublishDiagnosticsParams,
};
use std::time::Duration;
use tokio_stream::StreamExt;
use tui::{init, restore};
use ui::render;

#[tokio::main]
async fn main() -> Result<()> {
    let mut tui = init()?;
    let mut app = App::new()?;

    let mut stream = EventStream::new();
    let mut interval = tokio::time::interval(Duration::from_millis(250));

    while app.running {
        tui.draw(|frame| render(&mut app, frame))?;

        let event = tokio::select! {
            _ = interval.tick() => Event::Tick,

            maybe_event = stream.next() => {
                match maybe_event {
                    Some(Ok(CrosstermEvent::Key(key))) => Event::Key(key),
                    Some(Ok(CrosstermEvent::Mouse(mouse))) => Event::Mouse(mouse),
                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => break,
                }
            },

            Some(lsp_message) = app.lsp_receiver.recv() => {
                if let lsp::LspMessage::Notification(method, params) = lsp_message {
                    if method == PublishDiagnostics::METHOD {
                        if let Ok(diagnostics) = serde_json::from_value::<PublishDiagnosticsParams>(params) {
                            app.diagnostics.insert(diagnostics.uri, diagnostics.diagnostics);
                        }
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
