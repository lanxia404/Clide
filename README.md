# Clide - A Modern Terminal IDE

Clide is a terminal-native IDE built with Rust, inspired by the principles and interface of modern editors like Visual Studio Code. It aims to provide a fast, efficient, and highly extensible development environment that lives entirely within your terminal, with a special focus on integration with AI agents.

## 🌟 Features

### Core Features
- [x] TUI Framework (built with `ratatui`)
- [x] Advanced File Tree Navigation (`../` global navigation, Enter to enter directory)
- [x] Text Editor with Syntax Highlighting
- [x] Mouse Support (Scroll wheel, Double-click to open/toggle)
- [x] Focus Management (Switch between panels)
- [x] Editor Enhancements (Line numbers, Current-line highlighting)
- [x] Dual Icon System (Unicode and Nerd Font support)
- [x] Internationalization (English, Simplified/Traditional Chinese)

### Planned Features
- [ ] **Full LSP Support:** Diagnostics, hover info, code completion.
- [ ] **Integrated Terminal:** A terminal panel directly within the IDE.
- [ ] **AI Agent Interaction:** A dedicated panel for interacting with AI agents.
- [ ] **Git Integration:** Manage your source control without leaving the editor.
- [ ] **Plugin System:** Extend Clide's functionality with custom plugins.

## 🖥️ Layout

Clide is designed with a familiar three-panel layout:

```
+----------------------------------------------------------------------+
| ☰ File  Edit  View  Go  Run  Terminal  Help                          |  <- Header
+----------------------+-----------------------------------------------+
|                      | 1 │ Welcome to Clide!                         |
|    > 📂 src          | 2 │                                           |
|      📄 main.rs      |   │                                           |  <- Editor
|      📄 ui.rs        |   │                                           |
|                      |                                               |
+----------------------+-----------------------------------------------+
| [No Name] | Ln 1, Col 1 | UTF-8 | Press 'l' to switch language       |  <- Footer
+----------------------------------------------------------------------+
```

## ⌨️ Keybindings

| Key         | Action                               |
|-------------|--------------------------------------|
| `Ctrl` + `Q`  | Quit the application                 |
| `Tab`       | Toggle focus between panels          |
| `Ctrl` + `L`  | Cycle through available languages    |
| `Enter`     | Open file or enter directory         |
| `Arrow Keys`| Navigate within the focused panel    |

## 🔧 Configuration

### Icons
Clide supports both standard Unicode icons and Nerd Font icons. To use Nerd Fonts, set the following environment variable before running:

```sh
export CLIDE_ICONS=nerd
```

## 🚀 Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) toolchain
- A Nerd Font (optional, for the best visual experience)

### Building and Running
1.  Clone the repository:
    ```sh
    git clone <repository-url>
    cd Clide
    ```
2.  Build the project:
    ```sh
    cargo build --release
    ```
3.  Run the IDE:
    ```sh
    cargo run --release
    ```

## 🤝 Contributing

Contributions are welcome! Please feel free to open an issue or submit a pull request.