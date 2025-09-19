// 專案模組宣告
// 這些 `mod` 宣告將對應檔案或目錄的程式碼引入到當前作用域。
mod agent;
mod app;
mod definitions;
mod editor;
mod file_tree;
mod panels;
mod ui;

// --- 外部依賴引入 ---
use anyhow::Result; // anyhow 用於提供更具上下文的錯誤處理。
use app::App; // 引入核心的 App 結構，它管理整個應用程式的狀態。
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEvent, KeyModifiers,
}; // crossterm 用於處理底層終端機事件，如鍵盤、滑鼠輸入。
use crossterm::execute; // 用於在終端機上執行命令。
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
}; // 用於控制終端機模式，如 raw 模式和備用螢幕。
use futures_util::StreamExt; // 提供了對非同步 Stream 的擴充方法，這裡是為了能使用 `events.next()`。
use log::{info, warn}; // log crate 用於日誌記錄。
use ratatui::backend::CrosstermBackend; // ratatui 的後端，用於將 TUI 元件繪製到由 crossterm 管理的終端機上。
use ratatui::Terminal; // ratatui 的核心元件，代表終端機介面。
use std::io::{self, stdout}; // 標準 I/O 函式庫。
use tokio::time::interval; // tokio 用於非同步執行，這裡用來建立一個定時器（ticker）。

/// 應用程式的主入口點。
///
/// `#[tokio::main]` 宏會將 `main` 函式轉換為一個非同步的 Tokio runtime，
/// 這使得我們可以在 `main` 函式中使用 `.await`。
#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日誌系統。
    init_logging();

    // --- 應用程式初始化 ---
    info!("啟動 Clide 介面");
    let workspace = std::env::current_dir()?;
    info!("工作區：{}", workspace.display());
    let mut app = App::new(workspace)?;

    // --- 終端機設定 ---
    let mut terminal = setup_terminal()?;

    // --- 主循環 ---
    let result = run(&mut terminal, &mut app).await;

    // --- 清理與恢復 ---
    // 在應用程式結束前，無論成功或失敗，都必須恢復終端機的原始狀態。
    restore_terminal(&mut terminal)?;

    // 如果主循環回傳了錯誤，將其印出到 stderr。
    if let Err(err) = result {
        eprintln!("應用程式發生錯誤：{err:?}");
    }

    Ok(())
}

/// 設定並初始化終端機。
///
/// 此函數會執行以下操作：
/// 1. 啟用 raw 模式：這會關閉終端機的標準行緩衝和字元回顯，讓應用程式可以即時處理每一個按鍵事件。
/// 2. 進入備用螢幕：切換到一個新的終端機緩衝區，應用程式結束時會恢復，不會污染使用者的終端機歷史。
/// 3. 啟用滑鼠捕獲：允許應用程式接收滑鼠點擊和移動事件。
/// 4. 建立並回傳一個新的 `Terminal` 實例以供後續的 UI 繪製。
fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// 恢復終端機至其原始狀態。
///
/// 此函數會執行以下操作：
/// 1. 停用 raw 模式。
/// 2. 離開備用螢幕，返回到使用者原本的終端機畫面。
/// 3. 停用滑鼠事件捕獲。
/// 4. 重新顯示游標。
fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}


/// 應用程式的主事件循環。
///
/// 這個函式負責處理使用者輸入、定時更新和畫面重繪。
///
/// # Arguments
/// * `terminal` - TUI 終端機介面的可變引用。
/// * `app` - `App` 結構的可變引用，代表應用程式的狀態。
async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    // 從 crossterm 建立一個非同步的事件流。
    let mut events = EventStream::new();
    // 從 app 設定中取得 tick 頻率，用於觸發定時更新。
    let tick_rate = app.tick_rate;
    // 建立一個非同步計時器，它會以 `tick_rate` 的間隔觸發。
    let mut ticker = interval(tick_rate);

    // 開始無限循環，直到 `app.should_quit` 標記為 true。
    loop {
        // 在每次循環開始時，呼叫 `terminal.draw` 來繪製整個 TUI 介面。
        // `ui::render` 函式負責定義 UI 的具體佈局和內容。
        terminal.draw(|f| ui::render(f, app))?;

        // 檢查退出標記。
        if app.should_quit {
            break;
        }

        // `tokio::select!` 宏可以同時等待多個非同步操作，並在其中任何一個完成時繼續執行。
        // 這對於同時處理使用者輸入和背景任務（如計時器）至關重要。
        tokio::select! {
            // 等待下一個終端機事件 (鍵盤、滑鼠等)。
            maybe_event = events.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) => app.handle_key(key).await, // 處理按鍵事件
                    Some(Ok(Event::Mouse(mouse))) => app.handle_mouse(mouse).await, // 處理滑鼠事件
                    Some(Ok(Event::Paste(data))) => { // 處理貼上事件
                        // 逐字元處理貼上內容。對於非常大的貼上內容，這可能是潛在的性能瓶頸。
                        // 如果未來遇到效能問題，可以考慮將其改為批次處理或更有效率的方式。
                        for ch in data.chars() {
                            let key = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
                            app.handle_key(key).await;
                        }
                    }
                    // 目前暫不處理這些事件，但保留匹配分支以便未來擴充。
                    Some(Ok(Event::FocusGained)) | Some(Ok(Event::FocusLost)) => {}
                    Some(Ok(Event::Resize(_, _))) => {} // 視窗大小改變事件，ratatui 會自動處理重繪。
                    Some(Err(err)) => { // 處理事件流中的錯誤。
                        warn!("事件讀取錯誤：{err}");
                        app.status_message = format!("事件讀取錯誤：{}", err);
                    }
                    None => break, // 如果事件流結束 (例如，終端機關閉)，則中斷循環。
                }
            }
            // 等待計時器的下一個 tick 事件。
            _ = ticker.tick() => {
                // 呼叫 app 的 `on_tick` 方法，用於處理需要定期更新的狀態。
                app.on_tick();
            }
        }
    }
    Ok(())
}

/// 初始化應用程式的日誌系統。
///
/// 此函數會嘗試從 `config/log4rs.yaml` 檔案初始化 `log4rs` 日誌系統。
/// 如果設定檔載入失敗（例如，檔案不存在或格式錯誤），它會印出錯誤訊息到 stderr，
/// 然後退回使用 `env_logger` 作為備用方案，確保應用程式仍有基本的日誌輸出。
///
/// 此外，此函數還會設定一個全域的 panic hook，用於捕獲並記錄任何未處理的 panic。
fn init_logging() {
    // 確保日誌目錄存在
    std::fs::create_dir_all("logs").ok();

    // 設定全域 panic hook
    std::panic::set_hook(Box::new(|panic_info| {
        let payload = panic_info.payload().downcast_ref::<&str>().unwrap_or(&"");
        let location = panic_info.location().unwrap();
        log::error!(
            "Panic occurred at {}:{}:{}: {}",
            location.file(),
            location.line(),
            location.column(),
            payload
        );
    }));

    match log4rs::init_file("config/log4rs.yaml", Default::default()) {
        Ok(_) => info!("日誌系統已從 config/log4rs.yaml 成功初始化"),
        Err(err) => {
            eprintln!("無法初始化日誌系統：{}，將使用預設設定", err);
            // 如果設定失敗，則退回簡易日誌記錄器
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
                .format_timestamp_secs()
                .try_init()
                .ok();
        }
    }
}

