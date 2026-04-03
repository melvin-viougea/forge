use gpui::*;
use gpui::prelude::*;

use crate::dock::{Dock, DockPosition};
use crate::pane::Pane;
use crate::status_bar::StatusBar;
use crate::theme;

/// Main workspace: 3-panel layout
/// Left: Projects sidebar
/// Center: Tabbed panes (terminals, editors, previews)
/// Right: Git commit (top) + File tree / Git changes (bottom)
pub struct IdeWorkspace {
    pub left_dock: Entity<Dock>,
    pub center_pane: Entity<Pane>,
    pub right_dock: Entity<Dock>,
    pub status_bar: Entity<StatusBar>,
}

impl IdeWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let left_dock = cx.new(|_cx| Dock::new(DockPosition::Left, 200.));
        let center_pane = cx.new(|_cx| Pane::new());
        let right_dock = cx.new(|_cx| Dock::new(DockPosition::Right, 280.));
        let status_bar = cx.new(|_cx| StatusBar::new());

        Self {
            left_dock,
            center_pane,
            right_dock,
            status_bar,
        }
    }
}

impl Render for IdeWorkspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::base())
            .text_color(theme::text())
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Titlebar spacer for macOS transparent titlebar
            .pt(px(28.))
            // Main content row: left dock | center | right dock
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    // Left dock (Projects)
                    .child(self.left_dock.clone())
                    // Center pane (Tabs: terminals, files, MD preview)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .overflow_hidden()
                            .child(self.center_pane.clone()),
                    )
                    // Right dock (Git + Files)
                    .child(self.right_dock.clone()),
            )
            // Status bar
            .child(self.status_bar.clone())
    }
}
