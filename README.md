# Clide

Clide 是一個以命令列為核心的多窗格 IDE 原型，專為結合 Rust 核心與 Python 代理流程而設計。介面借鑒 Microsoft Edit，提供檔案樹、編輯器、整合終端與代理面板四大區塊，支援鍵盤與滑鼠操作，並預留 LSP、Git 與 AI 同步的擴充能力。

## 功能概覽
- 三欄布局：左側檔案樹、中央編輯器+終端、右側代理建議。 
- Ropey 驅動的文字緩衝區，支援基本插入、刪除、游標移動與捲動。 
- 整合終端視窗，可滾動查看歷史輸出。 
- 代理面板顯示 AI 建議與差異摘要，預留 IPC 介面供 Python 代理使用。 
- 支援滑鼠與鍵盤快捷鍵：`Tab` 切換焦點、`Ctrl+Q` 離開、方向鍵操作。 

## 建置與執行
```bash
cargo run      # 以除錯模式建置並啟動 Clide
cargo build    # 建置執行檔，產出於 target/debug/clide
cargo build --release  # 最佳化建置，產出於 target/release/clide
```

## 專案結構
- `src/`: Rust 核心，含 `app.rs`, `editor.rs`, `ui.rs`。
- `config/`: JSON 配置與主題/鍵盤映射範例。
- `python/`: 代理範例及插件宣告，示範與核心協作方式。
- `AGENTS.md`: 倉庫指南（已加入 `.gitignore` 供本地參考）。

## 後續規劃
- 接入真正的 LSP 與 Git 整合，實現語法提示與版本控制操作。
- 定義代理通訊協定，完成即時同步與衝突處理。
- 擴充插件系統，允許以 JSON/Python 描述命令、工作流與主題。
