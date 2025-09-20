use crossterm::event::{KeyCode, KeyEvent};
use tokio::process::Command;
use tokio::sync::mpsc;

const MAX_TERMINAL_OUTPUT_LINES: usize = 5000;

pub struct TerminalState {
    pub command_history: Vec<String>,
    pub output_buffer: Vec<String>,
    pub input_line: String,
    output_rx: mpsc::Receiver<String>,
    output_tx: mpsc::Sender<String>,
}

impl Default for TerminalState {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalState {
    pub fn new() -> Self {
        let (output_tx, output_rx) = mpsc::channel(100);
        Self {
            command_history: Vec::new(),
            output_buffer: vec!["Welcome to the integrated terminal!".to_string()],
            input_line: String::new(),
            output_rx,
            output_tx,
        }
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char(c) => {
                self.input_line.push(c);
            }
            KeyCode::Backspace => {
                self.input_line.pop();
            }
            KeyCode::Enter => {
                self.execute_command();
            }
            _ => {}
        }
    }

    fn truncate_buffer(&mut self) {
        if self.output_buffer.len() > MAX_TERMINAL_OUTPUT_LINES {
            let to_remove = self.output_buffer.len() - MAX_TERMINAL_OUTPUT_LINES;
            self.output_buffer.drain(0..to_remove);
        }
    }

    pub fn poll_output(&mut self) {
        while let Ok(output_line) = self.output_rx.try_recv() {
            self.output_buffer.push(output_line);
        }
        self.truncate_buffer();
    }

    fn execute_command(&mut self) {
        let command_str = self.input_line.trim().to_string();
        if command_str.is_empty() {
            self.output_buffer.push("> ".to_string());
            self.input_line.clear(); // Clear even if empty to reset prompt
            self.truncate_buffer();
            return;
        }

        self.command_history.push(command_str.clone());
        self.output_buffer.push(format!("> {}", command_str));
        self.truncate_buffer();
        self.input_line.clear();

        let tx = self.output_tx.clone();

        tokio::spawn(async move {
            let output_result = Command::new("bash")
                .arg("-c")
                .arg(command_str) // Execute the whole string in a shell
                .output()
                .await;
            match output_result {
                Ok(output) => {
                    send_output(&output.stdout, &tx).await;
                    send_output(&output.stderr, &tx).await;
                }
                Err(e) => {
                    let _ = tx.send(format!("Error: {}", e)).await;
                }
            }
        });
    }
}

async fn send_output(buffer: &[u8], tx: &mpsc::Sender<String>) {
    let text = String::from_utf8_lossy(buffer);
    if !text.is_empty() {
        for line in text.trim().lines() {
            let _ = tx.send(line.to_string()).await;
        }
    }
}