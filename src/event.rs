use crossterm::event::{KeyEvent, MouseEvent};

/// Application events.
#[derive(Debug)]
pub enum Event {
    /// A tick event, sent at a regular interval.
    Tick,
    /// A key press event.
    Key(KeyEvent),
    /// A mouse event.
    Mouse(MouseEvent),
}