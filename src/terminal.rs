use crossterm::event::{KeyCode, KeyEvent};
use tokio::process::Command;
use tokio::sync::mpsc;

pub struct TerminalState {
    pub command_history: Vec<String>,
    pub output_buffer: Vec<String>,
    pub input_line: String,
    output_rx: mpsc::Receiver<String>,
    output_tx: mpsc::Sender<String>,
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

    pub fn poll_output(&mut self) {
        while let Ok(output_line) = self.output_rx.try_recv() {
            self.output_buffer.push(output_line);
        }
    }

    fn execute_command(&mut self) {
        let command_str = self.input_line.trim().to_string();
        if command_str.is_empty() {
            return;
        }

        self.command_history.push(command_str.clone());
        self.output_buffer.push(format!("> {}", command_str));
        self.input_line.clear();

        let mut parts = command_str.split_whitespace();
        let command = parts.next().unwrap_or("").to_string();
        let args: Vec<String> = parts.map(String::from).collect();
        let tx = self.output_tx.clone();

        tokio::spawn(async move {
            let output_result = Command::new(command).args(args).output().await;
            match output_result {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !stdout.is_empty() {
                        for line in stdout.trim().lines() {
                            let _ = tx.send(line.to_string()).await;
                        }
                    }
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() {
                        for line in stderr.trim().lines() {
                            let _ = tx.send(line.to_string()).await;
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("Error: {}", e)).await;
                }
            }
        });
    }
}