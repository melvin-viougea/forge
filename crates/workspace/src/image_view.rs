use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;
use std::rc::Rc;
use std::cell::Cell;

use crate::theme;

pub struct ImagePreviewView {
    path: PathBuf,
    zoom: f32,           // ratio of actual image pixels: 0.48 = 48%, 1.0 = 100%
    pan_x: f32,
    pan_y: f32,
    dragging: bool,
    last_mouse_pos: Option<Point<Pixels>>,
    image_dimensions: Option<(u32, u32)>,
    file_size: u64,
    container_bounds: Rc<Cell<(f32, f32, f32, f32)>>, // (origin_x, origin_y, width, height)
    needs_fit: bool,
}

impl ImagePreviewView {
    pub fn new(path: PathBuf) -> Self {
        let image_dimensions = image::image_dimensions(&path).ok();
        let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        Self {
            path,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            dragging: false,
            last_mouse_pos: None,
            image_dimensions,
            file_size,
            container_bounds: Rc::new(Cell::new((0.0, 0.0, 0.0, 0.0))),
            needs_fit: true,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    fn compute_fit_zoom(&self) -> f32 {
        let (_, _, cw, ch) = self.container_bounds.get();
        if let Some((iw, ih)) = self.image_dimensions {
            if cw > 0.0 && ch > 0.0 && iw > 0 && ih > 0 {
                let scale_w = cw / iw as f32;
                let scale_h = ch / ih as f32;
                return scale_w.min(scale_h).min(1.0); // never upscale beyond 100%
            }
        }
        1.0
    }

    fn zoom_in(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom * 1.25).min(10.0);
        self.needs_fit = false;
        cx.notify();
    }

    fn zoom_out(&mut self, cx: &mut Context<Self>) {
        self.zoom = (self.zoom / 1.25).max(0.01);
        self.needs_fit = false;
        cx.notify();
    }

    fn zoom_fit(&mut self, cx: &mut Context<Self>) {
        self.zoom = self.compute_fit_zoom();
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.needs_fit = false;
        cx.notify();
    }

    fn zoom_label(&self) -> String {
        format!("{}%", (self.zoom * 100.0).round() as u32)
    }

    fn dimensions_label(&self) -> String {
        match self.image_dimensions {
            Some((w, h)) => format!("{}×{}", w, h),
            None => String::new(),
        }
    }

    fn file_size_label(&self) -> String {
        if self.file_size == 0 {
            return String::new();
        }
        if self.file_size < 1024 {
            format!("{} B", self.file_size)
        } else if self.file_size < 1024 * 1024 {
            format!("{:.1} KB", self.file_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", self.file_size as f64 / (1024.0 * 1024.0))
        }
    }
}

impl Render for ImagePreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let path = self.path.clone();
        let filename = self.path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // If needs_fit and we have container bounds from previous frame, compute now
        if self.needs_fit {
            let (_, _, cw, ch) = self.container_bounds.get();
            if cw > 0.0 && ch > 0.0 {
                self.zoom = self.compute_fit_zoom();
                self.needs_fit = false;
            }
        }

        let zoom = self.zoom;
        let zoom_label = self.zoom_label();
        let dimensions_label = self.dimensions_label();
        let file_size_label = self.file_size_label();
        let pan_x = self.pan_x;
        let pan_y = self.pan_y;
        let image_dimensions = self.image_dimensions;

        // Compute image display width in pixels
        let display_width = image_dimensions.map(|(iw, _)| iw as f32 * zoom);

        // Clone Rc for canvas closure
        let container_bounds = self.container_bounds.clone();
        let needs_fit = self.needs_fit;
        let entity = cx.entity().clone();

        div()
            .id("image-preview")
            .size_full()
            .bg(theme::base())
            .flex()
            .flex_col()
            // Header bar
            .child(
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
                    // Left: filename + metadata
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(12.))
                            .child(div().text_sm().text_color(theme::text()).child(filename))
                            .when(!dimensions_label.is_empty(), |d| {
                                d.child(div().text_xs().text_color(theme::subtext()).child(dimensions_label))
                            })
                            .when(!file_size_label.is_empty(), |d| {
                                d.child(div().text_xs().text_color(theme::subtext()).child(file_size_label))
                            })
                    )
                    // Right: zoom controls
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .child(
                                div()
                                    .id("zoom-fit")
                                    .flex().items_center().justify_center()
                                    .h(px(24.)).px(px(8.)).rounded(px(4.))
                                    .cursor_pointer().text_xs()
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("Fit")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_fit(cx);
                                    }))
                            )
                            .child(div().w(px(1.)).h(px(16.)).bg(theme::surface1()))
                            .child(
                                div()
                                    .id("zoom-out")
                                    .flex().items_center().justify_center()
                                    .w(px(24.)).h(px(24.)).rounded(px(4.))
                                    .cursor_pointer().text_sm().font_weight(FontWeight::BOLD)
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("\u{2212}")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_out(cx);
                                    }))
                            )
                            .child(
                                div()
                                    .id("zoom-reset")
                                    .flex().items_center().justify_center()
                                    .min_w(px(48.)).h(px(24.)).rounded(px(4.))
                                    .cursor_pointer().text_xs().text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child(zoom_label)
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom = 1.0;
                                        this.pan_x = 0.0;
                                        this.pan_y = 0.0;
                                        this.needs_fit = false;
                                        cx.notify();
                                    }))
                            )
                            .child(
                                div()
                                    .id("zoom-in")
                                    .flex().items_center().justify_center()
                                    .w(px(24.)).h(px(24.)).rounded(px(4.))
                                    .cursor_pointer().text_sm().font_weight(FontWeight::BOLD)
                                    .text_color(theme::subtext())
                                    .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                    .child("+")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.zoom_in(cx);
                                    }))
                            )
                    )
            )
            // Image container
            .child(
                div()
                    .id("image-container")
                    .flex_1()
                    .overflow_hidden()
                    .cursor(if self.dragging { CursorStyle::ClosedHand } else { CursorStyle::OpenHand })
                    .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                        let delta_y: f32 = match ev.delta {
                            ScrollDelta::Lines(d) => d.y,
                            ScrollDelta::Pixels(d) => d.y / px(40.0),
                        };
                        let old_zoom = this.zoom;
                        if delta_y > 0.0 {
                            this.zoom = (this.zoom / 1.15).max(0.01);
                        } else if delta_y < 0.0 {
                            this.zoom = (this.zoom * 1.15).min(10.0);
                        }
                        // Zoom centered on cursor: adjust pan so the point under cursor stays fixed
                        if let Some((iw, ih)) = this.image_dimensions {
                            let (ox, oy, cw, ch) = this.container_bounds.get();
                            // Cursor in container-local coords
                            let mx = f32::from(ev.position.x) - ox;
                            let my = f32::from(ev.position.y) - oy;
                            // Image top-left before zoom
                            let old_img_x = (cw - iw as f32 * old_zoom) / 2.0 + this.pan_x;
                            let old_img_y = (ch - ih as f32 * old_zoom) / 2.0 + this.pan_y;
                            // Cursor position in image-pixel space
                            let rel_x = (mx - old_img_x) / old_zoom;
                            let rel_y = (my - old_img_y) / old_zoom;
                            // New image top-left so same image point stays under cursor
                            let new_img_x = mx - rel_x * this.zoom;
                            let new_img_y = my - rel_y * this.zoom;
                            this.pan_x = new_img_x - (cw - iw as f32 * this.zoom) / 2.0;
                            this.pan_y = new_img_y - (ch - ih as f32 * this.zoom) / 2.0;
                        }
                        this.needs_fit = false;
                        cx.notify();
                    }))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                        this.dragging = true;
                        this.last_mouse_pos = Some(ev.position);
                        cx.notify();
                    }))
                    .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                        this.dragging = false;
                        this.last_mouse_pos = None;
                        cx.notify();
                    }))
                    .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                        this.dragging = false;
                        this.last_mouse_pos = None;
                        cx.notify();
                    }))
                    .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                        if this.dragging {
                            if let Some(last) = this.last_mouse_pos {
                                let dx = f32::from(ev.position.x) - f32::from(last.x);
                                let dy = f32::from(ev.position.y) - f32::from(last.y);
                                this.pan_x += dx;
                                this.pan_y += dy;
                                cx.notify();
                            }
                            this.last_mouse_pos = Some(ev.position);
                        }
                    }))
                    // Checkerboard + size capture
                    .child(
                        canvas(
                            move |bounds, _window, cx| {
                                let new_bounds = (
                                    f32::from(bounds.origin.x),
                                    f32::from(bounds.origin.y),
                                    f32::from(bounds.size.width),
                                    f32::from(bounds.size.height),
                                );
                                let old_bounds = container_bounds.get();
                                container_bounds.set(new_bounds);
                                // If we need fit and just got a valid size, trigger re-render
                                if needs_fit && old_bounds.2 == 0.0 && new_bounds.2 > 0.0 {
                                    entity.update(cx, |_view, cx| cx.notify());
                                }
                            },
                            move |bounds, _, window, _cx| {
                                let tile = 24.0_f32;
                                let light = rgba(0x3a3a3aff);
                                let dark = rgba(0x2e2e2eff);
                                let cols = (f32::from(bounds.size.width) / tile).ceil() as usize + 1;
                                let rows = (f32::from(bounds.size.height) / tile).ceil() as usize + 1;
                                for row in 0..rows {
                                    for col in 0..cols {
                                        let color = if (row + col) % 2 == 0 { light } else { dark };
                                        let tile_bounds = Bounds {
                                            origin: Point {
                                                x: bounds.origin.x + px(col as f32 * tile),
                                                y: bounds.origin.y + px(row as f32 * tile),
                                            },
                                            size: Size {
                                                width: px(tile),
                                                height: px(tile),
                                            },
                                        };
                                        window.paint_quad(fill(tile_bounds, color));
                                    }
                                }
                            },
                        )
                        .size_full()
                        .absolute()
                        .top(px(0.))
                        .left(px(0.))
                    )
                    // Image layer — positioned absolutely, no flex centering
                    .child({
                        let (_, _, cw, ch) = self.container_bounds.get();
                        let (dw, dh) = if let Some((iw, ih)) = image_dimensions {
                            (iw as f32 * zoom, ih as f32 * zoom)
                        } else {
                            (cw, ch)
                        };
                        // Center image + apply pan offset
                        let img_x = (cw - dw) / 2.0 + pan_x;
                        let img_y = (ch - dh) / 2.0 + pan_y;

                        div()
                            .absolute()
                            .top(px(img_y))
                            .left(px(img_x))
                            .child({
                                let mut image = img(path)
                                    .object_fit(ObjectFit::Contain);
                                if let Some(w) = display_width {
                                    image = image.w(px(w));
                                } else {
                                    image = image.max_w_full().max_h_full();
                                }
                                image
                            })
                    })
            )
    }
}
