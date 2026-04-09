use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

use crate::status::{get_changes, get_commits, ChangeStatus, GitCommit, GitFileChange};

use ide_workspace::theme as colors;

/// Events emitted by the runner button
pub enum RunnerEvent {
    Start(String),
    Stop,
    PushRequested,
}

impl gpui::EventEmitter<RunnerEvent> for CommitPanel {}

/// Action bar: Run/Stop + Push
pub struct CommitPanel {
    root_path: PathBuf,
    pub is_pushing: bool,
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

    pub fn toggle_runner(&mut self, cx: &mut Context<Self>) {
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
        let run_color = if self.is_running {
            colors::red()
        } else {
            colors::green()
        };
        let run_icon = if self.is_running { "◼" } else { "▶" };
        let push_icon = if self.is_pushing { "⏳" } else { "\u{e726}" };

        div()
            .flex()
            .flex_row()
            .items_center()
            .w_full()
            .h(px(34.))
            .min_h(px(34.))
            .flex_shrink_0()
            .px(px(8.))
            .gap(px(6.))
            // Run / Stop
            .child(
                div()
                    .id("run-btn")
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(px(24.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(run_color)
                    .bg(colors::surface0())
                    .hover(|d| d.bg(colors::surface1()))
                    .child(run_icon)
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        this.toggle_runner(cx);
                        cx.notify();
                    })),
            )
            // Push
            .child(
                div()
                    .id("push-btn")
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(px(24.))
                    .rounded(px(4.))
                    .cursor_pointer()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors::blue())
                    .bg(colors::surface0())
                    .hover(|d| d.bg(colors::surface1()))
                    .child(div().font_family("MesloLGS NF").child(push_icon))
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        if this.is_pushing {
                            return;
                        }
                        cx.emit(RunnerEvent::PushRequested);
                    })),
            )
    }
}

pub enum GitChangesEvent {
    FileOpened(PathBuf),
    FileOpenedDirect(PathBuf),
    FilePreviewOpened(PathBuf),
}

impl gpui::EventEmitter<GitChangesEvent> for GitChangesPanel {}

/// Git changes panel (bottom of right dock)
pub struct GitChangesPanel {
    root_path: PathBuf,
    changes: Vec<GitFileChange>,
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
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(2))
                    .await;
                let result = this.update(cx, |view, cx| {
                    let new_changes = get_changes(&view.root_path);
                    if new_changes.len() != view.changes.len() {
                        view.changes = new_changes;
                        cx.notify();
                    } else {
                        // Check if any paths changed
                        let changed = view.changes.iter().zip(new_changes.iter()).any(|(a, b)| {
                            a.path != b.path
                                || a.insertions != b.insertions
                                || a.deletions != b.deletions
                        });
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
        let context_menu = self
            .context_menu
            .as_ref()
            .map(|m| (m.position, m.target_idx));

        let all_entries: Vec<_> = self
            .changes
            .iter()
            .enumerate()
            .map(|(idx, change)| render_change_entry(idx, change, cx))
            .collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .id("git-changes-scroll")
            .overflow_y_scroll()
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .children(all_entries)
            // Context menu with backdrop
            .when_some(context_menu, |d: Stateful<Div>, (pos, target_idx)| {
                d.child(deferred(
                    div()
                        .id("ctx-backdrop")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                this.context_menu = None;
                                cx.notify();
                            }),
                        )
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                this.context_menu = None;
                                cx.notify();
                            }),
                        )
                        .child({
                            let has_preview = target_idx < self.changes.len() && {
                                let name = &self.changes[target_idx].path.to_string_lossy().to_string();
                                name.ends_with(".md")
                                    || name.ends_with(".png") || name.ends_with(".jpg") || name.ends_with(".jpeg")
                                    || name.ends_with(".gif") || name.ends_with(".bmp") || name.ends_with(".webp")
                                    || name.ends_with(".svg") || name.ends_with(".ico")
                            };

                            anchored().position(pos).child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w(px(160.))
                                    .bg(colors::surface0())
                                    .border_1()
                                    .border_color(colors::surface1())
                                    .rounded(px(6.))
                                    .py(px(4.))
                                    .text_sm()
                                    .shadow_lg()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        |_ev: &MouseDownEvent, _window, cx| {
                                            cx.stop_propagation();
                                        },
                                    )
                                    // Open Preview (only for md/images)
                                    .when(has_preview, |d| {
                                        d.child(
                                            div()
                                                .id("ctx-open-preview")
                                                .flex()
                                                .items_center()
                                                .w_full()
                                                .h(px(26.))
                                                .px(px(12.))
                                                .cursor_pointer()
                                                .text_color(colors::text())
                                                .hover(|d| d.bg(colors::surface1()))
                                                .child("Open Preview")
                                                .on_click(cx.listener(
                                                    move |this, _ev, _window, cx| {
                                                        this.context_menu = None;
                                                        if target_idx >= this.changes.len() { return; }
                                                        let abs_path = this
                                                            .root_path
                                                            .join(&this.changes[target_idx].path);
                                                        cx.emit(GitChangesEvent::FilePreviewOpened(abs_path));
                                                        cx.notify();
                                                    },
                                                )),
                                        )
                                    })
                                    // Open File
                                    .child(
                                        div()
                                            .id("ctx-open-file")
                                            .flex()
                                            .items_center()
                                            .w_full()
                                            .h(px(26.))
                                            .px(px(12.))
                                            .cursor_pointer()
                                            .text_color(colors::text())
                                            .hover(|d| d.bg(colors::surface1()))
                                            .child("Open File")
                                            .on_click(cx.listener(
                                                move |this, _ev, _window, cx| {
                                                    this.context_menu = None;
                                                    if target_idx >= this.changes.len() { return; }
                                                    let abs_path = this
                                                        .root_path
                                                        .join(&this.changes[target_idx].path);
                                                    cx.emit(GitChangesEvent::FileOpenedDirect(abs_path));
                                                    cx.notify();
                                                },
                                            )),
                                    )
                                    // Discard Changes
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
                                            .on_click(cx.listener(
                                                move |this, _ev, _window, cx| {
                                                    if target_idx >= this.changes.len() { return; }
                                                    this.discard_change(target_idx);
                                                    cx.notify();
                                                },
                                            )),
                                    ),
                            )
                        }),
                ))
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
    cx: &mut Context<GitChangesPanel>,
) -> Stateful<Div> {
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
    let _stats_str = format!("+{} -{}", insertions, deletions);

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
        .h(px(22.))
        .px(px(8.))
        .cursor_pointer()
        .hover(|d| d.bg(colors::surface0()))
        // Status icon
        .child(div().w(px(18.)).text_color(color).child(label))
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
                .text_xs()
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
            if idx >= this.changes.len() { return; }
            let abs_path = this.root_path.join(&this.changes[idx].path);
            cx.emit(GitChangesEvent::FileOpened(abs_path));
            cx.notify();
        }))
        .on_mouse_down(
            MouseButton::Right,
            cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                if idx >= this.changes.len() { return; }
                this.context_menu = Some(ContextMenuState {
                    position: ev.position,
                    target_idx: idx,
                });
                cx.notify();
            }),
        )
}

// ── Git Log Panel ───────────────────────────────────────────

pub enum GitLogEvent {
    CommitClicked { hash: String, message: String },
}

impl gpui::EventEmitter<GitLogEvent> for GitLogPanel {}

pub struct GitLogPanel {
    root_path: PathBuf,
    commits: Vec<GitCommit>,
    _scroll_px: f32,
    _scroll_acc: f32,
    pub visible_height: f32,
    _poll_task: Task<()>,
}

impl GitLogPanel {
    pub fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let commits = get_commits(&root_path, 50);

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(5))
                    .await;
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

        Self {
            root_path,
            commits,
            _scroll_px: 0.0,
            _scroll_acc: 0.0,
            visible_height: 200.0,
            _poll_task: poll_task,
        }
    }
}

impl Render for GitLogPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let commits: Vec<_> = self
            .commits
            .iter()
            .map(|c| (c.hash.clone(), c.message.clone(), c.time_ago.clone()))
            .collect();
        div()
            .flex()
            .flex_col()
            .size_full()
            .id("git-log-scroll")
            .overflow_y_scroll()
            .text_xs()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .children(
                commits
                    .into_iter()
                    .enumerate()
                    .map(|(idx, (hash, message, time_ago))| {
                        let h = hash.clone();
                        let m = message.clone();
                        render_commit_entry(idx, &hash, &message, &time_ago).on_click(cx.listener(
                            move |_this, _ev, _window, cx| {
                                cx.emit(GitLogEvent::CommitClicked {
                                    hash: h.clone(),
                                    message: m.clone(),
                                });
                            },
                        ))
                    }),
            )
    }
}

fn render_commit_entry(idx: usize, hash: &str, message: &str, time_ago: &str) -> Stateful<Div> {
    div()
        .id(ElementId::Name(format!("commit-{}", idx).into()))
        .flex()
        .flex_row()
        .items_center()
        .w_full()
        .h(px(22.))
        .px(px(8.))
        .cursor_pointer()
        .hover(|d| d.bg(colors::surface0()))
        // Hash
        .child(
            div()
                .flex_shrink_0()
                .w(px(60.))
                .text_color(colors::blue())
                .child(hash.to_string()),
        )
        // Message (truncated)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .truncate()
                .text_color(colors::text())
                .child(message.to_string()),
        )
        // Time ago
        .child(
            div()
                .flex_shrink_0()
                .ml(px(4.))
                .text_color(colors::overlay())
                .child(time_ago.to_string()),
        )
}
