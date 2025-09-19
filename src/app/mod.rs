//! `app` 模組是 Clide 應用程式的核心。
//!
//! 它負責管理應用程式的整體狀態、處理使用者輸入（鍵盤、滑鼠）、
//! 並協調 UI 的不同元件。
//!
//! 這個 `mod.rs` 檔案作為 `app` 模組的入口點，
//! 負責宣告其所有子模組，並為了方便起見重新導出關鍵類型。

// --- 子模組宣告 ---
// 每個 `mod` 都對應一個 `.rs` 檔案，將應用程式的邏輯劃分為不同的職責領域。

/// `agent` 模組：處理與 `AgentManager` 的互動邏輯，如提交提示、處理來自代理的事件。
mod agent;
/// `files` 模組：包含與檔案系統操作相關的邏輯，如讀取、寫入、建立和刪除檔案。
mod files;
/// `init` 模組：負責 `App` 結構的初始化和設定。
mod init;
/// `keyboard` 模組：專門處理所有的鍵盤輸入事件。
mod keyboard;
/// `layout` 模組：管理 UI 窗格的佈局計算和繪製邏輯。
mod layout;
/// `menu` 模組：處理頂部選單欄的相關邏輯。
mod menu;
/// `mouse` 模組：專門處理所有的滑鼠輸入事件。
mod mouse;
/// `overlays` 模組：管理彈出式視窗（如命令面板、輸入提示）的狀態和邏輯。
mod overlays;
/// `preferences` 模組：處理與使用者偏好設定相關的邏輯。
mod preferences;
/// `state` 模組：定義了 `App` 結構以及所有核心的狀態類型。
mod state;
/// `tick` 模組：處理應用程式的定時更新事件（tick）。
mod tick;
mod actions;

// --- 公共 API 重新導出 ---
// 為了方便其他模組使用，我們將 `state` 子模組中定義的核心類型
// 透過 `pub use` 提升到 `app` 模組的頂層命名空間。
// 這樣，其他模組就可以使用 `crate::app::App` 而不是 `crate::app::state::App`，
// 使引用路徑更短、更直觀。
pub use state::{
    AgentComposer, AgentSwitcherState, App, CommandAction, CommandPaletteEntry,
    CommandPaletteState, ConfirmDeleteState, InputPromptState, OverlayState, PendingInputAction,
};

use crate::definitions::{DividerKind, FocusArea, PaneKind};


// 定義編輯器行號區域的寬度。
const EDITOR_GUTTER_WIDTH: u16 = 7;