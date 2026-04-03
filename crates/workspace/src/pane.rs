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
        if let Some(idx) = self.tabs.iter().position(|t| t.id == tab_id) {
            self.tabs.remove(idx);
            if self.active_tab >= self.tabs.len() && !self.tabs.is_empty() {
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

    fn render_tab_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active_tab = self.active_tab;

        div()
            .flex()
            .flex_row()
            .w_full()
            .h(px(36.))
            .bg(theme::mantle())
            .border_b_1()
            .border_color(theme::surface1())
            .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                let tab_id = tab.id;
                let is_active = idx == active_tab;
                let closable = tab.closable;

                div()
                    .id(ElementId::Name(format!("tab-{}", tab_id).into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .px(px(12.))
                    .py(px(6.))
                    .cursor_pointer()
                    .when(is_active, |d: Stateful<Div>| d.bg(theme::base()).border_b_2().border_color(theme::blue()))
                    .when(!is_active, |d: Stateful<Div>| d.hover(|d| d.bg(theme::surface0())))
                    .text_sm()
                    .text_color(if is_active { theme::text() } else { theme::subtext() })
                    .child(tab.icon)
                    .child(tab.title.clone())
                    .when(closable, |d: Stateful<Div>| {
                        d.child(
                            div()
                                .id(ElementId::Name(format!("close-tab-{}", tab_id).into()))
                                .ml(px(4.))
                                .text_xs()
                                .text_color(theme::overlay())
                                .hover(|d| d.text_color(theme::red()))
                                .cursor_pointer()
                                .child("x")
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
            }))
            .child(
                // "+" button to add new tab
                div()
                    .id("add-tab")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(32.))
                    .h_full()
                    .cursor_pointer()
                    .text_color(theme::overlay())
                    .hover(|d| d.text_color(theme::text()))
                    .child("+")
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        cx.emit(PaneEvent::NewTabRequested);
                    })),
            )
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_tabs = !self.tabs.is_empty();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(theme::base())
            // Tab bar only when tabs exist
            .when(has_tabs, |d: Div| {
                d.child(self.render_tab_bar(cx))
            })
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
            // Logo
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
            // Title
            .child(
                div()
                    .text_color(theme::text())
                    .text_size(px(24.))
                    .font_weight(FontWeight::BOLD)
                    .child("Forge"),
            )
            // Subtitle
            .child(
                div()
                    .text_color(theme::overlay())
                    .text_sm()
                    .child("Multi-Agent IDE"),
            )
    }
}
