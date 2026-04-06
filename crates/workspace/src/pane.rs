use gpui::*;
use gpui::prelude::*;

use crate::theme;

/// A single tab in a pane
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub icon: &'static str,
    pub view: AnyView,
    pub closable: bool,
}

/// Events emitted by Pane
pub enum PaneEvent {
    NewTabRequested,
}

/// A pane containing multiple tabs (center area)
pub struct Pane {
    tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: usize,
}

impl gpui::EventEmitter<PaneEvent> for Pane {}

impl Pane {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 0,
        }
    }

    pub fn add_tab(&mut self, title: String, icon: &'static str, view: AnyView, closable: bool) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab {
            id,
            title,
            icon,
            view,
            closable,
        });
        self.active_tab = self.tabs.len() - 1;
        id
    }

    pub fn close_tab(&mut self, tab_id: usize) {
        if self.tabs.len() <= 1 {
            return; // Always keep at least one terminal
        }
        if let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() {
                self.active_tab = self.tabs.len() - 1;
            }
        }
    }

    pub fn set_active_tab(&mut self, tab_id: usize) {
        if let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.active_tab = idx;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Vertical panel listing all terminal sessions (Cursor Glass style)
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let active_tab = self.active_tab;
        let count = self.tabs.len();

        div()
            .flex()
            .flex_col()
            .w(px(200.))
            .min_w(px(200.))
            .h_full()
            .flex_shrink_0()
            .bg(theme::mantle())
            .border_r_1()
            .border_color(theme::surface1())
            // Header: "N Terminals  +"
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(36.))
                    .min_h(px(36.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .border_b_1()
                    .border_color(theme::surface1())
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(theme::subtext())
                            .child(format!("{} Terminal{}", count, if count != 1 { "s" } else { "" })),
                    )
                    .child(
                        div()
                            .id("add-tab")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(24.))
                            .h(px(24.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_sm()
                            .text_color(theme::overlay())
                            .hover(|d| d.bg(theme::surface0()).text_color(theme::text()))
                            .child("+")
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(PaneEvent::NewTabRequested);
                            })),
                    ),
            )
            // Terminal entries
            .child(
                div()
                    .id("terminal-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .py(px(4.))
                    .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                        let tab_id = tab.id;
                        let is_active = idx == active_tab;
                        let closable = tab.closable && count > 1;

                        div()
                            .id(ElementId::Name(format!("tab-{}", tab_id).into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(32.))
                            .px(px(8.))
                            .mx(px(4.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .when(is_active, |d: Stateful<Div>| {
                                d.bg(theme::surface0())
                            })
                            .when(!is_active, |d: Stateful<Div>| {
                                d.hover(|d| d.bg(theme::surface0()))
                            })
                            // Terminal icon
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(theme::overlay())
                                    .mr(px(8.))
                                    .child(tab.icon),
                            )
                            // Title
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_xs()
                                    .text_color(if is_active { theme::text() } else { theme::subtext() })
                                    .child(tab.title.clone()),
                            )
                            // Close button (visible on hover via group)
                            .when(closable, |d: Stateful<Div>| {
                                d.child(
                                    div()
                                        .id(ElementId::Name(format!("close-{}", tab_id).into()))
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .w(px(20.))
                                        .h(px(20.))
                                        .rounded(px(4.))
                                        .text_xs()
                                        .text_color(theme::overlay())
                                        .cursor_pointer()
                                        .hover(|d| d.text_color(theme::text()).bg(theme::surface1()))
                                        .child("×")
                                        .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                            cx.stop_propagation();
                                        })
                                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                                            this.close_tab(tab_id);
                                            cx.notify();
                                        })),
                                )
                            })
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.set_active_tab(tab_id);
                                cx.notify();
                            }))
                    })),
            )
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_tabs = !self.tabs.is_empty();

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(theme::base())
            // Vertical sidebar (left)
            .when(has_tabs, |d: Div| {
                d.child(self.render_sidebar(cx))
            })
            // Active terminal content (right)
            .child(
                div()
                    .flex_1()
                    .size_full()
                    .overflow_hidden()
                    .when_some(self.tabs.get(self.active_tab), |d: Div, tab| {
                        d.child(tab.view.clone())
                    })
                    .when(!has_tabs, |d: Div| {
                        d.child(Self::render_welcome(cx))
                    }),
            )
    }
}

impl Pane {
    fn render_welcome(cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.))
            .child(
                div()
                    .w(px(64.))
                    .h(px(64.))
                    .rounded(px(16.))
                    .bg(theme::blue())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(theme::base())
                            .text_size(px(32.))
                            .font_weight(FontWeight::BOLD)
                            .child("F"),
                    ),
            )
            .child(
                div()
                    .text_color(theme::text())
                    .text_size(px(24.))
                    .font_weight(FontWeight::BOLD)
                    .child("Forge"),
            )
            .child(
                div()
                    .text_color(theme::overlay())
                    .text_sm()
                    .child("Multi-Agent IDE"),
            )
    }
}
