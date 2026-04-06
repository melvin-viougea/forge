use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::status::{get_changes, get_commits, ChangeStatus, GitCommit, GitFileChange};

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

/// Events emitted by the runner button
pub enum RunnerEvent {
    Start(String),
    Stop,
}

impl gpui::EventEmitter<RunnerEvent> for CommitPanel {}

/// Action bar: Run/Stop + Push
pub struct CommitPanel {
    root_path: PathBuf,
    is_pushing: bool,
    pub is_running: bool,
}

impl CommitPanel {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            is_pushing: false,
            is_running: false,
        }
    }

    fn toggle_runner(&mut self, cx: &mut Context<Self>) {
        if self.is_running {
            self.is_running = false;
            cx.emit(RunnerEvent::Stop);
        } else {
            let cmd = self.detect_run_command();
            self.is_running = true;
            cx.emit(RunnerEvent::Start(cmd));
        }
    }

    fn detect_run_command(&self) -> String {
        let root = &self.root_path;
        if root.join("Cargo.toml").exists() {
            "cargo run".to_string()
        } else if root.join("package.json").exists() {
            if root.join("bun.lock").exists() || root.join("bun.lockb").exists() {
                "bun run dev".to_string()
            } else {
                "npm run dev".to_string()
            }
        } else if root.join("Makefile").exists() {
            "make run".to_string()
        } else if root.join("main.py").exists() {
            "python3 main.py".to_string()
        } else if root.join("main.go").exists() {
            "go run .".to_string()
        } else {
            "echo 'No run command detected'".to_string()
        }
    }
}

impl Render for CommitPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .w_full()
            .bg(colors::mantle())
            .p(px(8.))
            .gap(px(6.))
            // Row: Run/Stop + Push
            .child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .gap(px(6.))
                    // Run / Stop button (50%)
                    .child(
                        div()
                            .id("run-btn")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .h(px(32.))
                            .bg(if self.is_running { colors::red() } else { colors::green() })
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_sm()
                            .text_color(rgb(0x1e1e2e))
                            .font_weight(FontWeight::BOLD)
                            .hover(|d| d.bg(colors::surface1()))
                            .child(if self.is_running { "■ Stop" } else { "▶ Run" })
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.toggle_runner(cx);
                                cx.notify();
                            })),
                    )
                    // Push button (50%)
                    .child(
                        div()
                            .id("push-btn")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .h(px(32.))
                            .bg(colors::blue())
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_sm()
                            .text_color(rgb(0x1e1e2e))
                            .font_weight(FontWeight::BOLD)
                            .hover(|d| d.bg(colors::lavender()))
                            .child(if self.is_pushing { "Pushing..." } else { "Push" })
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                if this.is_pushing {
                                    return;
                                }
                                this.is_pushing = true;
                                cx.notify();

                                let root = this.root_path.clone();
                                cx.spawn(async |this: WeakEntity<CommitPanel>, cx: &mut AsyncApp| {
                                    let result = cx.background_executor().spawn(async move {
                                        crate::operations::one_button_commit_and_push(&root)
                                    }).await;

                                    this.update(cx, |view, cx| {
                                        let _ = result;
                                        view.is_pushing = false;
                                        cx.notify();
                                    }).ok();
                                }).detach();
                            })),
                    ),
            )
    }
}

/// Git changes panel (bottom of right dock)
pub struct GitChangesPanel {
    root_path: PathBuf,
    changes: Vec<GitFileChange>,
    selected_index: Option<usize>,
    context_menu: Option<ContextMenuState>,
    _poll_task: Task<()>,
}

struct ContextMenuState {
    position: Point<Pixels>,
    target_idx: usize,
}

impl GitChangesPanel {
    pub fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let changes = get_changes(&root_path);

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                let result = this.update(cx, |view, cx| {
                    let new_changes = get_changes(&view.root_path);
                    if new_changes.len() != view.changes.len() {
                        view.changes = new_changes;
                        cx.notify();
                    } else {
                        // Check if any paths changed
                        let changed = view.changes.iter().zip(new_changes.iter())
                            .any(|(a, b)| a.path != b.path || a.insertions != b.insertions || a.deletions != b.deletions);
                        if changed {
                            view.changes = new_changes;
                            cx.notify();
                        }
                    }
                });
                if result.is_err() {
                    break;
                }
            }
        });

        Self {
            root_path,
            changes,
            selected_index: None,
            context_menu: None,
            _poll_task: poll_task,
        }
    }

    pub fn refresh(&mut self) {
        self.changes = get_changes(&self.root_path);
    }

    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    fn discard_change(&mut self, idx: usize) {
        if idx >= self.changes.len() {
            return;
        }
        let change = &self.changes[idx];
        let path = &change.path;

        match change.status {
            ChangeStatus::Untracked => {
                // Delete untracked file
                let abs = self.root_path.join(path);
                if abs.is_dir() {
                    let _ = std::fs::remove_dir_all(&abs);
                } else {
                    let _ = std::fs::remove_file(&abs);
                }
            }
            _ => {
                // git checkout -- <file>
                let _ = std::process::Command::new("git")
                    .args(["checkout", "--", &path.display().to_string()])
                    .current_dir(&self.root_path)
                    .output();
            }
        }

        self.context_menu = None;
        self.refresh();
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
        let selected_index = self.selected_index;
        let context_menu = self.context_menu.as_ref().map(|m| (m.position, m.target_idx));

        let all_entries: Vec<_> = self.changes
            .iter()
            .enumerate()
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
            .children(all_entries)
            // Context menu
            .when_some(context_menu, |d: Stateful<Div>, (pos, target_idx)| {
                d.child(
                    deferred(
                        anchored()
                            .position(pos)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w(px(160.))
                                    .bg(colors::surface0())
                                    .border_1()
                                    .border_color(colors::surface1())
                                    .rounded(px(6.))
                                    .py(px(4.))
                                    .text_xs()
                                    .shadow_lg()
                                    .child(
                                        div()
                                            .id("ctx-discard")
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .h(px(26.))
                                            .px(px(12.))
                                            .cursor_pointer()
                                            .text_color(colors::red())
                                            .hover(|d| d.bg(colors::surface1()))
                                            .child("Discard Changes")
                                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                this.discard_change(target_idx);
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    ),
                )
            })
    }
}

/// Shorten a path from the left, cutting anywhere (even mid-word):
/// "crates/file_explorer/src" with max 10 → "...orer/src"
fn shorten_path_left(path: &str, max_chars: usize) -> String {
    if path.chars().count() <= max_chars {
        return path.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let keep = max_chars - 3;
    let skip = path.chars().count() - keep;
    let truncated: String = path.chars().skip(skip).collect();
    format!("...{}", truncated)
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

    let filename = change
        .path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let dir = change
        .path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let insertions = change.insertions;
    let deletions = change.deletions;

    // Format stats string with fixed width for alignment
    let stats_str = format!("+{} -{}", insertions, deletions);

    // Budget for dir: panel is ~280px / ~7px per char ≈ 40 chars total
    // Subtract icon(2) + filename + space + stats(~10)
    let max_dir = 38_usize.saturating_sub(filename.chars().count());
    let dir_display = shorten_path_left(&dir, max_dir);

    div()
        .id(ElementId::Name(format!("change-{}", idx).into()))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(30.))
        .px(px(8.))
        .cursor_pointer()
        .when(is_selected, |d: Stateful<Div>| d.bg(colors::surface0()))
        .hover(|d| d.bg(colors::surface0()))
        // Status icon
        .child(
            div()
                .w(px(18.))
                .text_color(color)
                .child(label),
        )
        // Filename (never truncated)
        .child(
            div()
                .flex_shrink_0()
                .text_color(colors::text())
                .font_weight(FontWeight::MEDIUM)
                .child(format!("{} ", filename)),
        )
        // Directory path (left-truncated with ...)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .text_color(colors::overlay())
                .child(dir_display),
        )
        // +/- stats — always right-aligned
        .child(
            div()
                .flex_shrink_0()
                .ml(px(6.))
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(
                    div()
                        .text_color(colors::green())
                        .child(format!("+{}", insertions)),
                )
                .child(
                    div()
                        .text_color(colors::red())
                        .child(format!("-{}", deletions)),
                ),
        )
        .on_click(cx.listener(move |this, _ev, _window, cx| {
            this.context_menu = None;
            this.selected_index = Some(idx);
            cx.notify();
        }))
        .on_mouse_down(MouseButton::Right, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
            this.selected_index = Some(idx);
            this.context_menu = Some(ContextMenuState {
                position: ev.position,
                target_idx: idx,
            });
            cx.notify();
        }))
}

// ── Git Log Panel ───────────────────────────────────────────

pub struct GitLogPanel {
    root_path: PathBuf,
    commits: Vec<GitCommit>,
    _poll_task: Task<()>,
}

impl GitLogPanel {
    pub fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let commits = get_commits(&root_path, 50);

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(5)).await;
                let result = this.update(cx, |view, cx| {
                    let new_commits = get_commits(&view.root_path, 50);
                    // Only update if the top commit changed
                    let changed = match (view.commits.first(), new_commits.first()) {
                        (Some(a), Some(b)) => a.hash != b.hash,
                        (None, Some(_)) | (Some(_), None) => true,
                        (None, None) => false,
                    };
                    if changed {
                        view.commits = new_commits;
                        cx.notify();
                    }
                });
                if result.is_err() {
                    break;
                }
            }
        });

        Self { root_path, commits, _poll_task: poll_task }
    }
}

impl Render for GitLogPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .id("git-log-scroll")
            .overflow_y_scroll()
            .text_xs()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .children(
                self.commits.iter().enumerate().map(|(idx, commit)| {
                    render_commit_entry(idx, commit)
                }),
            )
    }
}

fn render_commit_entry(idx: usize, commit: &GitCommit) -> Stateful<Div> {
    div()
        .id(ElementId::Name(format!("commit-{}", idx).into()))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(28.))
        .px(px(8.))
        .hover(|d| d.bg(colors::surface0()))
        // Hash
        .child(
            div()
                .flex_shrink_0()
                .w(px(52.))
                .text_color(colors::blue())
                .child(commit.hash.clone()),
        )
        // Message (truncated)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .text_color(colors::text())
                .child(commit.message.clone()),
        )
        // Time ago
        .child(
            div()
                .flex_shrink_0()
                .ml(px(4.))
                .text_color(colors::overlay())
                .child(commit.time_ago.clone()),
        )
}
