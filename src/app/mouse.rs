use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::{App, DividerKind, PaneKind};
use crate::file_tree::FileEntryKind;

// Implementation block for mouse event handling in the App.
impl App {
    /// The main entry point for handling mouse events.
    ///
    /// This function is a router that dispatches mouse events to different handlers
    /// based on the event type (click, drag, scroll) and the location of the mouse.
    ///
    /// 注意：此函数中的鼠标事件调度逻辑（特别是嵌套的 `match` 和 `if` 检查）
    /// 可能会随着 UI 布局的复杂性增加而变得难以维护。未来可以考虑将其重构为
    /// 更模块化或基于组件的事件处理机制。
    pub async fn handle_mouse(&mut self, event: MouseEvent) {
        if self.overlay.is_some() {
            return;
        }

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => self.handle_mouse_down(event),
            MouseEventKind::Up(MouseButton::Left) => {
                self.layout.clear_drag();
                self.editor_drag_selecting = false;
            }
            MouseEventKind::Drag(MouseButton::Left) => self.handle_mouse_drag(event),
            MouseEventKind::ScrollUp => self.handle_mouse_scroll(event, -1),
            MouseEventKind::ScrollDown => self.handle_mouse_scroll(event, 1),
            MouseEventKind::Moved => self.handle_mouse_move(event),
            _ => {}
        }
        self.ensure_focus_available();
    }

    /// Handles left mouse button down events.
    fn handle_mouse_down(&mut self, event: MouseEvent) {
        let column = event.column;
        let row = event.row;

        // 1. Check for clicks on status bar controls.
        if let Some(control) = self.status_controls.hit_test(column, row)
            && self.handle_status_control_click(control) {
                return;
            }
        // 2. Check for clicks on pane dividers to start a drag.
        if let Some(divider) = self.layout.hit_test_divider(column, row) {
            self.layout.start_drag(divider, column, row);
            self.status_message = String::from("Dragging to resize layout");
            return;
        }
        // 3. Check for clicks on pane headers to toggle visibility.
        if let Some(pane) = self.layout.hit_test_header(column, row) {
            match pane {
                PaneKind::FileTree => self.layout.tree_visible = !self.layout.tree_visible,
                PaneKind::Agent => self.layout.agent_visible = !self.layout.agent_visible,
                PaneKind::Editor => self.layout.editor_visible = !self.layout.editor_visible,
                PaneKind::Terminal => self.layout.terminal_visible = !self.layout.terminal_visible,
            }
            self.ensure_focus_available();
            return;
        }

        // 4. Check for clicks within the body of a pane to focus and interact.
        if let Some(pane) = self.layout.hit_test_body(column, row) {
            match pane {
                PaneKind::FileTree => {
                    self.focus = super::FocusArea::FileTree;
                    self.mouse_select_file_tree(column, row);
                }
                PaneKind::Editor => {
                    self.focus = super::FocusArea::Editor;
                    let extend = event.modifiers.contains(KeyModifiers::SHIFT);
                    self.move_editor_cursor_with_mouse(column, row, extend);
                    self.editor_drag_selecting = extend;
                }
                PaneKind::Terminal => {
                    self.focus = super::FocusArea::Terminal;
                }
                PaneKind::Agent => {
                    self.focus = super::FocusArea::Agent;
                    self.mouse_select_agent(column, row);
                }
            }
        }
    }

    /// Handles mouse drag events.
    fn handle_mouse_drag(&mut self, event: MouseEvent) {
        if let Some(target) = self.layout.drag_state() {
            self.update_drag(target, event.column, event.row);
        } else if self.editor_drag_selecting {
            self.move_editor_cursor_with_mouse(event.column, event.row, true);
        }
    }

    /// Handles mouse scroll events.
    fn handle_mouse_scroll(&mut self, event: MouseEvent, delta: isize) {
        if let Some(pane) = self.layout.hit_test_body(event.column, event.row) {
            match pane {
                PaneKind::FileTree => {
                    self.file_tree.move_selection(delta);
                    self.update_file_tree_status();
                }
                PaneKind::Terminal => self.terminal.scroll(delta * 3),
                PaneKind::Agent => {
                    self.agent.move_selection(delta);
                    self.update_agent_status();
                }
                PaneKind::Editor => {}
            }
        }
    }

    /// Handles mouse move events for hover effects.
    fn handle_mouse_move(&mut self, event: MouseEvent) {
        // Handle hover effects for the menu bar.
        if self.menu_bar.open {
            if let Some(menu_idx) = self.menu_bar.layout.hit_item(event.column, event.row)
                && Some(menu_idx) != self.menu_bar.active_index {
                    self.menu_bar.open(menu_idx);
                    self.update_menu_hover_message();
                }
            if let Some(active) = self.menu_bar.active_index {
                let hover = self
                    .menu_bar
                    .layout
                    .hit_entry(active, event.column, event.row);
                self.menu_bar.highlighted_entry = hover;
                self.update_menu_hover_message();
            }
        }

        // Update the editor hover line.
        self.editor_hover_line = self.editor_line_at_visual_row(event.row);
    }

    /// Handles a click on an item in the file tree.
    fn mouse_select_file_tree(&mut self, column: u16, row: u16) {
        if let Some(geom) = self.layout.pane_geometry(PaneKind::FileTree) {
            if row <= geom.area.y || row >= geom.area.y.saturating_add(geom.area.height) - 1 {
                return;
            }
            if column < geom.area.x || column >= geom.area.x + geom.area.width {
                return;
            }
            let anchor = geom.area.y.saturating_add(1);
            let list_row = row.saturating_sub(anchor) as usize;
            self.file_tree.set_selection(list_row);
            self.update_file_tree_status();
        }
    }

    /// Handles a click on an item in the agent conversation.
    fn mouse_select_agent(&mut self, column: u16, row: u16) {
        if let Some(geom) = self.layout.pane_geometry(PaneKind::Agent) {
            if row <= geom.area.y || row >= geom.area.y.saturating_add(geom.area.height) - 1 {
                return;
            }
            if column < geom.area.x || column >= geom.area.x + geom.area.width {
                return;
            }
            let anchor = geom.area.y.saturating_add(1);
            let list_row = row.saturating_sub(anchor) as usize;
            self.agent.set_selection(list_row);
            self.update_agent_status();
        }
    }

    /// Updates the status message based on the file tree's current selection.
    fn update_file_tree_status(&mut self) {
        if let Some(entry) = self.file_tree.selected_entry() {
            self.status_message = match entry.kind {
                FileEntryKind::ParentLink => String::from("Select: ../"),
                FileEntryKind::WorkspaceRoot => String::from("Select: ./"),
                _ => format!("Select: {}", self.format_display_path(&entry.path)),
            };
        }
    }

    /// Updates the status message based on the agent panel's current selection.
    fn update_agent_status(&mut self) {
        if let Some(entry) = self.agent.selected_entry() {
            self.status_message = format!("Select Agent Item: {}", entry.title());
        }
    }

    /// Updates the layout based on a divider drag.
    fn update_drag(&mut self, target: DividerKind, column: u16, row: u16) {
        match target {
            DividerKind::TreeCenter => {
                if !self.layout.tree_visible {
                    return;
                }
                let workspace = self.layout.workspace;
                let total_width = workspace.width.max(1) as f32;
                let relative = column.saturating_sub(workspace.x) as f32 / total_width;
                let ratio = relative.clamp(0.1, 0.6);
                self.layout.tree_ratio = ratio;
                self.status_message = format!("File Tree Width: {:.0}%", ratio * 100.0);
            }
            DividerKind::CenterAgent => {
                if !self.layout.agent_visible {
                    return;
                }
                let workspace = self.layout.workspace;
                let total_width = workspace.width.max(1) as f32;
                let right_edge = workspace.x + workspace.width;
                let relative = right_edge.saturating_sub(column) as f32 / total_width;
                let ratio = relative.clamp(0.1, 0.5);
                self.layout.agent_ratio = ratio;
                self.status_message = format!("Agent Panel Width: {:.0}%", ratio * 100.0);
            }
            DividerKind::EditorTerminal => {
                if !(self.layout.editor_visible && self.layout.terminal_visible) {
                    return;
                }
                if let Some(center) = self.layout.center_area() {
                    let total_height = center.height.max(1) as f32;
                    let relative = row.saturating_sub(center.y) as f32 / total_height;
                    let ratio = relative.clamp(0.2, 0.9);
                    self.layout.editor_ratio = ratio;
                    self.status_message = format!("Editor Height: {:.0}%", ratio * 100.0);
                }
            }
        }
    }

    /// Calculates the inner drawable area of the editor pane.
    fn editor_inner_rect(&self) -> Option<Rect> {
        let geom = self.layout.pane_geometry(PaneKind::Editor)?;
        let area = geom.area;
        let width = area.width.saturating_sub(2);
        let height = area.height.saturating_sub(2);
        if width == 0 || height == 0 {
            return None;
        }
        Some(Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width,
            height,
        })
    }

    /// Calculates the metrics for the editor's text area.
    fn editor_text_metrics(&self) -> Option<(Rect, usize)> {
        let inner = self.editor_inner_rect()?;
        let text_width = inner
            .width
            .saturating_sub(super::EDITOR_GUTTER_WIDTH)
            .max(1) as usize;
        Some((inner, text_width))
    }

    /// Moves the editor cursor to the position corresponding to a mouse click.
    fn move_editor_cursor_with_mouse(&mut self, column: u16, row: u16, extend: bool) {
        let Some((inner, text_width)) = self.editor_text_metrics() else {
            return;
        };
        if row < inner.y || row >= inner.y + inner.height {
            return;
        }

        let visual_row = (row - inner.y) as usize;
        let text_start_x = inner.x.saturating_add(super::EDITOR_GUTTER_WIDTH);
        let visual_col = if column <= text_start_x {
            0
        } else if column >= inner.x.saturating_add(inner.width) {
            text_width
        } else {
            (column - text_start_x) as usize
        };

        let viewport_line = self.editor.viewport_start();
        let viewport_offset = self.editor.viewport_line_offset();
        self.editor.move_cursor_visual(
            viewport_line,
            viewport_offset,
            text_width,
            visual_row,
            visual_col,
            extend,
        );
        let (line, col) = self.editor.cursor();
        self.status_message = format!("Cursor: {}:{}", line + 1, col + 1);
    }

    /// Determines the logical line number in the editor at a given visual row on the screen.
    fn editor_line_at_visual_row(&self, row: u16) -> Option<usize> {
        let (inner, text_width) = self.editor_text_metrics()?;
        if row < inner.y || row >= inner.y + inner.height {
            return None;
        }
        let visual_row = (row - inner.y) as usize;
        let viewport_line = self.editor.viewport_start();
        let viewport_offset = self.editor.viewport_line_offset();
        self.editor
            .position_for_visual(viewport_line, viewport_offset, text_width, visual_row, 0)
            .map(|(line, _)| line)
            .or_else(|| {
                if self.editor.total_lines() == 0 {
                    Some(0)
                } else {
                    Some(self.editor.total_lines() - 1)
                }
            })
    }
}