# Clide

[![Build Status](https://github.com/lanxia404/Clide/actions/workflows/build.yml/badge.svg)](https://github.com/lanxia404/Clide/actions/workflows/build.yml)

> Language: [English](README.md) · [繁體中文](README.zh.md)

Clide is a keyboard-first yet mouse-friendly terminal IDE prototype built with Rust and complemented by a Python agent channel. The interface mirrors the classic Microsoft Edit palette and composes four primary panes—file tree, editor, integrated terminal, and agent feed—that can be toggled or resized on the fly.

## Feature Highlights
- **Multipane layout** – Tree/editor/terminal/agent panes can be shown or hidden individually, resized via drag handles, and restored through header clicks.
- **Ropey-powered editor** – Unicode-aware text buffer with real-time soft wrapping, per-line highlighting, and mouse text selection with live reverse-video feedback.
- **Status-aware UI** – Footer exposes wrap mode, EOL, encoding, indent style, cursor location, and dirty state; each item is clickable for quick toggles.
- **Quick actions** – Command palette (Ctrl+Shift+P) exposes toggle and file operations including hidden file visibility, new file creation, and agent controls.
- **Workflow-ready** – Built-in build workflow (`build.yml`) keeps the project linted and compilable on every push.

## Keyboard & Mouse Cheatsheet
- **Global**: `Ctrl+Q` exit · `F6` / `Shift+F6` cycle pane focus · `F10` toggle menu bar.
- **File operations**: `Ctrl+S` save · `Ctrl+Alt+S` save as · `Delete` removes the highlighted file (with confirmation).
- **Editor navigation**: `Home` / `End`, `PageUp` / `PageDown`, arrow keys, and soft-wrap aware cursor movement.
- **Mouse**: Click to focus panes, double-click files to open or folders to expand, drag splitters to resize. Inside the editor, click to reposition the caret and drag to create selections; scroll wheel pans any pane.

## Build & Run
```bash
cargo run               # Launch Clide in debug mode
cargo build             # Compile debug binary into target/debug/clide
cargo build --release   # Produce an optimized release binary
```

## Project Layout
- `src/main.rs` – Crossterm/Ratatui bootstrap and event loop.
- `src/app.rs` – Pane orchestration, input routing, menu actions, and status messaging.
- `src/editor.rs` – Ropey text buffer, Unicode width calculations, cursor/selection syncing, and viewport management.
- `src/file_tree.rs` – Directory traversal, expansion state, hidden file toggles, and file/directory activation.
- `src/panels/` – `terminal.rs` for scrollable terminal output, `agent.rs` for AI suggestion streams.
- `src/definitions.rs` – Shared enums, menu/toolbar layouts, pane geometry, and divider metadata.
- `python/` – Agent IPC sample (`agent_stub.py`) and plugin manifests.
- `config/` – Layout presets and future theme/keybinding configuration.

## Roadmap
- Integrate LSP, Git, and process management to graduate from prototype to daily driver.
- Expand agent APIs with permission gating and batch apply/diff workflows.
- Tighten automated testing and add visual regression coverage for pane layout interactions.

Enjoy exploring the prototype and feel free to file issues or ideas under the project tracker!
