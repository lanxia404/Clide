# Gemini Agent Context for the Clide Project

## Project Overview
- **Project Name:** Clide
- **Description:** A terminal-native IDE built with Rust, inspired by VS Code. It features a file tree, a text editor with syntax highlighting, and aims to integrate an interactive AI agent panel.
- **Tech Stack:** Rust, `ratatui` for the TUI, `crossterm` for terminal backend, `syntect` for syntax highlighting, `lsp-types` for Language Server Protocol.

## Key Files
- `src/main.rs`: Main application entry point and event loop.
- `src/app.rs`: Contains the main `App` struct, which holds the application's state.
- `src/ui.rs`: Handles all rendering logic, defining the layout and drawing widgets.
- `src/tui.rs`: Manages terminal initialization and restoration.
- `src/event.rs`: Defines event handling logic.
- `src/editor.rs`: State and logic for the text editor component.
- `src/file_tree.rs`: State and logic for the file tree component.
- `src/lsp.rs`: Implementation of the LSP client for code intelligence features.

## Development Goals
The primary goal is to build a fully functional, terminal-based IDE. The immediate next steps are:
1.  Implement the integrated terminal.
2.  Implement the AI agent interaction panel.
3.  Fully integrate LSP features (handling responses from the server).
4.  Add mouse support.
5.  Add Git integration.
