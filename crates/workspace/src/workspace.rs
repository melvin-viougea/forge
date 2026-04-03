use gpui::*;
use gpui::prelude::*;

use crate::dock::{Dock, DockPosition};
use crate::pane::Pane;
use crate::theme;

pub struct IdeWorkspace {
    pub left_dock: Entity<Dock>,
    pub center_pane: Entity<Pane>,
    pub right_dock: Entity<Dock>,
    pub update_version: Option<String>,
}

pub enum WorkspaceEvent {
    UpdateClicked,
}

impl EventEmitter<WorkspaceEvent> for IdeWorkspace {}

impl IdeWorkspace {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let left_dock = cx.new(|_cx| Dock::new(DockPosition::Left, 200.));
        let center_pane = cx.new(|_cx| Pane::new());
        let right_dock = cx.new(|_cx| Dock::new(DockPosition::Right, 280.));

        Self {
            left_dock,
            center_pane,
            right_dock,
            update_version: None,
        }
    }
}

impl Render for IdeWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::base())
            .text_color(theme::text())
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Titlebar with update button (left) and app name (right)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(28.))
                    .flex_shrink_0()
                    .bg(theme::mantle())
                    // Left: traffic lights + FORGE title + update button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .pl(px(78.))
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::blue())
                                    .child("FORGE"),
                            )
                            .when_some(self.update_version.clone(), |d: Div, version| {
                                d.child(
                                    div()
                                        .id("update-btn")
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.))
                                        .px(px(8.))
                                        .py(px(2.))
                                        .bg(theme::surface0())
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::text())
                                        .hover(|d| d.bg(theme::surface1()))
                                        .child("↓ Restart to Update")
                                        .on_click(cx.listener(|_this, _ev, _window, cx| {
                                            cx.emit(WorkspaceEvent::UpdateClicked);
                                        })),
                                )
                            }),
                    )
                    // Center spacer
                    .child(div().flex_1())
                    // Right: app name + version
                    .child(
                        div()
                            .pr(px(12.))
                            .text_xs()
                            .text_color(theme::subtext())
                            .child("Forge v0.9"),
                    ),
            )
            // Main content row: left dock | center | right dock
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.left_dock.clone())
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .overflow_hidden()
                            .child(self.center_pane.clone()),
                    )
                    .child(self.right_dock.clone()),
            )
    }
}
