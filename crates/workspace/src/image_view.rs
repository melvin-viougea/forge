use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::theme;

pub struct ImagePreviewView {
    path: PathBuf,
    zoom: f32,         // 1.0 = fit, >1 = zoom in, <1 = zoom out
    scroll_handle: ScrollHandle,
}

impl ImagePreviewView {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            zoom: 1.0,
            scroll_handle: ScrollHandle::new(),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom * 1.25).min(10.0);
        cx.notify();
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom / 1.25).max(0.1);
        cx.notify();
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.zoom = 1.0;
        cx.notify();
    }

    fn zoom_label(&self) -> String {
        format!("{}%", (self.zoom * 100.0).round() as u32)
    }
}

impl Render for ImagePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self.path.clone();
        let filename = self.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let zoom = self.zoom;
        let zoom_label = self.zoom_label();

        div()
            .id("image-preview")
            .size_full()
            .bg(theme::base())
            .flex()
            .flex_col()
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                let delta_y: f32 = match ev.delta {
                    ScrollDelta::Lines(d) => d.y,
                    ScrollDelta::Pixels(d) => d.y / px(40.0),
                };
                if delta_y > 0.0 {
                    this.zoom_out(cx);
                } else if delta_y < 0.0 {
                    this.zoom_in(cx);
                }
            }))
            .child(
                // Header: filename + zoom controls
                div()
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(12.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(theme::surface1())
                    // Filename
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme::subtext())
                            .child(filename)
                    )
                    // Zoom controls
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            // Zoom out
                            .child(
                                div()
                                    .id("zoom-out")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(24.))
                                    .h(px(24.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("\u{2212}") // minus
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_out(cx);
                                    }))
                            )
                            // Percentage (click to reset)
                            .child(
                                div()
                                    .id("zoom-reset")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .min_w(px(48.))
                                    .h(px(24.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_xs()
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child(zoom_label)
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_reset(cx);
                                    }))
                            )
                            // Zoom in
                            .child(
                                div()
                                    .id("zoom-in")
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(24.))
                                    .h(px(24.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("+")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_in(cx);
                                    }))
                            )
                    )
            )
            .child(
                // Image container (horizontal scroll only)
                div()
                    .id("image-scroll")
                    .flex_1()
                    .overflow_x_scroll()
                    .overflow_y_hidden()
                    .track_scroll(&self.scroll_handle)
                    .child(
                        // Image always fits vertically (with padding), scrolls horizontally when zoomed
                        div()
                            .h_full()
                            .min_w_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .p(px(16.))
                            .child(
                                img(path)
                                    .max_h_full()
                                    .w(relative(zoom))
                                    .object_fit(ObjectFit::Contain)
                            )
                    )
            )
    }
}
