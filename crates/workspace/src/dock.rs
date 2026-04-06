use gpui::*;
use gpui::prelude::*;

use crate::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DockPosition {
    Left,
    Right,
}

/// A side dock (left or right sidebar)
pub struct Dock {
    position: DockPosition,
    view: Option<AnyView>,
    width: f32,
}

impl Dock {
    pub fn new(position: DockPosition, width: f32) -> Self {
        Self {
            position,
            view: None,
            width,
        }
    }

    pub fn set_view(&mut self, view: AnyView) {
        self.view = Some(view);
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(150., 600.);
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let is_left = self.position == DockPosition::Left;

        div()
            .flex()
            .flex_col()
            .w(px(self.width))
            .min_w(px(self.width))
            .flex_shrink_0()
            .h_full()
            .bg(theme::mantle())
            .children(self.view.clone())
    }
}
