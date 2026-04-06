use gpui::*;
use gpui::prelude::*;

use crate::theme;

/// A single tab in a pane
pub struct Tab {
    pub id: usize,
    pub title: String,
    pub icon: &'static str,
    pub detail: String,
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

    pub fn add_tab(
        &mut self,
        title: String,
        icon: &'static str,
        detail: String,
        view: AnyView,
        closable: bool,
    ) -> usize {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab {
            id,
            title,
            icon,
            detail,
            view,
            closable,
        });
        self.active_tab = self.tabs.len() - 1;
        id
    }

    pub fn close_tab(&mut self, tab_id: usize) {
        if self.tabs.len() <= 1 {
            return;
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

    pub fn set_tab_title(&mut self, tab_id: usize, title: String) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
            tab.title = title;
        }
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// CMUX-style floating card sidebar
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let active_tab = self.active_tab;
        let count = self.tabs.len();

        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .min_w(px(240.))
            .h_full()
            .flex_shrink_0()
            .bg(theme::mantle())
            .border_r_1()
            .border_color(theme::surface1())
            // New Terminal button
            .child(
                div()
                    .id("add-tab")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(32.))
                    .flex_shrink_0()
                    .mx(px(6.))
                    .mt(px(6.))
                    .mb(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(theme::subtext())
                    .bg(theme::surface0())
                    .hover(|d| d.bg(theme::surface1()).text_color(theme::text()))
                    .child("+ New Terminal")
                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                        cx.emit(PaneEvent::NewTabRequested);
                    })),
            )
            // Terminal cards
            .child(
                div()
                    .id("terminal-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .p(px(6.))
                    .gap(px(2.))
                    .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                        let tab_id = tab.id;
                        let is_active = idx == active_tab;
                        let closable = tab.closable && count > 1;

                        div()
                            .id(ElementId::Name(format!("tab-{}", tab_id).into()))
                            .flex()
                            .flex_col()
                            .w_full()
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(is_active, |d: Stateful<Div>| {
                                d.bg(theme::surface1())
                                    .border_l_2()
                                    .border_color(theme::blue())
                            })
                            .when(!is_active, |d: Stateful<Div>| {
                                d.hover(|d| d.bg(theme::surface0()))
                            })
                            // Title row with close button
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_xs()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_active { theme::text() } else { theme::subtext() })
                                            .child(tab.title.clone()),
                                    )
                                    .when(closable, |d: Div| {
                                        d.child(
                                            div()
                                                .id(ElementId::Name(format!("close-{}", tab_id).into()))
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .w(px(16.))
                                                .h(px(16.))
                                                .rounded(px(3.))
                                                .text_xs()
                                                .text_color(theme::overlay())
                                                .cursor_pointer()
                                                .hover(|d| d.text_color(theme::text()).bg(theme::surface0()))
                                                .child("×")
                                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                                    cx.stop_propagation();
                                                })
                                                .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                    this.close_tab(tab_id);
                                                    cx.notify();
                                                })),
                                        )
                                    }),
                            )
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
            .when(has_tabs, |d: Div| {
                d.child(self.render_sidebar(cx))
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
                        d.child(Self::render_welcome())
                    }),
            )
    }
}

impl Pane {
    fn render_welcome() -> Div {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .child(
                div()
                    .text_color(theme::blue())
                    .text_size(px(28.))
                    .font_weight(FontWeight::BOLD)
                    .child("FORGE"),
            )
            .child(
                div()
                    .text_color(theme::overlay())
                    .text_xs()
                    .child("Multi-Agent IDE"),
            )
    }
}
