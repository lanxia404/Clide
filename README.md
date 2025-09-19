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
- `src/main.rs`: Main application entry point. Initializes the `tokio` runtime, logging, sets up and restores the terminal, and runs the main event loop.
- `src/app/`: Core application logic module.
    - `mod.rs`: Declares all submodules and re-exports the main `App` state struct.
    - `state.rs`: Defines the core `App` struct and all UI component states (e.g., `OverlayState`, `AgentComposer`).
    - `init.rs`: Responsible for the initialization of the `App` struct.
    - `keyboard.rs`, `mouse.rs`: Handle keyboard and mouse event dispatch, respectively.
    - `layout.rs`, `overlays.rs`, `menu.rs`: Manage UI layout, overlays, and menu logic.
    - `files.rs`: Handles all file system-related operations (open, save, delete).
    - `agent.rs`: Logic for the interaction between the `App` and the agent manager.
    - `tick.rs`: Handles the application's periodic update events.
    - `actions.rs`: Centralizes the execution logic for all user commands (from menus or the command palette).
- `src/agent/`: Agent management and communication module.
    - `manager.rs`: Home of the `AgentManager`, which is responsible for the agent lifecycle, profile management, and event polling.
    - `message.rs`: Defines `AgentRequest` and `AgentResponse`, the communication protocol between the app and agents.
    - `providers/`: Contains concrete implementations for communicating with different agent backends.
        - `http/`: Communicates with remote services (like OpenAI, Gemini) via HTTP APIs.
        - `local_process.rs`: Interacts with local subprocesses via stdio.
- `src/ui/`: TUI rendering logic module.
    - `mod.rs`: Contains all `ratatui` rendering functions that draw the `App` state to the terminal.
    - `theme.rs`: Centralizes all UI color constants.
- `src/editor.rs`: The core text editor based on `ropey`, handling the text buffer, cursor movement, syntax highlighting, etc.
- `src/file_tree.rs`: Data structure and traversal logic for the file tree.
- `src/panels/`: Defines the data structures for the main UI panels (e.g., `AgentPanel`, `TerminalPane`).
- `src/definitions.rs`: Contains core data structures and enums shared across the project (e.g., `FocusArea`, `LayoutState`, `CommandAction`).
- `python/`: Provides agent and plugin examples (`agent_stub.py`, `plugins/example_plugin.json`), demonstrating interaction with the main program via JSON IPC.
- `config/`: Contains default configuration files, such as `agents.example.toml`.

## Roadmap
- Integrate LSP, Git, and process management to graduate from prototype to daily driver.
- Expand agent APIs with permission gating and batch apply/diff workflows.
- Tighten automated testing and add visual regression coverage for pane layout interactions.

Enjoy exploring the prototype and feel free to file issues or ideas under the project tracker!
