use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use tokio::sync::mpsc;
use tokio::time::{self, Duration, Instant};

#[derive(Debug)]
pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
}

pub struct EventHandler {
    sender: mpsc::UnboundedSender<Event>,
    receiver: mpsc::UnboundedReceiver<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: u64) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        let sender_clone = sender.clone();

        tokio::spawn(async move {
            let tick_duration = Duration::from_millis(tick_rate);
            let mut last_tick = Instant::now();
            loop {
                let timeout = tick_duration.saturating_sub(last_tick.elapsed());
                if event::poll(timeout).unwrap_or(false) {
                    match event::read() {
                        Ok(CrosstermEvent::Key(key)) => sender_clone.send(Event::Key(key)).ok(),
                        Ok(CrosstermEvent::Mouse(mouse)) => sender_clone.send(Event::Mouse(mouse)).ok(),
                        Ok(CrosstermEvent::Resize(w, h)) => sender_clone.send(Event::Resize(w, h)).ok(),
                        _ => None,
                    };
                }
                if last_tick.elapsed() >= tick_duration {
                    sender_clone.send(Event::Tick).ok();
                    last_tick = Instant::now();
                }
            }
        });

        Self { sender, receiver }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.receiver.recv().await
    }
}