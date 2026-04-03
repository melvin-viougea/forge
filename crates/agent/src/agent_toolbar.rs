use gpui::*;
use gpui::prelude::*;

use crate::manager::AgentManager;
use crate::session::AgentStatus;

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn mantle() -> Rgba { rgb(0x181825) }
    pub fn surface0() -> Rgba { rgb(0x313244) }
    pub fn surface1() -> Rgba { rgb(0x45475a) }
    pub fn text() -> Rgba { rgb(0xcdd6f4) }
    pub fn subtext() -> Rgba { rgb(0xa6adc8) }
    pub fn blue() -> Rgba { rgb(0x89b4fa) }
    pub fn green() -> Rgba { rgb(0xa6e3a1) }
    pub fn red() -> Rgba { rgb(0xf38ba8) }
    pub fn overlay() -> Rgba { rgb(0x6c7086) }
    pub fn lavender() -> Rgba { rgb(0xb4befe) }
}

/// Agent toolbar view for managing Claude Code sessions
pub struct AgentToolbar {
    manager: AgentManager,
}

impl AgentToolbar {
    pub fn new(manager: AgentManager) -> Self {
        Self { manager }
    }

    pub fn manager(&self) -> &AgentManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut AgentManager {
        &mut self.manager
    }

    fn status_indicator(status: &AgentStatus) -> (&'static str, Rgba) {
        match status {
            AgentStatus::Starting => ("...", colors::overlay()),
            AgentStatus::Running => ("*", colors::green()),
            AgentStatus::Idle => ("o", colors::blue()),
            AgentStatus::Terminated => ("x", colors::red()),
            AgentStatus::Error(_) => ("!", colors::red()),
        }
    }
}

impl Render for AgentToolbar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sessions = self.manager.list_sessions();

        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(colors::mantle())
            .text_xs()
            // Header with "New Agent" button
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.))
                    .py(px(6.))
                    .child(
                        div()
                            .text_color(colors::subtext())
                            .child("AGENTS"),
                    )
                    .child(
                        div()
                            .id("new-agent")
                            .flex()
                            .items_center()
                            .justify_center()
                            .px(px(8.))
                            .py(px(2.))
                            .bg(colors::blue())
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(rgb(0x1e1e2e))
                            .font_weight(FontWeight::BOLD)
                            .hover(|d| d.bg(colors::lavender()))
                            .child("+ New")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                match this.manager.new_agent("You are a coding assistant. Help me with my project.") {
                                    Ok(_id) => {}
                                    Err(e) => {
                                        log::error!("Failed to spawn agent: {}", e);
                                    }
                                }
                                cx.notify();
                            })),
                    ),
            )
            // Agent list
            .children(sessions.into_iter().map(|(id, name, status)| {
                let (indicator, indicator_color) = Self::status_indicator(status);
                let id_owned = id.to_string();
                let id_for_kill = id_owned.clone();

                div()
                    .id(ElementId::Name(format!("agent-{}", id).into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(28.))
                    .px(px(8.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .hover(|d| d.bg(colors::surface0()))
                    // Status indicator
                    .child(
                        div()
                            .text_color(indicator_color)
                            .child(indicator),
                    )
                    // Agent name
                    .child(
                        div()
                            .flex_1()
                            .text_color(colors::text())
                            .child(name.to_string()),
                    )
                    // Kill button
                    .child(
                        div()
                            .id(ElementId::Name(format!("kill-{}", id).into()))
                            .text_color(colors::overlay())
                            .hover(|d| d.text_color(colors::red()))
                            .cursor_pointer()
                            .child("x")
                            .on_mouse_down(MouseButton::Left, |_ev, _window, cx| {
                                cx.stop_propagation();
                            })
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.manager.kill_agent(&id_for_kill);
                                cx.notify();
                            })),
                    )
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        this.manager.set_active(&id_owned);
                        cx.notify();
                    }))
            }))
            // Empty state
            .when(self.manager.agent_count() == 0, |d: Div| {
                d.child(
                    div()
                        .px(px(8.))
                        .py(px(12.))
                        .text_color(colors::overlay())
                        .text_center()
                        .child("No agents running.\nClick '+ New' to start one."),
                )
            })
    }
}
