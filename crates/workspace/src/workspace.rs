use gpui::*;
use gpui::prelude::*;

use crate::dock::{Dock, DockPosition};
use crate::pane::Pane;
use crate::theme;

#[derive(Clone, Copy, PartialEq)]
enum DragSide {
    Left,
    Right,
}

pub struct IdeWorkspace {
    pub left_dock: Entity<Dock>,
    pub center_pane: Entity<Pane>,
    pub right_dock: Entity<Dock>,
    pub update_version: Option<String>,
    pub is_running: bool,
    pub is_pushing: bool,
    pub is_pulling: bool,
    dragging: Option<DragSide>,
    drag_start_x: f32,
    drag_start_width: f32,
}

pub enum WorkspaceEvent {
    UpdateClicked,
    SettingsClicked,
    RunClicked,
    PushClicked,
    PullClicked,
    LayoutChanged,
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
            is_running: false,
            is_pushing: false,
            is_pulling: false,
            dragging: None,
            drag_start_x: 0.,
            drag_start_width: 0.,
        }
    }

    fn start_drag(&mut self, side: DragSide, x: f32, cx: &mut Context<Self>) {
        let width = match side {
            DragSide::Left => self.left_dock.read(cx).width(),
            DragSide::Right => self.right_dock.read(cx).width(),
        };
        self.dragging = Some(side);
        self.drag_start_x = x;
        self.drag_start_width = width;
    }

    fn update_drag(&mut self, x: f32, cx: &mut Context<Self>) {
        let Some(side) = self.dragging else { return };
        let delta = x - self.drag_start_x;
        match side {
            DragSide::Left => {
                let new_width = self.drag_start_width + delta;
                self.left_dock.update(cx, |dock, _cx| dock.set_width(new_width));
            }
            DragSide::Right => {
                let new_width = self.drag_start_width - delta;
                self.right_dock.update(cx, |dock, _cx| dock.set_width(new_width));
            }
        }
        cx.notify();
    }

    fn stop_drag(&mut self, cx: &mut Context<Self>) {
        if self.dragging.is_some() {
            self.dragging = None;
            cx.emit(WorkspaceEvent::LayoutChanged);
        }
    }
}

impl Render for IdeWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_fullscreen = window.is_fullscreen();
        let is_dragging = self.dragging.is_some();
        let run_color = if self.is_running { theme::red() } else { theme::green() };
        let run_label = if self.is_running { "◼ Stop" } else { "▶ Run" };
        let push_label = if self.is_pushing { "Pushing..." } else { "↑ Push" };

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::mantle())
            .text_color(theme::text())
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Global mouse move/up for drag handling
            .when(is_dragging, |d| {
                d.cursor(CursorStyle::ResizeLeftRight)
            })
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                if this.dragging.is_some() {
                    let x: f32 = ev.position.x.into();
                    this.update_drag(x, cx);
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                this.stop_drag(cx);
            }))
            // Titlebar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(30.))
                    .pt(px(2.))
                    .when(is_fullscreen, |d| d.pb(px(1.)))
                    .flex_shrink_0()
                    // Left: update button
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .h_full()
                            .flex_1()
                            .when(!is_fullscreen, |d| d.pl(px(78.)))
                            .when(is_fullscreen, |d| d.pl(px(16.)))
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
                                        .text_sm()
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
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::blue())
                                    .child(format!("FORGE v{}", env!("CARGO_PKG_VERSION"))),
                            ),
                    )
                    // Right: run + push + settings
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_end()
                            .h_full()
                            .flex_1()
                            .pr(px(8.))
                            .gap(px(6.))
                            // Run / Stop
                            .child(
                                div()
                                    .id("run-btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(32.))
                                    .h(px(22.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_size(px(if self.is_running { 16. } else { 13. }))
                                    .text_color(run_color)
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child(if self.is_running { "◼" } else { "▶" })
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.emit(WorkspaceEvent::RunClicked);
                                    })),
                            )
                            // Push
                            .child(
                                div()
                                    .id("push-btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(32.))
                                    .h(px(22.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_color(theme::blue())
                                    .hover(|d| d.text_color(theme::lavender()).bg(theme::surface0()))
                                    .child(
                                        svg()
                                            .path("crates/app/assets/git-push.svg")
                                            .size(px(16.))
                                            .text_color(theme::blue())
                                    )
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.emit(WorkspaceEvent::PushClicked);
                                    })),
                            )
                            // Pull
                            .child(
                                div()
                                    .id("pull-btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(32.))
                                    .h(px(22.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_size(px(16.))
                                    .text_color(theme::blue())
                                    .hover(|d| d.text_color(theme::lavender()).bg(theme::surface0()))
                                    .child(div().font_family("MesloLGS NF").child("\u{e726}"))
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.emit(WorkspaceEvent::PullClicked);
                                    })),
                            )
                            // Settings gear
                            .child(
                                div()
                                    .id("settings-btn")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(32.))
                                    .h(px(22.))
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
                    // Left dock
                    .child(self.left_dock.clone())
                    // Left divider
                    .child(
                        div()
                            .id("left-divider")
                            .flex()
                            .justify_center()
                            .w(px(5.))
                            .h_full()
                            .flex_shrink_0()
                            .cursor(CursorStyle::ResizeLeftRight)
                            .hover(|d| d.bg(theme::blue()))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                let x: f32 = ev.position.x.into();
                                this.start_drag(DragSide::Left, x, cx);
                            }))
                            .child(div().w(px(1.)).h_full().bg(theme::surface1())),
                    )
                    // Center pane
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .h_full()
                            .overflow_hidden()
                            .child(self.center_pane.clone()),
                    )
                    // Right divider
                    .child(
                        div()
                            .id("right-divider")
                            .flex()
                            .justify_center()
                            .w(px(5.))
                            .h_full()
                            .flex_shrink_0()
                            .cursor(CursorStyle::ResizeLeftRight)
                            .hover(|d| d.bg(theme::blue()))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                let x: f32 = ev.position.x.into();
                                this.start_drag(DragSide::Right, x, cx);
                            }))
                            .child(div().w(px(1.)).h_full().bg(theme::surface1())),
                    )
                    // Right dock
                    .child(self.right_dock.clone()),
            )
    }
}
