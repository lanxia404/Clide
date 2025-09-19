# Clide

[![Build Status](https://github.com/lanxia404/Clide/actions/workflows/build.yml/badge.svg)](https://github.com/lanxia404/Clide/actions/workflows/build.yml)

> 語言切換： [English](README.md) · [繁體中文](README.zh.md)

Clide 是一個命令列導向、可滑鼠操作的多窗格 IDE 原型，結合 Rust 核心與 Python 代理流程。介面借鑑 Microsoft Edit 的 VT framebuffer 配色，提供檔案樹、編輯器、整合終端與代理面板四大區塊，可依需求隱藏或拖曳調整尺寸，並支援鍵盤、滑鼠雙模式控制。

## 功能概覽
- 三欄 + 上下功能列：檔案樹、中央編輯器/終端機堆疊、代理建議動態配置，可依需求顯示或隱藏並即時重算比例。
- Ropey 駆動的文字緩衝區，提供插入、刪除、游標移動與視窗同步；支援 Unicode 寬度計算、長行自動換行，並可滑鼠拖曳選取文字區塊；狀態列列出換行、編碼、縮排與游標座標。
- 內建終端輸出窗格支援捲動瀏覽；代理面板列出佇列中的 AI 建議與敘述。
- 功能列具備選單（F10 開啟），狀態列按鈕可用滑鼠切換換行、編碼、縮排等偏好設定。
- 滑鼠支援點擊切換焦點、雙擊檔案開啟或展開目錄、拖曳分隔線調整 pane，標題列點擊可快速顯示/隱藏；在編輯器中可點擊移動游標、拖曳進行區域選取並自動反白顯示。
- 預留 LSP、Git 與代理同步介面，`python/agent_stub.py` 示範 JSON IPC 流程。

## 建置與執行
```bash
cargo run                 # 以除錯模式啟動 Clide
cargo build               # 建置除錯版執行檔，位於 target/debug/clide
cargo build --release     # 建置最佳化版本，位於 target/release/clide
```

## 操作提示
- 鍵盤：`Ctrl+Q` 離開、`F6`/`Shift+F6` 循環切換可見窗格焦點、`F10` 打開或收起選單；方向鍵與 `Enter` 用於檔案樹與代理面板；`Tab` 依縮排偏好插入空白或 Tab。
- 編輯器支援 `Home`/`End`、`PageUp`/`PageDown`、方向鍵移動游標，`Ctrl+S` 儲存、`Ctrl+Alt+S` 另存新檔、`Delete`/`Backspace` 對選取區塊或單一字元進行刪除。
- 滑鼠：點擊標題切換顯示、拖曳分隔線調整比例、滾輪捲動畫面，雙擊檔案開啟，雙擊資料夾展開/收合；在編輯器拖曳可選取文字，按住左鍵滑動即時更新高亮。
- 狀態列按鈕可點擊切換換行、換行符、編碼、縮排與顯示游標位置；`[SAVE:*]` 表示檔案有未儲存變更，`[SAVE:OK]` 代表乾淨狀態。

## 專案結構
- `src/main.rs`: 應用程式主入口。負責初始化 `tokio` runtime、日誌系統、設定並恢復終端機狀態，以及執行主事件迴圈。
- `src/app/`: 應用程式核心邏輯模組。
    - `mod.rs`: 宣告所有子模組，並重新導出 `App` 狀態結構。
    - `state.rs`: 定義核心的 `App` 結構及所有 UI 元件的狀態 (如 `OverlayState`, `AgentComposer`)。
    - `init.rs`: 負責 `App` 結構的初始化。
    - `keyboard.rs`, `mouse.rs`: 分別處理鍵盤和滑鼠事件的分派。
    - `layout.rs`, `overlays.rs`, `menu.rs`: 管理 UI 佈局、浮層和選單的邏輯。
    - `files.rs`: 處理所有檔案系統相關的操作 (開啟、儲存、刪除)。
    - `agent.rs`: 處理 `App` 與代理管理器之間互動的邏輯。
    - `tick.rs`: 處理應用程式的定時更新事件。
    - `actions.rs`: 集中處理所有使用者命令 (來自選單或指令面板) 的執行邏輯。
- `src/agent/`: 代理管理與通訊模組。
    - `manager.rs`: `AgentManager` 的所在地，負責代理的生命週期、設定檔管理和事件輪詢。
    - `message.rs`: 定義 `AgentRequest` 和 `AgentResponse`，即應用程式與代理之間的通訊協定。
    - `providers/`: 包含與不同代理後端通訊的具體實作。
        - `http/`: 透過 HTTP API 與遠端服務 (如 OpenAI, Gemini) 通訊。
        - `local_process.rs`: 透過標準輸入/輸出與本地子程序互動。
- `src/ui/`: TUI 渲染邏輯模組。
    - `mod.rs`: 包含所有 `ratatui` 的渲染函式，將 `App` 狀態繪製到終端機。
    - `theme.rs`: 集中管理所有 UI 顏色常數。
- `src/editor.rs`: 基於 `ropey` 的文字編輯器核心，處理文字緩衝、游標移動、語法高亮等。
- `src/file_tree.rs`: 檔案樹的資料結構與遍歷邏輯。
- `src/panels/`: 定義了 UI 中主要面板的資料結構 (如 `AgentPanel`, `TerminalPane`)。
- `src/definitions.rs`: 包含整個專案共用的核心資料結構與列舉 (如 `FocusArea`, `LayoutState`, `CommandAction`)。
- `python/`: 提供代理與外掛程式的範例 (`agent_stub.py`, `plugins/example_plugin.json`)，示範如何透過 JSON IPC 與主程式互動。
- `config/`: 包含預設設定檔，如 `agents.example.toml`。

## 後續規劃
- 串接 LSP、Git、實際終端子行程，完善 IDE 實用性。
- 擴充代理 API 與權限控管，支援差異同步、批次接受建議。
- 建立測試與自動化工作流程，確保 Pane 佈局與滑鼠交互穩定。
