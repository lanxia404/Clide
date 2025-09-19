# Clide 專案分析 (GEMINI.md)

本文檔由 AI 生成，旨在為開發者提供 Clide 專案的快速概覽。

## 1. 專案概覽

Clide 是一個用 Rust 編寫的現代化、高效能的終端原生 IDE。其設計靈感源於 VS Code，旨在提供一個完全在終端機內運行的開發環境，並特別注重與 AI 代理的整合。

- **語言**: Rust (2021 Edition)
- **核心框架**:
    - **TUI**: `ratatui` 和 `crossterm`
    - **非同步**: `tokio`
- **主要功能**:
    - 響應式 TUI 佈局 (自適應寬度)
    - 具備語法高亮功能的文字編輯器 (`syntect`)
    - 檔案總管 (可展開/摺疊目錄)
    - Language Server Protocol (LSP) 整合 (目前使用 `rust-analyzer`)
    - 國際化支援 (英文、簡體中文、繁體中文)
    - 滑鼠和鍵盤支援

## 2. 架構與模組分解

專案採用了清晰的模組化架構，將狀態管理、UI 渲染和核心邏輯分離。

```
src/
├── main.rs         # 應用程式進入點，主事件迴圈
├── app.rs          # 核心應用程式狀態管理 (App struct)
├── tui.rs          # 終端機介面初始化與恢復
├── event.rs        # 定義應用程式事件枚舉
│
├── ui/             # UI 渲染模組
│   ├── mod.rs      # 聲明 UI 模組並導出 render 函數
│   ├── layout.rs   # 定義主佈局 (頁首、頁尾、內容區)
│   ├── file_tree.rs# 渲染檔案總管 UI
│   └── editor.rs   # 渲染文字編輯器 UI (含語法高亮)
│
├── file_tree.rs    # 檔案樹的資料結構與邏輯
├── editor.rs       # 編輯器的資料結構與文字操作邏輯
├── syntax.rs       # 語法高亮引擎封裝
├── lsp.rs          # LSP 客戶端，與語言伺服器通訊
└── i18n.rs         # 國際化 (i18n) Trait 和語言實作
```

### 核心流程

1.  **`main.rs`**:
    - 初始化終端機 (`tui::init`)。
    - 建立 `App` 實例 (`app::App::new`)，其中包含了所有狀態。
    - 進入 `while app.running` 主迴圈。
    - 使用 `tokio::select!` 同時監聽終端機事件 (鍵盤、滑鼠) 和 LSP 伺服器訊息。
    - 將接收到的事件分派給 `app.handle_event()` 進行處理。
    - 每一幀都呼叫 `tui.draw()`，並傳入 `ui::render` 函數來繪製介面。
2.  **`app.rs`**:
    - `App` 結構體是單一事實來源 (Single Source of Truth)，持有 `FileTree`, `Editor`, `LspClient` 等狀態。
    - `handle_event` 方法根據目前的 `focus` (檔案樹或編輯器) 將事件路由到對應的處理函數。
    - 包含打開檔案、儲存檔案、切換語言、切換焦點等核心業務邏輯。
3.  **`ui/layout.rs`**:
    - `render` 函數是 UI 繪製的入口。
    - 它首先繪製整體佈局 (頁首、內容、頁尾)。
    - 根據終端機寬度和 `app.focus` 狀態，決定是渲染單欄視圖還是雙欄視圖。
    - 呼叫 `render_file_tree` 和 `render_editor` 來繪製具體的 UI 元件。

## 3. 關鍵依賴 (Dependencies)

-   `ratatui`: 用於構建文字使用者介面 (TUI) 的核心函式庫。
-   `crossterm`: 提供底層的終端機操作，如事件處理 (鍵盤、滑鼠)、游標控制、顏色設定等。
-   `tokio`: 非同步執行環境，用於同時處理 UI 事件和 LSP 的 I/O。
-   `syntect`: 高效的語法高亮函式庫。
-   `lsp-types`: 提供 Language Server Protocol 的標準化類型定義。
-   `serde` / `serde_json`: 用於 LSP 訊息的序列化與反序列化。
-   `anyhow`: 提供更方便的錯誤處理。

## 4. 如何構建、運行與測試

### 構建

使用 Cargo 進行構建。建議使用 release 模式以獲得最佳效能。

```bash
cargo build --release
```

### 運行

直接使用 Cargo 運行。

```bash
cargo run --release
```

#### Nerd Font 圖示

若要啟用 Nerd Font 圖示以獲得更好的視覺體驗，請在運行前設定環境變數：

```bash
export CLIDE_ICONS=nerd
cargo run --release
```

### 測試

執行專案的單元測試。

```bash
cargo test
```

## 5. 未來可探索的方向

-   **擴充 LSP 功能**: 實作程式碼補全、懸停提示 (hover)、轉到定義 (go to definition) 等功能。
-   **外掛系統**: 設計一個外掛架構，允許使用者擴充 IDE 的功能。
-   **整合終端機面板**: 在 IDE 中內建一個可互動的終端機面板。
-   **Git 整合**: 直接在 UI 中顯示 Git 狀態、執行 Git 命令。
-   **效能優化**: 對於大型檔案的編輯和渲染進行效能分析與優化，例如虛擬捲動 (virtual scrolling)。
