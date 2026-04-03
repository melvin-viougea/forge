use gpui::*;
use gpui::prelude::*;

use crate::theme;

pub struct StatusBar {
    pub branch_name: String,
    pub agent_count: usize,
    pub status_message: String,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            branch_name: "main".to_string(),
            agent_count: 0,
            status_message: String::new(),
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
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
            // Left: branch name
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(4.))
                    .child("⎇")
                    .child(self.branch_name.clone()),
            )
            // Center: status message
            .child(
                div()
                    .flex_1()
                    .text_center()
                    .child(self.status_message.clone()),
            )
            // Right: agent count
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .child(format!("Agents: {}", self.agent_count))
                    .child("Forge v0.2"),
            )
    }
}
