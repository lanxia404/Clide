
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
use event::{Event, EventHandler};
use tui::{init, restore};
use ui::render;

#[tokio::main]
async fn main() -> Result<()> {
    let mut tui = init()?;
    let mut app = App::new()?;
    let mut event_handler = EventHandler::new(250);

    while app.running {
        tui.draw(|frame| render(&mut app, frame))?;
        
        if let Some(event) = event_handler.next().await {
            match event {
                Event::Tick => app.tick(),
                _ => app.handle_event(event),
            }
        } else {
            break;
        }
    }

    restore()?;
    Ok(())
}

