# Clide - A Modern Terminal IDE

Clide is a terminal-native IDE built with Rust, inspired by the principles and interface of modern editors like Visual Studio Code. It aims to provide a fast, efficient, and highly extensible development environment that lives entirely within your terminal, with a special focus on integration with AI agents.

## 🌟 Features

### Core Features
- [x] TUI Framework (built with `ratatui`)
- [x] File Tree Navigation
- [x] Text Editor with Syntax Highlighting (powered by `syntect`)
- [ ] Mouse Support

### Planned Features
- [ ] **Full LSP Support:** Code completion, diagnostics, go-to-definition, and more.
- [ ] **Integrated Terminal:** A terminal panel directly within the IDE.
- [ ] **AI Agent Interaction:** A dedicated panel for interacting with AI agents and visualizing their changes to the codebase in real-time.
- [ ] **Git Integration:** Manage your source control without leaving the editor.
- [ ] **Plugin System:** Extend Clide's functionality with custom plugins.

## 🖥️ Layout

Clide is designed with a familiar three-panel layout:

```
+----------------------+----------------------+----------------------+
|                      |                      |                      |
|      File Tree       |   Editor / Terminal  |      AI Agent        |
|      (Left)        |       (Center)       |       (Right)        |
|                      |                      |                      |
|                      |                      |                      |
+----------------------+----------------------+----------------------+
```

## 🚀 Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) toolchain

### Building and Running
1.  Clone the repository:
    ```sh
    git clone <repository-url>
    cd Clide
    ```
2.  Build the project:
    ```sh
    cargo build
    ```
3.  Run the IDE:
    ```sh
    cargo run
    ```

## 🤝 Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.
