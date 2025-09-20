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
use crossterm::event::{Event as CrosstermEvent, EventStream};
use event::Event;
use std::{panic, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tui::{init, restore};
use ui::layout::render;

#[tokio::main]
async fn main() -> Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));

    let mut tui = init()?;
    let mut app = App::new()?;

    // Setup LSP and plugin systems after App is created
    let (lsp_writer_tx, lsp_writer_rx) = mpsc::unbounded_channel();
    let (lsp_event_tx, lsp_event_rx) = mpsc::unbounded_channel();
    app.lsp_client.writer = Some(lsp_writer_tx.clone());
    app.lsp_receiver = lsp_event_rx;
    app.start_lsp_server(lsp_writer_tx, lsp_writer_rx, lsp_event_tx);
    app.plugin_manager.load_plugins();

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
                app.handle_lsp_message(lsp_message);
                continue;
            }

            Some(task_result) = app.task_receiver.recv() => {
                app.handle_task_result(task_result);
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