use super::{App, FocusArea, PaneKind};

// Implementation block for layout-related logic in the App.
impl App {
    /// Cycles the focus between the currently visible panes.
    ///
    /// # Arguments
    ///
    /// * `direction` - 1 to cycle forwards, -1 to cycle backwards.
    pub(crate) fn cycle_focus(&mut self, direction: isize) {
        let order = self.visible_focus_order();
        if order.is_empty() {
            return;
        }
        if let Some(current) = order.iter().position(|area| *area == self.focus) {
            let len = order.len() as isize;
            let mut next = current as isize + direction;
            // Wrap around the list.
            if next < 0 {
                next += len * (((-next) / len) + 1);
            }
            next %= len;
            self.focus = order[next as usize];
        } else {
            // If the current focus is not in the visible order, jump to the first one.
            self.focus = order[0];
        }
        self.status_message = format!("Focus changed to: {}", self.focus.label());
    }

    /// Determines the order of focusable panes based on their visibility.
    ///
    /// 注意：当前窗格的焦点顺序是硬编码的（文件树 -> 编辑器 -> 终端 -> 代理）。
    /// 如果未来需要支持自定义布局或用户定义的窗格顺序，此逻辑需要进行修改，
    /// 以便从配置中读取顺序或允许用户动态调整。
    pub(crate) fn visible_focus_order(&self) -> Vec<FocusArea> {
        let mut order = Vec::new();
        if self.layout.tree_visible {
            order.push(FocusArea::FileTree);
        }
        if self.layout.editor_visible {
            order.push(FocusArea::Editor);
        }
        if self.layout.terminal_visible {
            order.push(FocusArea::Terminal);
        }
        if self.layout.agent_visible {
            order.push(FocusArea::Agent);
        }
        // As a fallback, ensure the editor is always an option if nothing else is visible.
        if order.is_empty() {
            order.push(FocusArea::Editor);
        }
        order
    }

    /// Ensures that the currently focused pane is visible.
    /// If not, it moves the focus to the first available visible pane.
    pub(crate) fn ensure_focus_available(&mut self) {
        let order = self.visible_focus_order();
        if !order.contains(&self.focus)
            && let Some(first) = order.first() {
                self.focus = *first;
            }
    }

    /// Toggles the visibility of a specific pane.
    pub(crate) fn toggle_pane(&mut self, pane: PaneKind) {
        match pane {
            PaneKind::FileTree => {
                self.layout.tree_visible = !self.layout.tree_visible;
                let state = if self.layout.tree_visible { "shown" } else { "hidden" };
                self.status_message = format!("File Tree is now {}", state);
            }
            PaneKind::Agent => {
                self.layout.agent_visible = !self.layout.agent_visible;
                let state = if self.layout.agent_visible { "shown" } else { "hidden" };
                self.status_message = format!("Agent Panel is now {}", state);
            }
            PaneKind::Editor => self.toggle_central_pane(true),
            PaneKind::Terminal => self.toggle_central_pane(false),
        }
        // After toggling, ensure the focus is still on a visible pane.
        self.ensure_focus_available();
    }

    /// Toggles the visibility of a central pane (Editor or Terminal).
    fn toggle_central_pane(&mut self, is_editor: bool) {
        let (pane_visible, other_pane_visible, pane_name) = if is_editor {
            (
                &mut self.layout.editor_visible,
                self.layout.terminal_visible,
                "Editor",
            )
        } else {
            (
                &mut self.layout.terminal_visible,
                self.layout.editor_visible,
                "Terminal",
            )
        };

        // Prevent hiding both panes.
        if *pane_visible && !other_pane_visible {
            self.status_message =
                format!("Cannot hide the {} when the other pane is also hidden", pane_name);
            return;
        }

        *pane_visible = !*pane_visible;
        let state = if *pane_visible { "shown" } else { "hidden" };
        self.status_message = format!("{} is now {}", pane_name, state);
    }
}