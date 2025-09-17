mod app;
mod editor;
mod ui;

use std::io::{self, stdout};
use std::time::Duration;

use anyhow::Result;
use app::App;
use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

fn main() -> Result<()> {
    let workspace = std::env::current_dir()?;
    let mut app = App::new(workspace)?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("error: {err:?}");
    }

    Ok(())
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Resize(_, _) => {}
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                Event::Paste(data) => {
                    for ch in data.chars() {
                        let key = crossterm::event::KeyEvent::new(
                            crossterm::event::KeyCode::Char(ch),
                            crossterm::event::KeyModifiers::NONE,
                        );
                        app.handle_key(key);
                    }
                }
                Event::FocusGained | Event::FocusLost => {}
            }
        } else {
            app.on_tick();
        }
    }
    Ok(())
}
