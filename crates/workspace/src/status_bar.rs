use gpui::*;
use gpui::prelude::*;

use crate::theme;

pub struct StatusBar {
    pub branch_name: String,
    pub agent_count: usize,
    pub status_message: String,
    pub update_version: Option<String>,
    pub on_update_click: Option<Box<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            branch_name: "main".to_string(),
            agent_count: 0,
            status_message: String::new(),
            update_version: None,
            on_update_click: None,
        }
    }

    pub fn set_branch(&mut self, name: String) {
        self.branch_name = name;
    }

    pub fn set_agent_count(&mut self, count: usize) {
        self.agent_count = count;
    }

    pub fn set_status(&mut self, msg: String) {
        self.status_message = msg;
    }
}

impl Render for StatusBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(24.))
            .bg(theme::mantle())
            .border_t_1()
            .border_color(theme::surface1())
            .px(px(12.))
            .text_xs()
            .text_color(theme::subtext())
            // Left: update button OR branch name
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    // Update button (if available)
                    .when_some(self.update_version.clone(), |d: Div, version| {
                        d.child(
                            div()
                                .id("update-btn")
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(4.))
                                .px(px(6.))
                                .py(px(1.))
                                .bg(theme::surface0())
                                .rounded(px(4.))
                                .cursor_pointer()
                                .text_color(theme::text())
                                .hover(|d| d.bg(theme::surface1()))
                                .child("↓")
                                .child(format!("Restart to Update (v{})", version))
                                .on_click(cx.listener(|this, _ev, window, cx| {
                                    if let Some(cb) = &this.on_update_click {
                                        cb(window, cx);
                                    }
                                })),
                        )
                    })
                    // Branch name
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .child("⎇")
                            .child(self.branch_name.clone()),
                    ),
            )
            // Center: status message
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .child(self.status_message.clone()),
            )
            // Right: agent count + version
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(format!("Agents: {}", self.agent_count))
                    .child("Forge v0.5"),
            )
    }
}
