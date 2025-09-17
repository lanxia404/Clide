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
- `src/main.rs`: 事件迴圈與 Crossterm/Ratatui 初始設定。
- `src/app.rs`: 窗格配置、鍵盤/滑鼠處理、狀態訊息與偏好切換。
- `src/editor.rs`: Ropey 緩衝管理、Unicode 寬度計算、游標/選取與視窗同步邏輯。
- `src/file_tree.rs`: 檔案樹瀏覽、展開狀態與開檔/換目錄操作。
- `src/panels/`: `terminal.rs` 管理終端輸出，`agent.rs` 儲存代理訊息列表。
- `src/definitions.rs`: pane、分隔線、選單、狀態列控制等共用列舉與布局資料結構。
- `config/`: 版面與主題設定 JSON。
- `python/`: 代理示例與插件宣告，展示 Rust <-> Python IPC。

## 後續規劃
- 串接 LSP、Git、實際終端子行程，完善 IDE 實用性。
- 擴充代理 API 與權限控管，支援差異同步、批次接受建議。
- 建立測試與自動化工作流程，確保 Pane 佈局與滑鼠交互穩定。
