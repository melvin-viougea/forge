use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::status::{get_changes, ChangeStatus, GitFileChange};

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn base() -> Rgba { rgb(0x1e1e2e) }
    pub fn mantle() -> Rgba { rgb(0x181825) }
    pub fn surface0() -> Rgba { rgb(0x313244) }
    pub fn surface1() -> Rgba { rgb(0x45475a) }
    pub fn text() -> Rgba { rgb(0xcdd6f4) }
    pub fn subtext() -> Rgba { rgb(0xa6adc8) }
    pub fn blue() -> Rgba { rgb(0x89b4fa) }
    pub fn green() -> Rgba { rgb(0xa6e3a1) }
    pub fn red() -> Rgba { rgb(0xf38ba8) }
    pub fn yellow() -> Rgba { rgb(0xf9e2af) }
    pub fn overlay() -> Rgba { rgb(0x6c7086) }
    pub fn lavender() -> Rgba { rgb(0xb4befe) }
}

/// Commit panel (top of right dock)
pub struct CommitPanel {
    root_path: PathBuf,
    commit_message: String,
    is_loading: bool,
    status_text: String,
}

impl CommitPanel {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            commit_message: String::new(),
            is_loading: false,
            status_text: String::new(),
        }
    }
}

impl Render for CommitPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .bg(colors::mantle())
            .p(px(8.))
            .gap(px(6.))
            // Title
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors::text())
                            .child("COMMIT"),
                    ),
            )
            // Commit message display
            .child(
                div()
                    .w_full()
                    .min_h(px(48.))
                    .bg(colors::surface0())
                    .rounded(px(4.))
                    .p(px(6.))
                    .text_xs()
                    .text_color(if self.commit_message.is_empty() {
                        colors::overlay()
                    } else {
                        colors::text()
                    })
                    .child(if self.commit_message.is_empty() {
                        "AI-generated message will appear here...".to_string()
                    } else {
                        self.commit_message.clone()
                    }),
            )
            // Action buttons
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap(px(4.))
                    .w_full()
                    // Generate AI message button
                    .child(
                        div()
                            .id("generate-msg")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .h(px(28.))
                            .bg(colors::surface0())
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(colors::lavender())
                            .hover(|d| d.bg(colors::surface1()))
                            .child(if self.is_loading {
                                "Generating..."
                            } else {
                                "AI Generate"
                            })
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                if this.is_loading {
                                    return;
                                }
                                this.is_loading = true;
                                this.status_text = "Generating...".to_string();
                                cx.notify();

                                let root = this.root_path.clone();
                                match crate::operations::generate_commit_message(&root) {
                                    Ok(msg) => {
                                        this.commit_message = msg;
                                        this.status_text = "Message generated".to_string();
                                    }
                                    Err(e) => {
                                        this.status_text = format!("Error: {}", e);
                                    }
                                }
                                this.is_loading = false;
                                cx.notify();
                            })),
                    )
                    // One-button commit + push
                    .child(
                        div()
                            .id("commit-push")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .h(px(28.))
                            .bg(colors::blue())
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgb(0x1e1e2e))
                            .font_weight(FontWeight::BOLD)
                            .hover(|d| d.bg(colors::lavender()))
                            .child("Commit & Push")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.is_loading = true;
                                this.status_text = "Committing...".to_string();
                                cx.notify();

                                let root = this.root_path.clone();
                                match crate::operations::one_button_commit_and_push(&root) {
                                    Ok(msg) => {
                                        this.status_text = msg;
                                        this.commit_message.clear();
                                    }
                                    Err(e) => {
                                        this.status_text = format!("Error: {}", e);
                                    }
                                }
                                this.is_loading = false;
                                cx.notify();
                            })),
                    ),
            )
            // Status text
            .when(!self.status_text.is_empty(), |d: Div| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(colors::subtext())
                        .child(self.status_text.clone()),
                )
            })
    }
}

/// Git changes panel (bottom of right dock)
pub struct GitChangesPanel {
    root_path: PathBuf,
    changes: Vec<GitFileChange>,
    selected_index: Option<usize>,
}

impl GitChangesPanel {
    pub fn new(root_path: PathBuf) -> Self {
        let changes = get_changes(&root_path);
        Self {
            root_path,
            changes,
            selected_index: None,
        }
    }

    pub fn refresh(&mut self) {
        self.changes = get_changes(&self.root_path);
    }

    fn status_color(status: &ChangeStatus) -> Rgba {
        match status {
            ChangeStatus::Modified => colors::yellow(),
            ChangeStatus::Added | ChangeStatus::Untracked => colors::green(),
            ChangeStatus::Deleted => colors::red(),
            ChangeStatus::Renamed => colors::blue(),
        }
    }
}

impl Render for GitChangesPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Pre-extract data from self.changes to avoid borrow conflicts with cx
        let selected_index = self.selected_index;
        let has_staged = self.changes.iter().any(|c| c.staged);
        let has_unstaged = self.changes.iter().any(|c| !c.staged);
        let change_count = self.changes.len();

        let staged_entries: Vec<_> = self.changes
            .iter()
            .enumerate()
            .filter(|(_, c)| c.staged)
            .map(|(idx, change)| {
                render_change_entry(idx, change, selected_index, cx)
            })
            .collect();

        let unstaged_entries: Vec<_> = self.changes
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.staged)
            .map(|(idx, change)| {
                render_change_entry(idx, change, selected_index, cx)
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .id("git-changes-scroll")
            .overflow_y_scroll()
            .text_xs()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.))
                    .py(px(6.))
                    .text_color(colors::subtext())
                    .child(format!("CHANGES ({})", change_count))
                    .child(
                        div()
                            .id("refresh-changes")
                            .cursor_pointer()
                            .hover(|d| d.text_color(colors::text()))
                            .child("R")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.refresh();
                                cx.notify();
                            })),
                    ),
            )
            // Staged section
            .when(has_staged, |d: Stateful<Div>| {
                d.child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .text_color(colors::subtext())
                        .child("Staged"),
                )
            })
            .children(staged_entries)
            // Unstaged section
            .when(has_unstaged, |d: Stateful<Div>| {
                d.child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .mt(px(4.))
                        .text_color(colors::subtext())
                        .child("Changes"),
                )
            })
            .children(unstaged_entries)
    }
}

fn render_change_entry(
    idx: usize,
    change: &GitFileChange,
    selected: Option<usize>,
    cx: &mut Context<GitChangesPanel>,
) -> Stateful<Div> {
    let is_selected = selected == Some(idx);
    let color = GitChangesPanel::status_color(&change.status);
    let label = change.status.label();
    let path = change
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir = change
        .path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    div()
        .id(ElementId::Name(format!("change-{}", idx).into()))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(22.))
        .px(px(12.))
        .cursor_pointer()
        .when(is_selected, |d: Stateful<Div>| d.bg(colors::surface0()))
        .hover(|d| d.bg(colors::surface0()))
        .child(
            div()
                .w(px(16.))
                .text_color(color)
                .child(label),
        )
        .child(
            div()
                .flex_1()
                .text_color(colors::text())
                .child(path),
        )
        .child(
            div()
                .text_color(colors::overlay())
                .child(dir),
        )
        .on_click(cx.listener(move |this, _ev, _window, cx| {
            this.selected_index = Some(idx);
            cx.notify();
        }))
}
