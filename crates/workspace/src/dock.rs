use gpui::*;
use gpui::prelude::*;

use crate::theme;

#[derive(Clone, Copy, PartialEq)]
pub enum DockPosition {
    Left,
    Right,
}

/// A panel entry in a dock
pub struct DockPanel {
    pub title: String,
    pub icon: &'static str,
    pub view: AnyView,
}

/// A side dock (left or right sidebar)
pub struct Dock {
    position: DockPosition,
    panels: Vec<DockPanel>,
    active_panel: usize,
    visible: bool,
    width: f32,
}

impl Dock {
    pub fn new(position: DockPosition, width: f32) -> Self {
        Self {
            position,
            panels: Vec::new(),
            active_panel: 0,
            visible: true,
            width,
        }
    }

    pub fn add_panel(&mut self, title: String, icon: &'static str, view: AnyView) {
        self.panels.push(DockPanel { title, icon, view });
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn set_active_panel(&mut self, idx: usize) {
        if idx < self.panels.len() {
            self.active_panel = idx;
        }
    }
}

impl Render for Dock {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.visible || self.panels.is_empty() {
            return div().w(px(0.)).h_full();
        }

        let is_left = self.position == DockPosition::Left;
        let active = self.active_panel;

        div()
            .flex()
            .flex_col()
            .w(px(self.width))
            .min_w(px(self.width))
            .flex_shrink_0()
            .h_full()
            .bg(theme::mantle())
            .when(is_left, |d: Div| d.border_r_1().border_color(theme::surface1()))
            .when(!is_left, |d: Div| d.border_l_1().border_color(theme::surface1()))
            // Panel selector tabs (top)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .h(px(32.))
                    .bg(theme::mantle())
                    .border_b_1()
                    .border_color(theme::surface1())
                    .children(self.panels.iter().enumerate().map(|(idx, panel)| {
                        let is_active = idx == active;
                        div()
                            .id(ElementId::Name(format!("dock-tab-{}", idx).into()))
                            .flex()
                            .items_center()
                            .px(px(8.))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(if is_active { theme::text() } else { theme::subtext() })
                            .when(is_active, |d: Stateful<Div>| d.border_b_2().border_color(theme::blue()))
                            .hover(|d| d.bg(theme::surface0()))
                            .child(panel.icon)
                            .child(div().ml(px(4.)).child(panel.title.clone()))
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.set_active_panel(idx);
                                cx.notify();
                            }))
                    })),
            )
            // Active panel content
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .overflow_hidden()
                    .when_some(self.panels.get(self.active_panel), |d: Div, panel| {
                        d.child(panel.view.clone())
                    }),
            )
    }
}
