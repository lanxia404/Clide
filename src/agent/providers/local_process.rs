use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use crate::agent::config::LocalProcessConfig;
use crate::agent::{AgentEvent, AgentRequest, AgentResponse};

use super::AgentBackend;

/// `AgentBackend` 的本地程序實作，用於與透過標準輸入/輸出（stdio）進行通訊的子程序互動。
pub struct LocalProcessBackend {
    /// 持有子程序的控制代碼。
    child: Child,
    /// 子程序的標準輸入（stdin）的寫入端。
    stdin: ChildStdin,
    /// 一個 MPSC channel 的接收端，用於從 I/O 執行緒接收事件。
    events: Receiver<AgentEvent>,
    /// 用於在 UI 中顯示的代理標籤。
    label: String,
    /// 追蹤是否已發送過 Terminated 事件的旗標。
    terminated: bool,
}

impl LocalProcessBackend {
    /// 根據提供的設定，啟動一個新的本地程序代理。
    ///
    /// 此函式會：
    /// 1. 設定 `Command` 來執行指定的程式。
    /// 2. 將子程序的 stdin, stdout, stderr 重新導向到管道（pipes）。
    /// 3. 啟動子程序。
    /// 4. 為 stdout 和 stderr 分別建立一個監聽執行緒。
    /// 5. 回傳一個 `LocalProcessBackend` 實例。
    pub fn start(config: &LocalProcessConfig, workspace_root: &Path) -> Result<Self> {
        let label = Path::new(&config.program)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&config.program)
            .to_string();

        let mut command = Command::new(&config.program);
        command.args(&config.args);
        if let Some(dir) = config.working_dir.as_ref() {
            command.current_dir(dir);
        } else {
            command.current_dir(workspace_root);
        }
        command.envs(&config.env);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::NotFound {
                    return Err(anyhow!("找不到代理執行檔：{}", config.program));
                }
                return Err(anyhow!("無法啟動代理程序 {}: {}", label, err));
            }
        };

        // 從子程序中取得 I/O 控制代碼。
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("代理 stdin 管道不存在"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("代理 stdout 管道不存在"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("代理 stderr 管道不存在"))?;

        // 建立一個 MPSC channel，用於將來自 I/O 執行緒的事件傳送給主執行緒。
        let (tx, rx) = mpsc::channel();

        // 為 stdout 和 stderr 建立監聽執行緒。
        spawn_stdout_listener(stdout, tx.clone(), label.clone());
        spawn_stderr_listener(stderr, tx);

        let mut backend = Self {
            child,
            stdin,
            events: rx,
            label,
            terminated: false,
        };

        // 如果設定了初始 stdin 內容，則在啟動後立即寫入。
        if let Some(initial) = &config.stdin_initial {
            backend.write_raw(initial)?;
        }

        Ok(backend)
    }

    /// 將原始字串寫入子程序的 stdin。
    fn write_raw(&mut self, payload: &str) -> Result<()> {
        writeln!(self.stdin, "{}", payload).context("寫入代理請求失敗")?;
        self.stdin.flush().context("刷新代理 stdin 失敗")?;
        Ok(())
    }
}

/// 建立一個新執行緒來監聽子程序的標準輸出 (stdout)。
///
/// 這個執行緒會逐行讀取 stdout，嘗試將每一行解析為 `AgentResponse` JSON 物件，
/// 並將結果作為 `AgentEvent` 發送到 MPSC channel。
fn spawn_stdout_listener(
    stdout: std::process::ChildStdout,
    tx: mpsc::Sender<AgentEvent>,
    connection_label: String,
) {
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        // 首先發送一個 Connected 事件。
        let _ = tx.send(AgentEvent::Connected(connection_label));
        // 逐行讀取 stdout。
        for line in reader.lines() {
            match line {
                Ok(raw) => {
                    if raw.trim().is_empty() {
                        continue;
                    }
                    // 嘗試將每一行解析為 AgentResponse JSON。
                    match serde_json::from_str::<AgentResponse>(&raw) {
                        Ok(response) => {
                            // 如果 channel 已關閉，則中斷循環。
                            if tx.send(AgentEvent::Response(response)).is_err() {
                                break;
                            }
                        }
                        Err(_) => {
                            let _ =
                                tx.send(AgentEvent::Error(format!("代理回應解析失敗: {}", raw)));
                        }
                    }
                }
                Err(err) => {
                    let _ = tx.send(AgentEvent::Error(format!("讀取代理標準輸出失敗: {err}")));
                    break;
                }
            }
        }
        // 當 stdout 串流結束時，發送一個 Terminated 事件。
        let _ = tx.send(AgentEvent::Terminated);
    });
}

/// 建立一個新執行緒來監聽子程序的標準錯誤 (stderr)。
///
/// 這個執行緒會逐行讀取 stderr，並將每一行都作為一個錯誤事件 (`AgentEvent::Error`)
/// 發送到 MPSC channel。
fn spawn_stderr_listener(stderr: std::process::ChildStderr, tx: mpsc::Sender<AgentEvent>) {
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(raw) => {
                    // 將 stderr 的每一行都當作一個錯誤事件來發送。
                    if tx.send(AgentEvent::Error(format!("代理錯誤輸出: {raw}")))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    let _ = tx.send(AgentEvent::Error(format!("讀取代理錯誤輸出失敗: {err}")));
                    break;
                }
            }
        }
    });
}

#[async_trait]
impl AgentBackend for LocalProcessBackend {
    fn name(&self) -> &str {
        &self.label
    }

    /// 將 `AgentRequest` 序列化為 JSON 字串，並寫入子程序的 stdin。
    /// 雖然 trait 要求此方法是 `async` 的，但對於本地程序 I/O，
    /// 寫入操作通常足夠快，可以視為同步操作。
    async fn send(&mut self, request: AgentRequest) -> Result<()> {
        let payload = serde_json::to_string(&request).context("序列化代理請求失敗")?;
        self.write_raw(&payload)
    }

    /// 從事件 channel 中輪詢一個事件。
    /// 這是一個非阻塞操作。
    fn poll_event(&mut self) -> Option<AgentEvent> {
        if self.terminated {
            return None;
        }

        match self.events.try_recv() {
            Ok(event) => {
                if matches!(event, AgentEvent::Terminated) {
                    self.terminated = true;
                }
                Some(event)
            }
            Err(TryRecvError::Empty) => None, // 沒有事件
            Err(TryRecvError::Disconnected) => {
                self.terminated = true;
                Some(AgentEvent::Terminated)
            }
        }
    }
}

/// `Drop` trait 的實作確保當 `LocalProcessBackend` 實例被銷毀時，
/// 其管理的子程序會被可靠地終止。
impl Drop for LocalProcessBackend {
    fn drop(&mut self) {
        // 嘗試殺死子程序，忽略任何錯誤。
        let _ = self.child.kill();
    }
}
