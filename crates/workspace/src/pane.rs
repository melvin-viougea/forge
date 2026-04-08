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
    LayoutChanged,
}

#[derive(Clone)]
struct DraggedTab {
    tab_id: usize,
    title: String,
}

struct DragTabPreview {
    name: String,
}

impl Render for DragTabPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(8.))
            .w(px(160.))
            .rounded(px(6.))
            .bg(theme::surface0())
            .border_1()
            .border_color(theme::blue())
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(theme::text())
            .opacity(0.85)
            .child(self.name.clone())
    }
}

/// A pane containing multiple tabs (center area)
pub struct Pane {
    pub tabs: Vec<Tab>,
    active_tab: usize,
    next_tab_id: usize,
    sidebar_width: f32,
    dragging_sidebar: bool,
    drag_start_x: f32,
    drag_start_width: f32,
    drop_indicator: Option<usize>,
}

impl gpui::EventEmitter<PaneEvent> for Pane {}

impl Pane {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 0,
            sidebar_width: 240.,
            dragging_sidebar: false,
            drag_start_x: 0.,
            drag_start_width: 0.,
            drop_indicator: None,
        }
    }

    fn reorder_tab(&mut self, tab_id: usize, insert_pos: usize) {
        let Some(src) = self.tabs.iter().position(|t| t.id == tab_id) else { return };
        let active_id = self.tabs.get(self.active_tab).map(|t| t.id);
        let tab = self.tabs.remove(src);
        let adj = if src < insert_pos { insert_pos - 1 } else { insert_pos };
        let adj = adj.min(self.tabs.len());
        self.tabs.insert(adj, tab);
        // Restore active tab index
        if let Some(aid) = active_id {
            if let Some(new_idx) = self.tabs.iter().position(|t| t.id == aid) {
                self.active_tab = new_idx;
            }
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

    pub fn sidebar_width(&self) -> f32 {
        self.sidebar_width
    }

    pub fn set_sidebar_width(&mut self, width: f32) {
        self.sidebar_width = width.clamp(150., 400.);
    }

    /// CMUX-style floating card sidebar
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Div {
        let active_tab = self.active_tab;
        let count = self.tabs.len();
        let drop_indicator = self.drop_indicator;
        let tab_count = self.tabs.len();

        div()
            .flex()
            .flex_col()
            .w(px(self.sidebar_width))
            .min_w(px(self.sidebar_width))
            .h_full()
            .flex_shrink_0()
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
                    .text_sm()
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
                    .on_drop(cx.listener(|this, info: &DraggedTab, _window, cx| {
                        if let Some(insert_pos) = this.drop_indicator {
                            this.reorder_tab(info.tab_id, insert_pos);
                        }
                        this.drop_indicator = None;
                        cx.notify();
                    }))
                    .children(self.tabs.iter().enumerate().map(|(idx, tab)| {
                        let tab_id = tab.id;
                        let is_active = idx == active_tab;
                        let closable = tab.closable && count > 1;
                        let drag_title = tab.title.clone();
                        let show_above = drop_indicator == Some(idx);
                        let show_below = idx == tab_count - 1 && drop_indicator == Some(tab_count);

                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            // Gap zone above
                            .child(
                                div()
                                    .id(ElementId::Name(format!("tab-gap-{}", idx).into()))
                                    .w_full()
                                    .h(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_drag_move::<DraggedTab>(cx.listener(move |this, ev: &DragMoveEvent<DraggedTab>, _window, cx| {
                                        let mouse_y: f32 = ev.event.position.y.into();
                                        let oy: f32 = ev.bounds.origin.y.into();
                                        let h: f32 = ev.bounds.size.height.into();
                                        if mouse_y < oy || mouse_y >= oy + h { return; }
                                        if this.drop_indicator != Some(idx) {
                                            this.drop_indicator = Some(idx);
                                            cx.notify();
                                        }
                                    }))
                                    .when(show_above, |d: Stateful<Div>| {
                                        d.child(div().w_full().h(px(2.)).bg(theme::blue()).rounded(px(1.)))
                                    })
                            )
                            // Tab card
                            .child(
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
                                    .on_drag(DraggedTab { tab_id, title: drag_title }, move |info, _offset, _window, cx| {
                                        cx.new(|_cx| DragTabPreview { name: info.title.clone() })
                                    })
                                    .on_drag_move::<DraggedTab>(cx.listener(move |this, ev: &DragMoveEvent<DraggedTab>, _window, cx| {
                                        let mouse_y: f32 = ev.event.position.y.into();
                                        let oy: f32 = ev.bounds.origin.y.into();
                                        let h: f32 = ev.bounds.size.height.into();
                                        if mouse_y < oy || mouse_y >= oy + h { return; }
                                        let target = if mouse_y < oy + h / 2.0 {
                                            Some(idx)
                                        } else {
                                            Some(idx + 1)
                                        };
                                        if this.drop_indicator != target {
                                            this.drop_indicator = target;
                                            cx.notify();
                                        }
                                    }))
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
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(4.))
                                                    .overflow_hidden()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(if is_active { theme::text() } else { theme::subtext() })
                                                    .when(tab.icon.ends_with(".svg"), |d: Div| {
                                                        d.child(
                                                            svg()
                                                                .path(tab.icon)
                                                                .size(px(14.))
                                                                .flex_shrink_0()
                                                                .text_color(if is_active { theme::blue() } else { theme::subtext() })
                                                        )
                                                    })
                                                    .when(!tab.icon.ends_with(".svg"), |d: Div| {
                                                        d.child(
                                                            div().flex_shrink_0().child(tab.icon)
                                                        )
                                                    })
                                                    .child(div().truncate().child(tab.title.clone())),
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
                                                        .text_sm()
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
                            )
                            // Bottom gap zone (last tab only)
                            .when(idx == tab_count - 1, |d: Div| {
                                d.child(
                                    div()
                                        .id("tab-gap-bottom")
                                        .w_full()
                                        .h(px(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .on_drag_move::<DraggedTab>(cx.listener(move |this, ev: &DragMoveEvent<DraggedTab>, _window, cx| {
                                            let mouse_y: f32 = ev.event.position.y.into();
                                            let oy: f32 = ev.bounds.origin.y.into();
                                            let h: f32 = ev.bounds.size.height.into();
                                            if mouse_y < oy || mouse_y >= oy + h { return; }
                                            if this.drop_indicator != Some(tab_count) {
                                                this.drop_indicator = Some(tab_count);
                                                cx.notify();
                                            }
                                        }))
                                        .when(show_below, |d: Stateful<Div>| {
                                            d.child(div().w_full().h(px(2.)).bg(theme::blue()).rounded(px(1.)))
                                        })
                                )
                            })
                    })),
            )
    }
}

impl Render for Pane {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let has_tabs = !self.tabs.is_empty();
        let is_dragging = self.dragging_sidebar;

        div()
            .flex()
            .flex_row()
            .size_full()
            .when(is_dragging, |d| {
                d.cursor(CursorStyle::ResizeLeftRight)
            })
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                if this.dragging_sidebar {
                    let x: f32 = ev.position.x.into();
                    let delta = x - this.drag_start_x;
                    this.sidebar_width = (this.drag_start_width + delta).clamp(150., 400.);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                if this.dragging_sidebar {
                    this.dragging_sidebar = false;
                    cx.emit(PaneEvent::LayoutChanged);
                }
            }))
            // Sidebar + divider (single bg to avoid seam)
            .when(has_tabs, |d: Div| {
                d.child(
                    div()
                        .flex()
                        .flex_row()
                        .h_full()
                        .flex_shrink_0()
                        .child(self.render_sidebar(cx))
                        .child(
                            div()
                                .id("sidebar-divider")
                                .flex()
                                .justify_center()
                                .w(px(5.))
                                .h_full()
                                .flex_shrink_0()
                                .cursor(CursorStyle::ResizeLeftRight)
                                .hover(|d| d.bg(theme::blue()))
                                .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                    let x: f32 = ev.position.x.into();
                                    this.dragging_sidebar = true;
                                    this.drag_start_x = x;
                                    this.drag_start_width = this.sidebar_width;
                                    cx.notify();
                                }))
                                .child(div().w(px(1.)).h_full().bg(theme::surface1())),
                        ),
                )
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
                    .text_sm()
                    .child("Multi-Agent IDE"),
            )
    }
}