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

### 核心流程 (詳細說明)

為了解決 LSP `stderr` 直接輸出導致 TUI 渲染混亂的問題，專案採用了**狀態驅動的事件處理模型**。其核心是將所有外部輸入（包括 LSP 的 `stderr`）轉化為結構化的事件，在主事件迴圈中統一處理，更新中央狀態，最後由 UI 進行渲染。

1.  **`lsp.rs` - 事件定義**:
    - `LspMessage` 枚舉被擴展，加入了 `Stderr(String)` 型別。
    - **設計理念**: 這一步是關鍵，它將原始的、非結構化的 `stderr` 字串提升為應用程式可以理解的、有明確語義的事件。

2.  **`app.rs` - 訊息捕獲與狀態管理**:
    - `App` 結構體新增了 `lsp_message: Option<String>` 欄位。它作為一個狀態緩衝區，儲存從 LSP 收到的最新一條 `stderr` 訊息。
    - `start_lsp_server` 方法在啟動 `rust-analyzer` 子程序時，會為其 `stderr` 流建立一個專門的非同步任務 `stderr_task`。
    - `stderr_task` 的職責被徹底改變：它不再使用 `eprintln!` 直接打印，而是讀取 `stderr` 的每一行，將其包裝成 `LspMessage::Stderr` 事件，並透過 `mpsc` channel 發送到主事件迴圈。
    - **設計理念**: 這實現了**輸入/輸出的解耦**。LSP 子程序可以自由地輸出其內部狀態或錯誤，而主應用程式則以一種受控的、非阻塞的方式接收這些訊息，完全避免了對 TUI 渲染的直接干擾。

3.  **`main.rs` - 統一事件迴圈**:
    - `tokio::select!` 巨集是整個應用的心臟。它平等地等待來自不同來源的事件：`crossterm` 的終端機輸入和 `lsp_receiver` 的 LSP 訊息。
    - `match lsp_message` 區塊中新增了一個分支：`lsp::LspMessage::Stderr(msg) => { app.lsp_message = Some(msg); }`。
    - **設計理念**: 當 `Stderr` 訊息到達時，它不會立即觸發任何 UI 操作，而僅僅是更新 `App` 這個**單一事實來源 (Single Source of Truth)**。這確保了所有狀態變更都集中在一個地方，使得邏輯清晰且易於除錯。

4.  **`ui/layout.rs` - 狀態驅動的渲染**:
    - `render_footer` 函數在每一幀被呼叫時，會檢查 `app.lsp_message` 的狀態。
    - **渲染邏輯**: 如果 `app.lsp_message` 是 `Some(msg)`，則將 `msg` 的內容顯示在狀態列的右側。它擁有比一般診斷訊息更高的顯示優先級。如果為 `None`，則回退顯示游標位置的診斷訊息或預設提示。
    - **設計理念**: UI 的繪製完全依賴於 `App` 的當前狀態，而不是由事件直接驅動。這是一個典型的**宣告式 UI** 模式，`ratatui` 正是為此而設計。UI 只是狀態的一個「鏡像」，狀態改變，UI 自動更新，使得渲染邏輯保持簡單和可預測。

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

## 6. 未來開發日程 (TODO)

這部分記錄了專案未來的主要開發方向和宏大目標。

- [ ] **架構重構：更換 TUI 引擎**
  - **目標**: 將目前的 TUI 引擎從 `ratatui` 更換為 `microsoft/edit` 專案的自訂 TUI 引擎。
  - **動機**: 為了更深度地整合 `edit` 專案的效能優化、外觀和底層邏輯，實現更一致的使用者體驗。
  - **涉及範圍**:
    - 移植 `edit` 的 TUI 核心程式碼。
    - 完全重寫 `src/ui/` 目錄下的所有渲染邏輯。
    - 重構 `main.rs` 的主事件迴圈。
    - 調整 `app.rs` 的狀態管理以適應新引擎。
  - **備註**: 這是一項巨大的工程，需要對兩個專案的原始碼都有深入的理解。