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
    SettingsClicked,
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_fullscreen = window.is_fullscreen();
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::base())
            .text_color(theme::text())
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Titlebar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(30.))
                    .pt(px(2.))
                    .flex_shrink_0()
                    .bg(theme::mantle())
                    // Left: update button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h_full()
                            .flex_1()
                            .when(!is_fullscreen, |d| d.pl(px(78.)))
                            .when(is_fullscreen, |d| d.pl(px(0.)))
                            .when_some(self.update_version.clone(), |d: Div, _version| {
                                d.child(
                                    div()
                                        .id("update-btn")
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.))
                                        .px(px(10.))
                                        .py(px(4.))
                                        .bg(theme::surface0())
                                        .rounded(px(4.))
                                        .cursor_pointer()
                                        .text_xs()
                                        .text_color(theme::blue())
                                        .hover(|d| d.bg(theme::surface1()))
                                        .child("Update Available")
                                        .on_click(cx.listener(|_this, _ev, _window, cx| {
                                            cx.emit(WorkspaceEvent::UpdateClicked);
                                        })),
                                )
                            }),
                    )
                    // Center: title
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .h_full()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::blue())
                                    .child("FORGE v0.9.9"),
                            ),
                    )
                    // Right: settings gear
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .h_full()
                            .flex_1()
                            .pr(px(8.))
                            .child(
                                div()
                                    .id("settings-btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(28.))
                                    .h(px(28.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_size(px(20.))
                                    .text_color(theme::overlay())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("⚙")
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.emit(WorkspaceEvent::SettingsClicked);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .h(px(1.))
                    .flex_shrink_0()
                    .bg(theme::surface1()),
            )
            // Main content
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
