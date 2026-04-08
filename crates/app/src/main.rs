mod updater;
mod session;
mod settings;

use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

use ide_file_explorer::{FileExplorerPanel, FileExplorerEvent};
use ide_git_panel::{CommitPanel, CommitDiffView, GitChangesPanel, GitChangesEvent, GitLogPanel, GitLogEvent, RunnerEvent};
use ide_workspace::theme::{self, ThemeName};
use ide_workspace::{FileView, FileViewEvent, ImagePreviewView, MarkdownPreviewView};
use ide_terminal::{LayoutDimensions, TerminalView, TerminalViewEvent};
use ide_workspace::{IdeWorkspace, Pane, PaneEvent, TabActivity, WorkspaceEvent};

// ── Project Panel (left sidebar) ─────────────────────────────

struct ProjectPanel {
    projects: Vec<ProjectEntry>,
    active_project: Option<usize>,
    order: Vec<usize>,
    drop_indicator: Option<usize>,
    /// Per-project agent activity counts: (idle, active, done)
    activity_counts: std::collections::HashMap<usize, (usize, usize, usize)>,
}

impl EventEmitter<ProjectPanelEvent> for ProjectPanel {}

enum ProjectPanelEvent {
    AddProjectRequested,
    ProjectSelected(usize, PathBuf),
    ProjectClosed(usize),
    ProjectReordered,
}

#[derive(Clone)]
struct DraggedProject {
    actual_idx: usize,
    name: String,
}

struct DragProjectPreview {
    name: String,
}

impl Render for DragProjectPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(8.))
            .w(px(160.))
            .rounded(px(6.))
            .bg(colors::surface0())
            .border_1()
            .border_color(colors::blue())
            .text_sm()
            .font_weight(FontWeight::BOLD)
            .text_color(colors::text())
            .opacity(0.85)
            .child(self.name.clone())
    }
}

struct ProjectEntry {
    name: String,
    path: PathBuf,
    display_path: String,
}

/// Shorten a path for display: replace $HOME with ~
fn shorten_path(path: &PathBuf) -> String {
    let s = path.display().to_string();
    if let Ok(home) = std::env::var("HOME") {
        if s.starts_with(&home) {
            return format!("~{}", &s[home.len()..]);
        }
    }
    s
}

use ide_workspace::theme as colors;

impl ProjectPanel {
    fn new() -> Self {
        Self {
            projects: Vec::new(),
            active_project: None,
            order: Vec::new(),
            drop_indicator: None,
            activity_counts: std::collections::HashMap::new(),
        }
    }
}

impl Render for ProjectPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active_project;

        div()
            .flex()
            .flex_col()
            .size_full()
            .text_sm()
            // New Workspace button
            .child(
                div()
                    .p(px(6.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("add-project")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_full()
                            .h(px(32.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(colors::subtext())
                            .bg(colors::surface0())
                            .hover(|d| d.bg(colors::surface1()).text_color(colors::text()))
                            .child("+ New Workspace")
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(ProjectPanelEvent::AddProjectRequested);
                            })),
                    ),
            )
            // CMUX-style floating cards
            .child({
                let drop_indicator = self.drop_indicator;
                let order_len = self.order.len();
                div()
                    .id("projects-list")
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p(px(6.))
                    // Container drop handler
                    .on_drop(cx.listener(|this, info: &DraggedProject, _window, cx| {
                        if let Some(insert_pos) = this.drop_indicator {
                            if let Some(src) = this.order.iter().position(|&i| i == info.actual_idx) {
                                let val = this.order.remove(src);
                                let adj = if src < insert_pos { insert_pos - 1 } else { insert_pos };
                                this.order.insert(adj, val);
                                cx.emit(ProjectPanelEvent::ProjectReordered);
                            }
                        }
                        this.drop_indicator = None;
                        cx.notify();
                    }))
                    .children(self.order.iter().enumerate().map(|(display_pos, &actual_idx)| {
                        let project = &self.projects[actual_idx];
                        let is_active = active == Some(actual_idx);
                        let path = project.path.clone();
                        let count = self.projects.len();
                        let drag_name = project.name.clone();
                        let project_name = project.name.clone();
                        let show_above = drop_indicator == Some(display_pos);
                        let show_below = display_pos == order_len - 1 && drop_indicator == Some(order_len);
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            // Gap zone above: hover here → insert ABOVE this card
                            .child(
                                div()
                                    .id(ElementId::Name(format!("gap-{}", display_pos).into()))
                                    .w_full()
                                    .h(px(6.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_drag_move::<DraggedProject>(cx.listener(move |this, ev: &DragMoveEvent<DraggedProject>, _window, cx| {
                                        let mouse_y: f32 = ev.event.position.y.into();
                                        let oy: f32 = ev.bounds.origin.y.into();
                                        let h: f32 = ev.bounds.size.height.into();
                                        if mouse_y < oy || mouse_y >= oy + h { return; }
                                        if this.drop_indicator != Some(display_pos) {
                                            this.drop_indicator = Some(display_pos);
                                            cx.notify();
                                        }
                                    }))
                                    .when(show_above, |d: Stateful<Div>| {
                                        d.child(div().w_full().h(px(2.)).bg(colors::blue()).rounded(px(1.)))
                                    })
                            )
                            // Card: hover here → insert BELOW this card
                            .child(
                                div()
                                    .id(ElementId::Name(format!("project-{}", actual_idx).into()))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .px(px(10.))
                                    .py(px(12.))
                                    .rounded(px(6.))
                                    .cursor_pointer()
                                    .when(is_active, |d: Stateful<Div>| {
                                        d.bg(colors::blue())
                                    })
                                    .when(!is_active, |d: Stateful<Div>| {
                                        d.hover(|d| d.bg(colors::surface0()))
                                    })
                                    .on_drag(DraggedProject { actual_idx, name: drag_name }, move |info, _offset, _window, cx| {
                                        cx.new(|_cx| DragProjectPreview { name: info.name.clone() })
                                    })
                                    .on_drag_move::<DraggedProject>(cx.listener(move |this, ev: &DragMoveEvent<DraggedProject>, _window, cx| {
                                        let mouse_y: f32 = ev.event.position.y.into();
                                        let oy: f32 = ev.bounds.origin.y.into();
                                        let h: f32 = ev.bounds.size.height.into();
                                        if mouse_y < oy || mouse_y >= oy + h { return; }
                                        let target = if mouse_y < oy + h / 2.0 {
                                            Some(display_pos)
                                        } else {
                                            Some(display_pos + 1)
                                        };
                                        if this.drop_indicator != target {
                                            this.drop_indicator = target;
                                            cx.notify();
                                        }
                                    }))
                                    .child({
                                        let counts = self.activity_counts.get(&actual_idx).copied().unwrap_or((0, 0, 0));
                                        let (idle, active, done) = counts;
                                        // Colors: white on active blue card, themed on inactive
                                        let dot_color = if is_active { gpui::rgb(0xffffffcc) } else { gpui::rgb(0x5b9bf5) };
                                        let text_color = if is_active { gpui::rgb(0xffffffaa) } else { colors::subtext() };
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .truncate()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .text_color(if is_active { gpui::rgb(0xffffff) } else { colors::text() })
                                                    .child(project_name),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .text_xs()
                                                    .text_color(text_color)
                                                    // Idle: circle outline + count
                                                    .child(
                                                        div().flex().flex_row().items_center().gap(px(3.))
                                                            .child(
                                                                div().w(px(6.)).h(px(6.)).rounded_full()
                                                                    .border_1().border_color(dot_color)
                                                            )
                                                            .child(format!("{}", idle))
                                                    )
                                                    // Active: thick ring + count
                                                    .child(
                                                        div().flex().flex_row().items_center().gap(px(3.))
                                                            .child(
                                                                div().w(px(6.)).h(px(6.)).rounded_full()
                                                                    .border_2().border_color(dot_color)
                                                            )
                                                            .child(format!("{}", active))
                                                    )
                                                    // Done: solid dot + count
                                                    .child(
                                                        div().flex().flex_row().items_center().gap(px(3.))
                                                            .child(
                                                                div().w(px(6.)).h(px(6.)).rounded_full()
                                                                    .bg(dot_color)
                                                            )
                                                            .child(format!("{}", done))
                                                    )
                                            )
                                    })
                                    .when(count > 1, |d: Stateful<Div>| {
                                        d.child(
                                            div()
                                                .id(ElementId::Name(format!("close-project-{}", actual_idx).into()))
                                                .flex_shrink_0()
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .w(px(16.))
                                                .h(px(16.))
                                                .rounded(px(3.))
                                                .text_sm()
                                                .text_color(if is_active { gpui::rgb(0xffffffaa) } else { colors::overlay() })
                                                .cursor_pointer()
                                                .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                                .child("×")
                                                .on_mouse_down(MouseButton::Left, move |_ev, _window, cx| {
                                                    cx.stop_propagation();
                                                })
                                                .on_click(cx.listener(move |_this, _ev, _window, cx| {
                                                    cx.emit(ProjectPanelEvent::ProjectClosed(actual_idx));
                                                })),
                                        )
                                    })
                                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                                        this.active_project = Some(actual_idx);
                                        cx.emit(ProjectPanelEvent::ProjectSelected(actual_idx, path.clone()));
                                        cx.notify();
                                    })),
                            )
                            // Bottom gap zone (last card only)
                            .when(display_pos == order_len - 1, |d: Div| {
                                d.child(
                                    div()
                                        .id("gap-bottom")
                                        .w_full()
                                        .h(px(6.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .on_drag_move::<DraggedProject>(cx.listener(move |this, ev: &DragMoveEvent<DraggedProject>, _window, cx| {
                                            let mouse_y: f32 = ev.event.position.y.into();
                                            let oy: f32 = ev.bounds.origin.y.into();
                                            let h: f32 = ev.bounds.size.height.into();
                                            if mouse_y < oy || mouse_y >= oy + h { return; }
                                            if this.drop_indicator != Some(order_len) {
                                                this.drop_indicator = Some(order_len);
                                                cx.notify();
                                            }
                                        }))
                                        .when(show_below, |d: Stateful<Div>| {
                                            d.child(div().w_full().h(px(2.)).bg(colors::blue()).rounded(px(1.)))
                                        })
                                )
                            })
                    }))
            })
    }
}

// ── Right Panel (git + files per project) ────────────────────

enum RightTab {
    Changes,
    Files,
}

struct RightPanel {
    pub commit_panel: Entity<CommitPanel>,
    pub file_explorer: Entity<FileExplorerPanel>,
    pub git_changes: Entity<GitChangesPanel>,
    pub git_log: Entity<GitLogPanel>,
    active_tab: RightTab,
    runner_terminal: Option<Entity<TerminalView>>,
    pub log_expanded: bool,
    pub log_height: f32,
    pub runner_expanded: bool,
    pub runner_height: f32,
    dragging_log: bool,
    dragging_runner: bool,
    drag_start_y: f32,
    drag_start_height: f32,
}

impl RightPanel {
    fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let commit_panel = cx.new(|_cx| CommitPanel::new(root_path.clone()));
        let file_explorer = cx.new(|cx| FileExplorerPanel::new(root_path.clone(), cx));
        let git_changes = cx.new(|cx| GitChangesPanel::new(root_path.clone(), cx));
        let git_log = cx.new(|cx| GitLogPanel::new(root_path, cx));

        Self {
            commit_panel,
            file_explorer,
            git_changes,
            git_log,
            active_tab: RightTab::Changes,
            runner_terminal: None,
            log_expanded: false,
            log_height: 250.,
            runner_expanded: true,
            runner_height: 200.,
            dragging_log: false,
            dragging_runner: false,
            drag_start_y: 0.,
            drag_start_height: 0.,
        }
    }

    fn set_runner(&mut self, terminal: Entity<TerminalView>) {
        self.runner_terminal = Some(terminal);
        self.runner_expanded = true;
    }

    fn clear_runner(&mut self) {
        self.runner_terminal = None;
        self.runner_expanded = false;
    }
}

pub enum RightPanelEvent {
    LayoutChanged,
}

impl EventEmitter<RightPanelEvent> for RightPanel {}

impl Render for RightPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let change_count = self.git_changes.read(cx).change_count();
        let has_runner = self.runner_terminal.is_some();
        let log_expanded = self.log_expanded;
        let runner_expanded = self.runner_expanded;

        let is_changes = matches!(self.active_tab, RightTab::Changes);
        let is_files = matches!(self.active_tab, RightTab::Files);

        let is_dragging = self.dragging_log || self.dragging_runner;
        let log_height = self.log_height;
        let runner_height = self.runner_height;

        div()
            .flex()
            .flex_col()
            .size_full()
            .when(is_dragging, |d| {
                d.cursor(CursorStyle::ResizeUpDown)
            })
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                if this.dragging_log {
                    let y: f32 = ev.position.y.into();
                    let delta = this.drag_start_y - y;
                    this.log_height = (this.drag_start_height + delta).clamp(100., 600.);
                    this.git_log.update(cx, |log, _| {
                        log.visible_height = this.log_height - 28.0;
                    });
                    cx.notify();
                } else if this.dragging_runner {
                    let y: f32 = ev.position.y.into();
                    let delta = this.drag_start_y - y;
                    this.runner_height = (this.drag_start_height + delta).clamp(100., 600.);
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                if this.dragging_log || this.dragging_runner {
                    this.dragging_log = false;
                    this.dragging_runner = false;
                    cx.emit(RightPanelEvent::LayoutChanged);
                }
            }))
            // ── Tabs ─────────────────────────────────────────
            .child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .h(px(30.))
                    .min_h(px(30.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(colors::surface1())
                    // Changes tab
                    .child(
                        div()
                            .id("tab-changes")
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_sm()
                            .when(is_changes, |d: Stateful<Div>| {
                                d.text_color(colors::text())
                                    .border_b_2()
                                    .border_color(colors::blue())
                            })
                            .when(!is_changes, |d: Stateful<Div>| {
                                d.text_color(colors::subtext())
                                    .hover(|d| d.text_color(colors::text()))
                            })
                            .child(format!("Changes ({})", change_count))
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.active_tab = RightTab::Changes;
                                cx.notify();
                            })),
                    )
                    // Files tab
                    .child(
                        div()
                            .id("tab-files")
                            .flex_1()
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_sm()
                            .when(is_files, |d: Stateful<Div>| {
                                d.text_color(colors::text())
                                    .border_b_2()
                                    .border_color(colors::blue())
                            })
                            .when(!is_files, |d: Stateful<Div>| {
                                d.text_color(colors::subtext())
                                    .hover(|d| d.text_color(colors::text()))
                            })
                            .child("Files")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.active_tab = RightTab::Files;
                                cx.notify();
                            })),
                    ),
            )
            // ── Content area ─────────────────────────────────
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .when(is_changes, |d: Div| d.child(self.git_changes.clone()))
                    .when(is_files, |d: Div| d.child(self.file_explorer.clone())),
            )
            // ── Runner section (above Git Log) ────────────────
            .when(has_runner, |d: Div| {
                d
                    // Runner divider (draggable)
                    .child(
                        div()
                            .id("runner-divider")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_full()
                            .h(px(5.))
                            .flex_shrink_0()
                            .when(runner_expanded, |d: Stateful<Div>| {
                                d.cursor(CursorStyle::ResizeUpDown)
                                    .hover(|d| d.bg(colors::blue()))
                                    .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                        let y: f32 = ev.position.y.into();
                                        this.dragging_runner = true;
                                        this.drag_start_y = y;
                                        this.drag_start_height = this.runner_height;
                                        cx.notify();
                                    }))
                            })
                            .child(div().w_full().h(px(1.)).bg(colors::surface1())),
                    )
                    // Runner header + content
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .when(runner_expanded, |d: Div| d.h(px(runner_height)).min_h(px(100.)))
                            .child(
                                div()
                                    .id("runner-header")
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .h(px(28.))
                                    .min_h(px(28.))
                                    .flex_shrink_0()
                                    .px(px(8.))
                                    .cursor_pointer()
                                    .hover(|d| d.bg(colors::surface0()))
                                    .text_sm()
                                    .text_color(colors::subtext())
                                    .child(if runner_expanded { "▼ " } else { "▲ " })
                                    .child("Runner")
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        this.runner_expanded = !this.runner_expanded;
                                        cx.emit(RightPanelEvent::LayoutChanged);
                                        cx.notify();
                                    })),
                            )
                            .when(runner_expanded, |d: Div| {
                                d.child(
                                    div()
                                        .flex_1()
                                        .w_full()
                                        .overflow_hidden()
                                        .when_some(
                                            self.runner_terminal.clone(),
                                            |d: Div, terminal| d.child(terminal),
                                        ),
                                )
                            }),
                    )
            })
            // ── Git Log divider (draggable) ────────────────
            .child(
                div()
                    .id("log-divider")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w_full()
                    .h(px(5.))
                    .flex_shrink_0()
                    .when(log_expanded, |d: Stateful<Div>| {
                        d.cursor(CursorStyle::ResizeUpDown)
                            .hover(|d| d.bg(colors::blue()))
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _window, cx| {
                                let y: f32 = ev.position.y.into();
                                this.dragging_log = true;
                                this.drag_start_y = y;
                                this.drag_start_height = this.log_height;
                                cx.notify();
                            }))
                    })
                    .child(div().w_full().h(px(1.)).bg(colors::surface1())),
            )
            // ── Git Log section (collapsible, bottom) ────────
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .when(log_expanded, |d: Div| d.h(px(log_height)).min_h(px(100.)))
                    .child(
                        div()
                            .id("log-header")
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(28.))
                            .min_h(px(28.))
                            .flex_shrink_0()
                            .px(px(8.))
                            .cursor_pointer()
                            .hover(|d| d.bg(colors::surface0()))
                            .text_sm()
                            .text_color(colors::subtext())
                            .child(if log_expanded { "▼ " } else { "▲ " })
                            .child("Git Log")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.log_expanded = !this.log_expanded;
                                if this.log_expanded {
                                    this.git_log.update(cx, |log, _| {
                                        log.visible_height = this.log_height - 28.0;
                                    });
                                }
                                cx.emit(RightPanelEvent::LayoutChanged);
                                cx.notify();
                            })),
                    )
                    .when(log_expanded, |d: Div| {
                        d.child(
                            div()
                                .flex_1()
                                .w_full()
                                .overflow_hidden()
                                .child(self.git_log.clone()),
                        )
                    }),
            )
    }
}

// ── Per-project state ────────────────────────────────────────

struct ProjectState {
    path: PathBuf,
    pane: Entity<Pane>,
    right_panel: Entity<RightPanel>,
    runner_terminal: Option<Entity<TerminalView>>,
    _pane_sub: Subscription,
    _runner_sub: Subscription,
    _right_panel_sub: Subscription,
    _file_explorer_sub: Subscription,
    _git_changes_sub: Subscription,
    _git_log_sub: Subscription,
}

// ── AppView ──────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ToastKind {
    Progress,
    Success,
    Error,
}

#[derive(Clone)]
struct Toast {
    id: usize,
    label: String,
    message: String,
    kind: ToastKind,
    percent: Option<u8>,
}

struct AppView {
    workspace: Entity<IdeWorkspace>,
    project_panel: Entity<ProjectPanel>,
    project_states: Vec<ProjectState>,
    active_project: Option<usize>,
    terminal_count: usize,
    update_info: Option<updater::UpdateInfo>,
    update_status: Option<(u8, String)>,
    settings_open: bool,
    wallpaper_path: Option<String>,
    wallpaper_opacity: f32,
    wallpaper_crop_x: f32,
    wallpaper_crop_y: f32,
    wallpaper_crop_zoom: f32,
    wallpaper_img_size: Option<(u32, u32)>,
    crop_picker_open: bool,
    crop_drag_start: Option<(f32, f32)>,
    crop_drag_initial: (f32, f32),
    crop_dragging: bool,
    crop_preview_bounds: std::rc::Rc<std::cell::Cell<(f32, f32, f32, f32)>>,
    toasts: Vec<Toast>,
    next_toast_id: usize,
    _project_subscription: Subscription,
    _workspace_subscription: Subscription,
    _update_task: Task<()>,
}

/// Read image dimensions from PNG/JPEG file headers
fn image_dimensions(path: &std::path::Path) -> Option<(u32, u32)> {
    let data = std::fs::read(path).ok()?;
    if data.len() < 24 { return None; }

    // PNG: magic 89 50 4E 47, IHDR at offset 16
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }

    // JPEG: magic FF D8, scan for SOF marker
    if data.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < data.len() {
            if data[i] != 0xFF { i += 1; continue; }
            let marker = data[i + 1];
            if (0xC0..=0xC2).contains(&marker) {
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return Some((w, h));
            }
            if marker == 0xD9 || marker == 0xDA { break; }
            let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
            i += 2 + len;
        }
    }

    None
}

/// Read EXIF orientation tag from a JPEG file (returns 1 if not found or not JPEG)
fn read_exif_orientation(path: &std::path::Path) -> u16 {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return 1,
    };
    if !data.starts_with(&[0xFF, 0xD8]) { return 1; }

    let mut i = 2;
    while i + 4 < data.len() {
        if data[i] != 0xFF { i += 1; continue; }
        let marker = data[i + 1];
        if i + 4 > data.len() { break; }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;

        if marker == 0xE1 && i + 10 < data.len() && &data[i + 4..i + 10] == b"Exif\0\0" {
            let tiff = &data[i + 10..];
            if tiff.len() < 8 { return 1; }
            let le = tiff[0..2] == *b"II";
            let r16 = |o: usize| -> u16 {
                if o + 2 > tiff.len() { return 0; }
                if le { u16::from_le_bytes([tiff[o], tiff[o + 1]]) }
                else { u16::from_be_bytes([tiff[o], tiff[o + 1]]) }
            };
            let ifd = if le {
                u32::from_le_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
            } else {
                u32::from_be_bytes([tiff[4], tiff[5], tiff[6], tiff[7]])
            } as usize;
            if ifd + 2 > tiff.len() { return 1; }
            let count = r16(ifd) as usize;
            for e in 0..count {
                let off = ifd + 2 + e * 12;
                if off + 12 > tiff.len() { break; }
                if r16(off) == 0x0112 { return r16(off + 8); }
            }
            return 1;
        }
        if marker == 0xDA { break; }
        i += 2 + len;
    }
    1
}

/// Fix JPEG orientation using macOS sips, returns path to corrected file (or original if no fix needed)
fn fix_image_orientation(path: &std::path::Path) -> PathBuf {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if ext != "jpg" && ext != "jpeg" {
        return path.to_path_buf();
    }

    let orientation = read_exif_orientation(path);
    if orientation <= 1 || orientation > 8 {
        return path.to_path_buf();
    }

    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return path.to_path_buf(),
    };
    let cache_dir = PathBuf::from(&home).join(".forge").join("wallpaper-cache");
    let _ = std::fs::create_dir_all(&cache_dir);

    let filename = path.file_name().unwrap_or_default();
    let cache_path = cache_dir.join(filename);
    if std::fs::copy(path, &cache_path).is_err() {
        return path.to_path_buf();
    }

    let cache_str = cache_path.to_string_lossy().to_string();

    // sips --rotate uses clockwise degrees
    match orientation {
        2 => { let _ = std::process::Command::new("sips").args(["--flip", "horizontal", &cache_str]).output(); }
        3 => { let _ = std::process::Command::new("sips").args(["--rotate", "180", &cache_str]).output(); }
        4 => { let _ = std::process::Command::new("sips").args(["--flip", "vertical", &cache_str]).output(); }
        5 => {
            let _ = std::process::Command::new("sips").args(["--rotate", "270", &cache_str]).output();
            let _ = std::process::Command::new("sips").args(["--flip", "horizontal", &cache_str]).output();
        }
        6 => { let _ = std::process::Command::new("sips").args(["--rotate", "90", &cache_str]).output(); }
        7 => {
            let _ = std::process::Command::new("sips").args(["--rotate", "90", &cache_str]).output();
            let _ = std::process::Command::new("sips").args(["--flip", "horizontal", &cache_str]).output();
        }
        8 => { let _ = std::process::Command::new("sips").args(["--rotate", "270", &cache_str]).output(); }
        _ => {}
    }

    // Reset EXIF orientation tag
    let _ = std::process::Command::new("sips")
        .args(["--setProperty", "orientation", "1", &cache_str])
        .output();

    cache_path
}

impl AppView {
    fn sync_activity_counts(&self, cx: &mut App) {
        let mut counts = std::collections::HashMap::new();
        for (idx, state) in self.project_states.iter().enumerate() {
            let pane = state.pane.read(cx);
            let mut idle = 0usize;
            let mut active = 0usize;
            let mut done = 0usize;
            for tab in &pane.tabs {
                if !tab.is_claude { continue; }
                match tab.activity {
                    TabActivity::Idle => idle += 1,
                    TabActivity::Active => active += 1,
                    TabActivity::Done => done += 1,
                }
            }
            counts.insert(idx, (idle, active, done));
        }
        self.project_panel.update(cx, |panel, cx| {
            panel.activity_counts = counts;
            cx.notify();
        });
    }

    fn save_session(&self, cx: &App) {
        // Save paths in display order
        let panel = self.project_panel.read(cx);
        let paths: Vec<PathBuf> = panel.order.iter()
            .map(|&i| self.project_states[i].path.clone())
            .collect();
        let active_actual = self.active_project.unwrap_or(0);
        // Translate active index to display position
        let active = panel.order.iter().position(|&i| i == active_actual).unwrap_or(0);

        let ws = self.workspace.read(cx);
        let left_w = ws.left_dock.read(cx).width();
        let right_w = ws.right_dock.read(cx).width();
        let sidebar_w = ws.center_pane.read(cx).sidebar_width();

        // Read log layout from the active project's right panel
        let (log_height, log_expanded) = self.active_project
            .and_then(|idx| self.project_states.get(idx))
            .map(|ps| {
                let rp = ps.right_panel.read(cx);
                (rp.log_height, rp.log_expanded)
            })
            .unwrap_or((250., false));

        let layout = session::SavedLayout {
            left_dock_width: left_w,
            right_dock_width: right_w,
            pane_sidebar_width: sidebar_w,
            log_height,
            log_expanded,
        };

        session::save(&paths, active, &layout);
    }
}

impl AppView {
    fn notify_all(&self, cx: &mut Context<Self>) {
        self.workspace.update(cx, |ws, cx| {
            ws.left_dock.update(cx, |_, cx| cx.notify());
            ws.right_dock.update(cx, |_, cx| cx.notify());
            ws.center_pane.update(cx, |_, cx| cx.notify());
            cx.notify();
        });
        self.project_panel.update(cx, |_, cx| cx.notify());
        for state in &self.project_states {
            state.pane.update(cx, |_, cx| cx.notify());
            state.right_panel.update(cx, |rp, cx| {
                rp.commit_panel.update(cx, |_, cx| cx.notify());
                rp.file_explorer.update(cx, |_, cx| cx.notify());
                rp.git_changes.update(cx, |_, cx| cx.notify());
                rp.git_log.update(cx, |_, cx| cx.notify());
                cx.notify();
            });
            if let Some(ref rt) = state.runner_terminal {
                rt.update(cx, |_, cx| cx.notify());
            }
        }
        cx.notify();
    }

    fn apply_theme(&mut self, name: ThemeName, cx: &mut Context<Self>) {
        theme::set_theme(name);
        self.save_settings();
        self.notify_all(cx);
    }

    fn apply_wallpaper(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        if let Some(p) = path {
            // Fix EXIF orientation (creates corrected copy if needed)
            let fixed = fix_image_orientation(std::path::Path::new(&p));
            let fixed_str = fixed.to_string_lossy().to_string();

            theme::set_wallpaper(Some(fixed_str.clone()));
            self.wallpaper_img_size = image_dimensions(&fixed);
            self.wallpaper_path = Some(fixed_str);
            self.crop_picker_open = true;
            self.wallpaper_crop_x = 0.5;
            self.wallpaper_crop_y = 0.5;
            self.wallpaper_crop_zoom = 1.0;
        } else {
            theme::set_wallpaper(None);
            self.wallpaper_img_size = None;
            self.wallpaper_path = None;
            self.crop_picker_open = false;
        }
        self.save_settings();
        self.notify_all(cx);
    }

    fn apply_wallpaper_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        self.wallpaper_opacity = opacity;
        theme::set_wallpaper_opacity(opacity);
        self.save_settings();
        self.notify_all(cx);
    }

    fn save_settings(&self) {
        settings::save(
            theme::current_name(),
            self.wallpaper_path.as_deref(),
            self.wallpaper_opacity,
            self.wallpaper_crop_x,
            self.wallpaper_crop_y,
            self.wallpaper_crop_zoom,
        );
    }

    fn add_toast(&mut self, label: &str, message: &str, kind: ToastKind, percent: Option<u8>, cx: &mut Context<Self>) -> usize {
        let id = self.next_toast_id;
        self.next_toast_id += 1;
        self.toasts.push(Toast {
            id,
            label: label.to_string(),
            message: message.to_string(),
            kind,
            percent,
        });
        cx.notify();
        id
    }

    fn update_toast(&mut self, id: usize, message: &str, kind: ToastKind, percent: Option<u8>, cx: &mut Context<Self>) {
        if let Some(toast) = self.toasts.iter_mut().find(|t| t.id == id) {
            toast.message = message.to_string();
            toast.kind = kind;
            toast.percent = percent;
            cx.notify();
        }
    }

    fn dismiss_toast(&mut self, id: usize, cx: &mut Context<Self>) {
        self.toasts.retain(|t| t.id != id);
        cx.notify();
    }

    fn open_file_in_pane(&mut self, pane: &Entity<Pane>, path: PathBuf, cx: &mut Context<Self>) {
        // Markdown files open in preview by default
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            self.open_markdown_preview(pane, path, cx);
            return;
        }

        // Image files open in preview (read-only)
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "svg" | "ico") {
                self.open_image_preview(pane, path, cx);
                return;
            }
        }

        self.open_file_editor(pane.clone(), path, cx);
    }

    fn open_file_editor(&mut self, pane: Entity<Pane>, path: PathBuf, cx: &mut Context<Self>) {
        // Check if file is already open in this pane — switch to it
        let existing = pane.read(cx).tabs.iter().position(|tab| tab.detail == path.to_string_lossy());
        if let Some(idx) = existing {
            let tab_id = pane.read(cx).tabs[idx].id;
            pane.update(cx, |p, cx| {
                p.set_active_tab(tab_id);
                cx.notify();
            });
            return;
        }

        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        let icon = "crates/app/assets/file.svg";

        let detail = path.to_string_lossy().to_string();
        let file_view = cx.new(|cx| FileView::new(path, cx));
        let tab_id = pane.update(cx, |p, _cx| {
            p.add_tab(filename, icon, detail, AnyView::from(file_view.clone()), true)
        });
        // Subscribe to title changes (modified indicator)
        let pane_entity = pane.clone();
        cx.subscribe(&file_view, move |_this: &mut AppView, _fv, event: &FileViewEvent, cx| {
            match event {
                FileViewEvent::TitleChanged(new_title) => {
                    pane_entity.update(cx, |pane, cx| {
                        pane.set_tab_title(tab_id, new_title.clone());
                        cx.notify();
                    });
                }
            }
        }).detach();
        cx.notify();
    }

    fn open_markdown_preview(&mut self, pane: &Entity<Pane>, path: PathBuf, cx: &mut Context<Self>) {
        let detail = format!("preview:{}", path.to_string_lossy());

        // Check if preview is already open for this file
        let existing = pane.read(cx).tabs.iter().position(|tab| tab.detail == detail);
        if let Some(idx) = existing {
            let tab_id = pane.read(cx).tabs[idx].id;
            pane.update(cx, |p, cx| {
                p.set_active_tab(tab_id);
                cx.notify();
            });
            return;
        }

        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "preview".to_string());

        let title = format!("Preview: {}", filename);
        let icon = "crates/app/assets/markdown.svg";
        let md_view = cx.new(|_cx| MarkdownPreviewView::new(path));

        pane.update(cx, |p, _cx| {
            p.add_tab(title, icon, detail, AnyView::from(md_view), true);
        });
        cx.notify();
    }

    fn open_image_preview(&mut self, pane: &Entity<Pane>, path: PathBuf, cx: &mut Context<Self>) {
        let detail = format!("image:{}", path.to_string_lossy());

        let existing = pane.read(cx).tabs.iter().position(|tab| tab.detail == detail);
        if let Some(idx) = existing {
            let tab_id = pane.read(cx).tabs[idx].id;
            pane.update(cx, |p, cx| {
                p.set_active_tab(tab_id);
                cx.notify();
            });
            return;
        }

        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "image".to_string());

        let icon = "crates/app/assets/image.svg";
        let img_view = cx.new(|_cx| ImagePreviewView::new(path));

        pane.update(cx, |p, _cx| {
            p.add_tab(filename, icon, detail, AnyView::from(img_view), true);
        });
        cx.notify();
    }

    fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.add_project_with_cmd(path, Some("ccc"), cx);
    }

    fn add_project_with_cmd(&mut self, path: PathBuf, auto_cmd: Option<&str>, cx: &mut Context<Self>) {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".to_string());

        // Create a Pane for this project
        let pane = cx.new(|_cx| Pane::new());

        // Create the right panel for this project
        let right_panel = cx.new(|cx| RightPanel::new(path.clone(), cx));

        // Subscribe to pane "+" events
        let workspace = self.workspace.clone();
        let pane_sub = cx.subscribe(&pane, move |this: &mut AppView, _pane, event: &PaneEvent, cx| {
            match event {
                PaneEvent::NewTabRequested => {
                    this.terminal_count += 1;
                    let title = format!("zsh {}", this.terminal_count);
                    if let Some(idx) = this.active_project {
                        let project_path = this.project_states[idx].path.clone();
                        let detail = shorten_path(&project_path);
                        let terminal_view = cx.new(|cx| TerminalView::new_in(title.clone(), Some(project_path), cx));
                        let tab_id = this.project_states[idx].pane.update(cx, |pane, _cx| {
                            pane.add_tab(title, "crates/app/assets/terminal.svg", detail, AnyView::from(terminal_view.clone()), true)
                        });
                        // Subscribe to OSC title changes
                        let pane_entity = this.project_states[idx].pane.clone();
                        cx.subscribe(&terminal_view, move |_this: &mut AppView, _tv, event: &TerminalViewEvent, cx| {
                            match event {
                                TerminalViewEvent::TitleChanged(new_title) => {
                                    pane_entity.update(cx, |pane, cx| {
                                        pane.set_tab_title(tab_id, new_title.clone());
                                        if new_title == "Claude" {
                                            pane.set_tab_icon(tab_id, "crates/app/assets/claude.svg");
                                            pane.set_tab_claude(tab_id, true);
                                        }
                                        cx.notify();
                                    });
                                }
                                TerminalViewEvent::Bell => {
                                    pane_entity.update(cx, |pane, cx| {
                                        if pane.tab_activity(tab_id) == Some(TabActivity::Active) {
                                            pane.set_tab_activity(tab_id, TabActivity::Done);
                                            cx.notify();
                                        }
                                    });
                                }
                                TerminalViewEvent::ActivityStarted => {
                                    pane_entity.update(cx, |pane, cx| {
                                        pane.set_tab_activity(tab_id, TabActivity::Active);
                                        cx.notify();
                                    });
                                }
                                TerminalViewEvent::UserInput => {
                                    pane_entity.update(cx, |pane, cx| {
                                        pane.set_tab_activity(tab_id, TabActivity::Idle);
                                        cx.notify();
                                    });
                                }
                            }
                        }).detach();
                    }
                    cx.notify();
                }
                PaneEvent::LayoutChanged => {
                    this.save_session(cx);
                }
            }
        });

        // Open a terminal in this project directory
        self.terminal_count += 1;
        let project_path = path.clone();
        let detail = shorten_path(&project_path);
        let is_claude = auto_cmd.is_some();
        let term_title = if is_claude { "Claude".to_string() } else { format!("zsh {}", self.terminal_count) };
        let term_icon = if is_claude { "crates/app/assets/claude.svg" } else { "crates/app/assets/terminal.svg" };
        let terminal_view = cx.new(|cx| TerminalView::new_in(term_title.clone(), Some(project_path), cx));
        let tab_id = pane.update(cx, |p, _cx| {
            let id = p.add_tab(term_title, term_icon, detail, AnyView::from(terminal_view.clone()), true);
            if is_claude { p.set_tab_claude(id, true); }
            id
        });
        // Subscribe to OSC title changes
        let pane_for_title = pane.clone();
        cx.subscribe(&terminal_view, move |_this: &mut AppView, _tv, event: &TerminalViewEvent, cx| {
            match event {
                TerminalViewEvent::TitleChanged(new_title) => {
                    pane_for_title.update(cx, |pane, cx| {
                        pane.set_tab_title(tab_id, new_title.clone());
                        if new_title == "Claude" {
                            pane.set_tab_icon(tab_id, "crates/app/assets/claude.svg");
                            pane.set_tab_claude(tab_id, true);
                        }
                        cx.notify();
                    });
                }
                TerminalViewEvent::Bell => {
                    pane_for_title.update(cx, |pane, cx| {
                        if pane.tab_activity(tab_id) == Some(TabActivity::Active) {
                            pane.set_tab_activity(tab_id, TabActivity::Done);
                            cx.notify();
                        }
                    });
                }
                TerminalViewEvent::ActivityStarted => {
                    pane_for_title.update(cx, |pane, cx| {
                        pane.set_tab_activity(tab_id, TabActivity::Active);
                        cx.notify();
                    });
                }
                TerminalViewEvent::UserInput => {
                    pane_for_title.update(cx, |pane, cx| {
                        pane.set_tab_activity(tab_id, TabActivity::Idle);
                        cx.notify();
                    });
                }
            }
        }).detach();

        // Auto-run command in the first terminal (e.g. "ccc" for session restore)
        if let Some(cmd) = auto_cmd {
            let cmd = cmd.to_string();
            let tv = terminal_view.clone();
            cx.spawn(async move |_this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                cx.background_executor().timer(Duration::from_millis(500)).await;
                tv.update(cx, |view, _cx| {
                    view.terminal.write_input(cmd.as_bytes());
                    view.terminal.write_input(b"\r");
                }).ok();
            }).detach();
        }

        // Subscribe to runner events from this project's commit panel
        let project_idx = self.project_states.len();
        let commit_panel_entity = right_panel.read(cx).commit_panel.clone();
        let runner_sub = cx.subscribe(&commit_panel_entity, move |this: &mut AppView, _panel, event: &RunnerEvent, cx| {
            match event {
                RunnerEvent::Start(cmd) => {
                    let state = &mut this.project_states[project_idx];
                    // Clear existing runner
                    state.right_panel.update(cx, |panel, _cx| panel.clear_runner());
                    state.runner_terminal = None;

                    // Create runner terminal in right panel
                    let project_path = state.path.clone();
                    let tv = cx.new(|cx| {
                        let mut view = TerminalView::new_in("Runner".to_string(), Some(project_path), cx);
                        view.compact = true;
                        view
                    });
                    state.runner_terminal = Some(tv.clone());
                    state.right_panel.update(cx, |panel, cx| {
                        panel.set_runner(tv.clone());
                        cx.notify();
                    });
                    // Sync topbar state
                    this.workspace.update(cx, |ws, cx| {
                        ws.is_running = true;
                        cx.notify();
                    });
                    cx.notify();

                    // Write command after shell initializes
                    let cmd = cmd.clone();
                    cx.spawn(async move |_this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                        cx.background_executor().timer(Duration::from_millis(300)).await;
                        tv.update(cx, |view, _cx| {
                            view.terminal.write_input(cmd.as_bytes());
                            view.terminal.write_input(b"\r");
                        }).ok();
                    }).detach();
                }
                RunnerEvent::Stop => {
                    let state = &mut this.project_states[project_idx];
                    // Send Ctrl+C to stop the running process
                    if let Some(ref terminal) = state.runner_terminal {
                        terminal.update(cx, |view, _cx| {
                            view.terminal.write_input(&[3]); // Ctrl+C
                        });
                    }
                    state.runner_terminal = None;
                    state.right_panel.update(cx, |panel, cx| {
                        panel.clear_runner();
                        cx.notify();
                    });
                    // Sync topbar state
                    this.workspace.update(cx, |ws, cx| {
                        ws.is_running = false;
                        cx.notify();
                    });
                }
            }
        });

        let right_panel_sub = cx.subscribe(&right_panel, |this: &mut AppView, _rp, event: &RightPanelEvent, cx| {
            match event {
                RightPanelEvent::LayoutChanged => {
                    this.save_session(cx);
                }
            }
        });

        // Subscribe to file open events from file explorer
        let file_explorer_entity = right_panel.read(cx).file_explorer.clone();
        let pane_for_fe = pane.clone();
        let pane_for_md = pane.clone();
        let file_explorer_sub = cx.subscribe(&file_explorer_entity, move |this: &mut AppView, _fe, event: &FileExplorerEvent, cx| {
            match event {
                FileExplorerEvent::FileOpened(path) => {
                    this.open_file_in_pane(&pane_for_fe, path.clone(), cx);
                }
                FileExplorerEvent::EditFile(path) => {
                    this.open_file_editor(pane_for_md.clone(), path.clone(), cx);
                }
            }
        });

        // Subscribe to file open events from git changes
        let git_changes_entity = right_panel.read(cx).git_changes.clone();
        let pane_for_gc = pane.clone();
        let git_changes_sub = cx.subscribe(&git_changes_entity, move |this: &mut AppView, _gc, event: &GitChangesEvent, cx| {
            match event {
                GitChangesEvent::FileOpened(path) => {
                    this.open_file_in_pane(&pane_for_gc, path.clone(), cx);
                }
            }
        });

        // Subscribe to commit click events from git log
        let git_log_entity = right_panel.read(cx).git_log.clone();
        let pane_for_gl = pane.clone();
        let project_path_for_gl = path.clone();
        let git_log_sub = cx.subscribe(&git_log_entity, move |_this: &mut AppView, _gl, event: &GitLogEvent, cx| {
            match event {
                GitLogEvent::CommitClicked { hash, message } => {
                    let detail = format!("commit:{}", hash);
                    let existing = pane_for_gl.read(cx).tabs.iter().position(|tab| tab.detail == detail);
                    if let Some(idx) = existing {
                        let tab_id = pane_for_gl.read(cx).tabs[idx].id;
                        pane_for_gl.update(cx, |p, cx| {
                            p.set_active_tab(tab_id);
                            cx.notify();
                        });
                    } else {
                        let title = format!("{}", &hash[..7.min(hash.len())]);
                        let diff_view = cx.new(|_cx| CommitDiffView::new(&project_path_for_gl, hash, message));
                        pane_for_gl.update(cx, |p, _cx| {
                            p.add_tab(title, "crates/app/assets/git-pull.svg", detail, AnyView::from(diff_view), true);
                        });
                    }
                    cx.notify();
                }
            }
        });

        let state = ProjectState {
            path,
            pane,
            right_panel,
            runner_terminal: None,
            _pane_sub: pane_sub,
            _runner_sub: runner_sub,
            _right_panel_sub: right_panel_sub,
            _file_explorer_sub: file_explorer_sub,
            _git_changes_sub: git_changes_sub,
            _git_log_sub: git_log_sub,
        };

        self.project_states.push(state);
        let idx = self.project_states.len() - 1;

        // Add to project panel
        let panel_path = self.project_states[idx].path.clone();
        let display_path = shorten_path(&panel_path);
        self.project_panel.update(cx, |panel, cx| {
            panel.projects.push(ProjectEntry { name, path: panel_path, display_path });
            panel.order.push(idx);
            panel.active_project = Some(idx);
            cx.notify();
        });

        // Switch to this project
        self.switch_project(idx, cx);
        self.save_session(cx);
    }

    fn switch_project(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.project_states.len() {
            return;
        }
        self.active_project = Some(idx);

        let state = &self.project_states[idx];
        let pane = state.pane.clone();
        let right_panel = state.right_panel.clone();

        // Sync topbar button states from the new active project
        let commit_panel = right_panel.read(cx).commit_panel.clone();
        let is_running = commit_panel.read(cx).is_running;
        let is_pushing = commit_panel.read(cx).is_pushing;

        self.workspace.update(cx, |ws, cx| {
            ws.center_pane = pane;
            ws.is_running = is_running;
            ws.is_pushing = is_pushing;
            ws.right_dock.update(cx, |dock, _cx| {
                dock.set_view(AnyView::from(right_panel));
            });
            cx.notify();
        });

        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Sync agent activity counts to project panel
        self.sync_activity_counts(cx);

        // Update layout dimensions global so terminals can compute their actual size
        {
            let ws = self.workspace.read(cx);
            let left_w = ws.left_dock.read(cx).width();
            let right_w = ws.right_dock.read(cx).width();
            let sidebar_w = ws.center_pane.read(cx).sidebar_width();
            cx.set_global(LayoutDimensions {
                left_dock_width: left_w,
                right_dock_width: right_w,
                pane_sidebar_width: sidebar_w,
            });
        }

        let mut base = div().size_full();

        // Wallpaper layer with crop positioning
        if let Some(ref wp) = self.wallpaper_path {
            let path = std::path::PathBuf::from(wp);
            if path.exists() {
                let crop_x = self.wallpaper_crop_x;
                let crop_y = self.wallpaper_crop_y;

                if let Some((img_w, img_h)) = self.wallpaper_img_size {
                    let win_bounds = window.bounds();
                    let win_w: f32 = win_bounds.size.width.into();
                    let win_h: f32 = win_bounds.size.height.into();

                    // Scale to cover, then apply zoom
                    let scale_x = win_w / img_w as f32;
                    let scale_y = win_h / img_h as f32;
                    let scale = scale_x.max(scale_y) * self.wallpaper_crop_zoom;
                    let scaled_w = img_w as f32 * scale;
                    let scaled_h = img_h as f32 * scale;

                    // Offset based on crop position
                    let offset_x = (scaled_w - win_w) * crop_x;
                    let offset_y = (scaled_h - win_h) * crop_y;

                    base = base.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .overflow_hidden()
                            .opacity(1.0 - self.wallpaper_opacity)
                            .child(
                                img(path)
                                    .absolute()
                                    .left(px(-offset_x))
                                    .top(px(-offset_y))
                                    .w(px(scaled_w))
                                    .h(px(scaled_h))
                            )
                    );
                } else {
                    // Fallback: no dimensions known, use cover
                    base = base.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .opacity(1.0 - self.wallpaper_opacity)
                            .child(
                                img(path)
                                    .size_full()
                                    .object_fit(ObjectFit::Cover)
                            )
                    );
                }
            }
        }

        base = base.child(self.workspace.clone());

        if let Some((percent, message)) = &self.update_status {
            let pct = *percent;
            let msg = message.clone();
            base = base.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000dd))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(16.))
                            .child(
                                div()
                                    .text_color(colors::text())
                                    .text_size(px(16.))
                                    .child(msg),
                            )
                            .child(
                                div()
                                    .w(px(300.))
                                    .h(px(6.))
                                    .rounded(px(3.))
                                    .bg(colors::surface1())
                                    .child(
                                        div()
                                            .h_full()
                                            .rounded(px(3.))
                                            .bg(colors::blue())
                                            .w(px(300. * pct as f32 / 100.)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(colors::subtext())
                                    .text_size(px(13.))
                                    .child(format!("{}%", pct)),
                            ),
                    ),
            );
        }

        // Crop picker overlay
        if self.crop_picker_open {
            if let (Some(ref wp), Some((img_w, img_h))) = (&self.wallpaper_path, self.wallpaper_img_size) {
                let win_bounds = window.bounds();
                let win_w: f32 = win_bounds.size.width.into();
                let win_h: f32 = win_bounds.size.height.into();
                let img_w = img_w as f32;
                let img_h = img_h as f32;

                // Preview area: 80% of window, centered
                let preview_w = win_w * 0.80;
                let preview_h = win_h * 0.70;

                // Scale image to fit in preview
                let img_scale = (preview_w / img_w).min(preview_h / img_h);
                let disp_w = img_w * img_scale;
                let disp_h = img_h * img_scale;

                // Screen frame: what portion of the image is visible at cover scale * zoom
                let zoom = self.wallpaper_crop_zoom;
                let base_cover = (win_w / img_w).max(win_h / img_h);
                let cover_scale = base_cover * zoom;
                let visible_w = win_w / cover_scale; // in image pixels
                let visible_h = win_h / cover_scale;
                let frame_w = (visible_w * img_scale).min(disp_w);
                let frame_h = (visible_h * img_scale).min(disp_h);

                // Frame position within displayed image
                let range_x = (disp_w - frame_w).max(0.0);
                let range_y = (disp_h - frame_h).max(0.0);
                let frame_x = self.wallpaper_crop_x * range_x;
                let frame_y = self.wallpaper_crop_y * range_y;

                // Image offset within preview area (centered)
                let img_offset_x = (preview_w - disp_w) / 2.0;
                let img_offset_y = (preview_h - disp_h) / 2.0;

                let path = std::path::PathBuf::from(wp);
                let zoom_label = format!("{}%", (zoom * 100.0).round() as u32);

                // Bounds tracking for cursor-centered zoom
                let container_bounds = self.crop_preview_bounds.clone();
                let container_bounds_for_canvas = self.crop_preview_bounds.clone();
                let entity = cx.entity().clone();

                base = base.child(
                    div()
                        .id("crop-picker-overlay")
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .bg(rgba(0x000000ee))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(12.))
                        // Title + zoom controls row
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap(px(16.))
                                // Title
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(colors::text())
                                        .child("Position your wallpaper"),
                                )
                                // Zoom controls (like image preview)
                                .child(
                                    div()
                                        .flex()
                                        .flex_row()
                                        .items_center()
                                        .gap(px(4.))
                                        // Reset button
                                        .child(
                                            div()
                                                .id("crop-zoom-reset")
                                                .flex().items_center().justify_center()
                                                .h(px(24.)).px(px(8.)).rounded(px(4.))
                                                .cursor_pointer().text_xs()
                                                .text_color(colors::subtext())
                                                .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                                .child("Reset")
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.wallpaper_crop_zoom = 1.0;
                                                    this.wallpaper_crop_x = 0.5;
                                                    this.wallpaper_crop_y = 0.5;
                                                    cx.notify();
                                                }))
                                        )
                                        .child(div().w(px(1.)).h(px(16.)).bg(colors::surface1()))
                                        // Zoom out
                                        .child(
                                            div()
                                                .id("crop-zoom-out")
                                                .flex().items_center().justify_center()
                                                .w(px(24.)).h(px(24.)).rounded(px(4.))
                                                .cursor_pointer().text_sm().font_weight(FontWeight::BOLD)
                                                .text_color(colors::subtext())
                                                .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                                .child("\u{2212}")
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.wallpaper_crop_zoom = (this.wallpaper_crop_zoom / 1.25).max(1.0);
                                                    cx.notify();
                                                }))
                                        )
                                        // Zoom percentage
                                        .child(
                                            div()
                                                .id("crop-zoom-label")
                                                .flex().items_center().justify_center()
                                                .min_w(px(48.)).h(px(24.)).rounded(px(4.))
                                                .cursor_pointer().text_xs().text_color(colors::subtext())
                                                .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                                .child(zoom_label)
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.wallpaper_crop_zoom = 1.0;
                                                    cx.notify();
                                                }))
                                        )
                                        // Zoom in
                                        .child(
                                            div()
                                                .id("crop-zoom-in")
                                                .flex().items_center().justify_center()
                                                .w(px(24.)).h(px(24.)).rounded(px(4.))
                                                .cursor_pointer().text_sm().font_weight(FontWeight::BOLD)
                                                .text_color(colors::subtext())
                                                .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                                .child("+")
                                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                                    this.wallpaper_crop_zoom = (this.wallpaper_crop_zoom * 1.25).min(5.0);
                                                    cx.notify();
                                                }))
                                        )
                                )
                        )
                        // Preview area with image + frame
                        .child(
                            div()
                                .id("crop-preview")
                                .relative()
                                .w(px(preview_w))
                                .h(px(preview_h))
                                .overflow_hidden()
                                .cursor(if self.crop_dragging { CursorStyle::ClosedHand } else { CursorStyle::OpenHand })
                                // Invisible canvas to capture preview bounds
                                .child(
                                    canvas(
                                        move |bounds, _window, _cx| {
                                            container_bounds_for_canvas.set((
                                                f32::from(bounds.origin.x),
                                                f32::from(bounds.origin.y),
                                                f32::from(bounds.size.width),
                                                f32::from(bounds.size.height),
                                            ));
                                        },
                                        |_bounds, _, _window, _cx| {},
                                    )
                                    .size_full()
                                    .absolute()
                                    .top(px(0.))
                                    .left(px(0.))
                                )
                                // Full image
                                .child(
                                    img(path)
                                        .absolute()
                                        .left(px(img_offset_x))
                                        .top(px(img_offset_y))
                                        .w(px(disp_w))
                                        .h(px(disp_h))
                                )
                                // Dark overlay on top of image (outside frame area)
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(img_offset_x))
                                        .top(px(img_offset_y))
                                        .w(px(disp_w))
                                        .h(px(disp_h))
                                        .bg(rgba(0x00000088))
                                )
                                // Bright frame (the visible crop area)
                                .child(
                                    div()
                                        .absolute()
                                        .left(px(img_offset_x + frame_x))
                                        .top(px(img_offset_y + frame_y))
                                        .w(px(frame_w))
                                        .h(px(frame_h))
                                        .border_2()
                                        .border_color(colors::blue())
                                        .overflow_hidden()
                                        // Show the image portion inside the frame (bright, no overlay)
                                        .child(
                                            img(std::path::PathBuf::from(self.wallpaper_path.as_ref().unwrap()))
                                                .absolute()
                                                .left(px(-frame_x))
                                                .top(px(-frame_y))
                                                .w(px(disp_w))
                                                .h(px(disp_h))
                                        )
                                )
                                // Drag handling
                                .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, _window, cx| {
                                    let mx: f32 = ev.position.x.into();
                                    let my: f32 = ev.position.y.into();
                                    this.crop_drag_start = Some((mx, my));
                                    this.crop_drag_initial = (this.wallpaper_crop_x, this.wallpaper_crop_y);
                                    this.crop_dragging = true;
                                    cx.notify();
                                }))
                                .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _window, cx| {
                                    if let Some((start_x, start_y)) = this.crop_drag_start {
                                        let mx: f32 = ev.position.x.into();
                                        let my: f32 = ev.position.y.into();
                                        let dx = mx - start_x;
                                        let dy = my - start_y;
                                        let (init_x, init_y) = this.crop_drag_initial;

                                        let new_x = if range_x > 0.0 {
                                            (init_x + dx / range_x).clamp(0.0, 1.0)
                                        } else { 0.5 };
                                        let new_y = if range_y > 0.0 {
                                            (init_y + dy / range_y).clamp(0.0, 1.0)
                                        } else { 0.5 };

                                        this.wallpaper_crop_x = new_x;
                                        this.wallpaper_crop_y = new_y;
                                        cx.notify();
                                    }
                                }))
                                .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                                    this.crop_drag_start = None;
                                    this.crop_dragging = false;
                                    cx.notify();
                                }))
                                .on_mouse_up_out(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                                    this.crop_drag_start = None;
                                    this.crop_dragging = false;
                                    cx.notify();
                                }))
                                // Scroll zoom centered on cursor (like image preview)
                                .on_scroll_wheel(cx.listener(move |this, ev: &ScrollWheelEvent, _window, cx| {
                                    let delta_y: f32 = match ev.delta {
                                        ScrollDelta::Lines(d) => d.y,
                                        ScrollDelta::Pixels(d) => d.y / px(40.0),
                                    };
                                    let old_zoom = this.wallpaper_crop_zoom;
                                    let new_zoom = if delta_y > 0.0 {
                                        (old_zoom / 1.15).max(1.0)
                                    } else if delta_y < 0.0 {
                                        (old_zoom * 1.15).min(5.0)
                                    } else {
                                        old_zoom
                                    };
                                    if (new_zoom - old_zoom).abs() < f32::EPSILON {
                                        return;
                                    }

                                    // Cursor-centered zoom: adjust crop position so the point
                                    // under the cursor stays fixed in image space
                                    if let Some((iw, ih)) = this.wallpaper_img_size {
                                        let (ox, oy, _, _) = container_bounds.get();
                                        let iw = iw as f32;
                                        let ih = ih as f32;

                                        // Cursor in preview-local coords
                                        let mx = f32::from(ev.position.x) - ox;
                                        let my = f32::from(ev.position.y) - oy;

                                        // Cursor position in image-pixel space
                                        let cursor_img_x = (mx - img_offset_x) / img_scale;
                                        let cursor_img_y = (my - img_offset_y) / img_scale;

                                        // Old frame in image-pixel space
                                        let old_cover = base_cover * old_zoom;
                                        let old_vis_w = win_w / old_cover;
                                        let old_vis_h = win_h / old_cover;
                                        let old_range_img_x = (iw - old_vis_w).max(0.0);
                                        let old_range_img_y = (ih - old_vis_h).max(0.0);
                                        let old_frame_x = this.wallpaper_crop_x * old_range_img_x;
                                        let old_frame_y = this.wallpaper_crop_y * old_range_img_y;

                                        // Cursor fraction within old frame
                                        let frac_x = if old_vis_w > 0.0 { (cursor_img_x - old_frame_x) / old_vis_w } else { 0.5 };
                                        let frac_y = if old_vis_h > 0.0 { (cursor_img_y - old_frame_y) / old_vis_h } else { 0.5 };

                                        // New frame in image-pixel space
                                        let new_cover = base_cover * new_zoom;
                                        let new_vis_w = win_w / new_cover;
                                        let new_vis_h = win_h / new_cover;
                                        let new_range_img_x = (iw - new_vis_w).max(0.0);
                                        let new_range_img_y = (ih - new_vis_h).max(0.0);

                                        // New frame position so cursor stays at same fraction
                                        let new_frame_x = cursor_img_x - frac_x * new_vis_w;
                                        let new_frame_y = cursor_img_y - frac_y * new_vis_h;

                                        this.wallpaper_crop_x = if new_range_img_x > 0.0 {
                                            (new_frame_x / new_range_img_x).clamp(0.0, 1.0)
                                        } else { 0.5 };
                                        this.wallpaper_crop_y = if new_range_img_y > 0.0 {
                                            (new_frame_y / new_range_img_y).clamp(0.0, 1.0)
                                        } else { 0.5 };
                                    }

                                    this.wallpaper_crop_zoom = new_zoom;
                                    cx.notify();
                                })),
                        )
                        // Buttons
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .id("crop-confirm")
                                        .px(px(20.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .bg(colors::blue())
                                        .text_sm()
                                        .text_color(colors::base())
                                        .font_weight(FontWeight::BOLD)
                                        .hover(|d| d.opacity(0.85))
                                        .child("Confirm")
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.crop_picker_open = false;
                                            this.crop_dragging = false;
                                            this.save_settings();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id("crop-cancel")
                                        .px(px(20.))
                                        .py(px(8.))
                                        .rounded(px(6.))
                                        .cursor_pointer()
                                        .bg(colors::surface1())
                                        .text_sm()
                                        .text_color(colors::text())
                                        .hover(|d| d.opacity(0.85))
                                        .child("Cancel")
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.crop_picker_open = false;
                                            this.crop_dragging = false;
                                            this.wallpaper_crop_x = 0.5;
                                            this.wallpaper_crop_y = 0.5;
                                            this.wallpaper_crop_zoom = 1.0;
                                            this.save_settings();
                                            cx.notify();
                                        })),
                                ),
                        ),
                );
            }
        }

        if self.settings_open {
            let current = theme::current_name();
            base = base.child(
                div()
                    .id("settings-overlay")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(rgba(0x000000cc))
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                        this.settings_open = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id("settings-panel")
                            .flex()
                            .flex_col()
                            .w(px(500.))
                            .max_h(px(700.))
                            .bg(colors::mantle())
                            .border_1()
                            .border_color(colors::surface1())
                            .rounded(px(8.))
                            .overflow_hidden()
                            .on_mouse_down(MouseButton::Left, |_ev: &MouseDownEvent, _window, cx| {
                                cx.stop_propagation();
                            })
                            // Header
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .w_full()
                                    .h(px(44.))
                                    .px(px(16.))
                                    .flex_shrink_0()
                                    .border_b_1()
                                    .border_color(colors::surface1())
                                    .child(
                                        div()
                                            .flex_1()
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(colors::text())
                                            .child("Settings"),
                                    )
                                    .child(
                                        div()
                                            .id("close-settings")
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(24.))
                                            .h(px(24.))
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .text_size(px(16.))
                                            .text_color(colors::overlay())
                                            .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                            .child("×")
                                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                                this.settings_open = false;
                                                cx.notify();
                                            })),
                                    ),
                            )
                            // Theme section
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .p(px(16.))
                                    .gap(px(12.))
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(colors::subtext())
                                            .child("THEME"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.))
                                            .children(ThemeName::all().iter().map(|&name| {
                                                let is_selected = name == current;
                                                let tc = name.colors();
                                                div()
                                                    .id(ElementId::Name(format!("theme-{}", name.as_str()).into()))
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .w_full()
                                                    .px(px(12.))
                                                    .py(px(10.))
                                                    .rounded(px(6.))
                                                    .cursor_pointer()
                                                    .when(is_selected, |d: Stateful<Div>| {
                                                        d.bg(colors::surface0())
                                                            .border_1()
                                                            .border_color(colors::blue())
                                                    })
                                                    .when(!is_selected, |d: Stateful<Div>| {
                                                        d.hover(|d| d.bg(colors::surface0()))
                                                    })
                                                    // Color preview swatches
                                                    .child(
                                                        div()
                                                            .flex()
                                                            .flex_row()
                                                            .gap(px(3.))
                                                            .mr(px(12.))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.base)))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.surface0)))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.blue)))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.green)))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.red)))
                                                            .child(div().w(px(16.)).h(px(16.)).rounded(px(3.)).bg(rgb(tc.lavender)))
                                                    )
                                                    // Theme name
                                                    .child(
                                                        div()
                                                            .flex_1()
                                                            .text_size(px(13.))
                                                            .text_color(if is_selected { colors::text() } else { colors::subtext() })
                                                            .font_weight(if is_selected { FontWeight::BOLD } else { FontWeight::NORMAL })
                                                            .child(name.label()),
                                                    )
                                                    // Checkmark for selected
                                                    .when(is_selected, |d: Stateful<Div>| {
                                                        d.child(
                                                            div()
                                                                .text_size(px(14.))
                                                                .text_color(colors::blue())
                                                                .child("✓"),
                                                        )
                                                    })
                                                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                        this.apply_theme(name, cx);
                                                    }))
                                            })),
                                    ),
                            )
                            // Wallpaper section
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .p(px(16.))
                                    .gap(px(12.))
                                    .border_t_1()
                                    .border_color(colors::surface1())
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(colors::subtext())
                                            .child("WALLPAPER"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .gap(px(12.))
                                            .child(
                                                div()
                                                    .id("choose-wallpaper")
                                                    .px(px(12.))
                                                    .py(px(8.))
                                                    .rounded(px(6.))
                                                    .cursor_pointer()
                                                    .bg(colors::surface0())
                                                    .text_sm()
                                                    .text_color(colors::text())
                                                    .hover(|d| d.bg(colors::surface1()))
                                                    .child("Choose Image...")
                                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                                        this.settings_open = false;
                                                        cx.notify();
                                                        let receiver = cx.prompt_for_paths(PathPromptOptions {
                                                            files: true,
                                                            directories: false,
                                                            multiple: false,
                                                            prompt: Some("Select wallpaper image".into()),
                                                        });
                                                        cx.spawn(async |this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                                            if let Ok(Ok(Some(paths))) = receiver.await {
                                                                if let Some(path) = paths.first() {
                                                                    let path_str = path.display().to_string();
                                                                    this.update(cx, |view, cx| {
                                                                        view.apply_wallpaper(Some(path_str), cx);
                                                                    }).ok();
                                                                }
                                                            }
                                                        }).detach();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_size(px(11.))
                                                    .text_color(colors::subtext())
                                                    .truncate()
                                                    .child(
                                                        self.wallpaper_path
                                                            .as_ref()
                                                            .map(|p| PathBuf::from(p).file_name()
                                                                .map(|n| n.to_string_lossy().to_string())
                                                                .unwrap_or_else(|| p.clone()))
                                                            .unwrap_or_else(|| "None".to_string())
                                                    ),
                                            ),
                                    )
                                    .when(self.wallpaper_path.is_some(), |d: Div| {
                                        let opacity = self.wallpaper_opacity;
                                        let pct = format!("{}%", (opacity * 100.0).round() as u32);
                                        let slider_fill = opacity as f64;
                                        d
                                            // Opacity slider: [ - ] ████░░░░ [ + ]  80%
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_row()
                                                    .items_center()
                                                    .gap(px(8.))
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(colors::subtext())
                                                            .child("Opacity"),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("opacity-minus")
                                                            .px(px(8.))
                                                            .py(px(4.))
                                                            .rounded(px(6.))
                                                            .cursor_pointer()
                                                            .bg(colors::surface0())
                                                            .text_sm()
                                                            .text_color(colors::text())
                                                            .hover(|d| d.bg(colors::surface1()))
                                                            .child("\u{2212}") // minus sign
                                                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                                let new_val = (opacity - 0.05).max(0.0);
                                                                this.apply_wallpaper_opacity(new_val, cx);
                                                            })),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("opacity-slider")
                                                            .flex_1()
                                                            .h(px(6.))
                                                            .rounded(px(3.))
                                                            .bg(colors::surface1())
                                                            .child(
                                                                div()
                                                                    .h_full()
                                                                    .rounded(px(3.))
                                                                    .bg(colors::blue())
                                                                    .w(relative(slider_fill as f32)),
                                                            ),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("opacity-plus")
                                                            .px(px(8.))
                                                            .py(px(4.))
                                                            .rounded(px(6.))
                                                            .cursor_pointer()
                                                            .bg(colors::surface0())
                                                            .text_sm()
                                                            .text_color(colors::text())
                                                            .hover(|d| d.bg(colors::surface1()))
                                                            .child("+")
                                                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                                let new_val = (opacity + 0.05).min(1.0);
                                                                this.apply_wallpaper_opacity(new_val, cx);
                                                            })),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(colors::text())
                                                            .w(px(36.))
                                                            .child(pct),
                                                    ),
                                            )
                                            // Remove wallpaper button
                                            .child(
                                                div()
                                                    .id("remove-wallpaper")
                                                    .px(px(12.))
                                                    .py(px(6.))
                                                    .rounded(px(6.))
                                                    .cursor_pointer()
                                                    .bg(colors::surface0())
                                                    .text_sm()
                                                    .text_color(colors::red())
                                                    .hover(|d| d.bg(colors::surface1()))
                                                    .child("Remove Wallpaper")
                                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                                        this.apply_wallpaper(None, cx);
                                                    })),
                                            )
                                    }),
                            ),
                    ),
            );
        }

        // ── Toast notifications (top-right floating) ──
        if !self.toasts.is_empty() {
            base = base.child(
                div()
                    .absolute()
                    .top(px(44.))
                    .right(px(12.))
                    .w(px(300.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(self.toasts.iter().map(|toast| {
                        let toast_id = toast.id;
                        let (accent, icon) = match toast.kind {
                            ToastKind::Progress => (colors::blue(), "\u{25cf}"),
                            ToastKind::Success => (colors::green(), "\u{2713}"),
                            ToastKind::Error => (colors::red(), "\u{2717}"),
                        };

                        let mut card = div()
                            .id(ElementId::Name(format!("toast-{}", toast_id).into()))
                            .flex()
                            .flex_col()
                            .w_full()
                            .bg(colors::mantle())
                            .border_1()
                            .border_color(colors::surface1())
                            .rounded(px(8.))
                            .shadow_lg()
                            .overflow_hidden();

                        // Progress bar at top
                        if let Some(pct) = toast.percent {
                            card = card.child(
                                div()
                                    .w_full()
                                    .h(px(3.))
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(pct as f32 / 100.0))
                                            .bg(accent)
                                    )
                            );
                        }

                        // Content
                        card = card.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .gap(px(10.))
                                .px(px(12.))
                                .py(px(10.))
                                // Icon
                                .child(
                                    div()
                                        .text_color(accent)
                                        .font_weight(FontWeight::BOLD)
                                        .text_sm()
                                        .mt(px(1.))
                                        .child(icon)
                                )
                                // Text
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap(px(2.))
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .text_color(colors::text())
                                                .child(toast.label.clone())
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(colors::subtext())
                                                .child(toast.message.clone())
                                        )
                                )
                                // Close button
                                .when(toast.kind != ToastKind::Progress, |d: Div| {
                                    d.child(
                                        div()
                                            .id(ElementId::Name(format!("toast-close-{}", toast_id).into()))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .w(px(16.))
                                            .h(px(16.))
                                            .rounded(px(3.))
                                            .text_xs()
                                            .text_color(colors::overlay())
                                            .cursor_pointer()
                                            .hover(|d| d.text_color(colors::text()).bg(colors::surface0()))
                                            .child("\u{00d7}")
                                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                                this.dismiss_toast(toast_id, cx);
                                            }))
                                    )
                                })
                        );

                        card
                    }))
            );
        }

        base
    }
}

// ── Main ─────────────────────────────────────────────────────

/// Asset source that embeds SVG icons at compile time
struct EmbeddedAssets;

impl AssetSource for EmbeddedAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match path {
            "crates/app/assets/terminal.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/terminal.svg")))),
            "crates/app/assets/file.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/file.svg")))),
            "crates/app/assets/folder.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/folder.svg")))),
            "crates/app/assets/git-pull.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/git-pull.svg")))),
            "crates/app/assets/markdown.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/markdown.svg")))),
            "crates/app/assets/git-push.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/git-push.svg")))),
            "crates/app/assets/image.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/image.svg")))),
            "crates/app/assets/claude.svg" => Ok(Some(std::borrow::Cow::Borrowed(include_bytes!("../assets/claude.svg")))),
            _ => Ok(None),
        }
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

fn main() {
    env_logger::init();

    Application::new().with_assets(EmbeddedAssets).run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Forge".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window: &mut Window, cx: &mut App| {
                let workspace = cx.new(|cx| IdeWorkspace::new(cx));

                // Setup left dock: Project panel
                let project_panel = cx.new(|_cx| ProjectPanel::new());
                let project_panel_for_dock = project_panel.clone();
                workspace.update(cx, |ws, cx| {
                    ws.left_dock.update(cx, |dock, _cx| {
                        dock.set_view(AnyView::from(project_panel_for_dock));
                    });
                });

                cx.new(|cx| {
                    // Subscribe to project panel events
                    let project_sub = cx.subscribe(&project_panel, |this: &mut AppView, _panel, event: &ProjectPanelEvent, cx| {
                        match event {
                            ProjectPanelEvent::AddProjectRequested => {
                                let receiver = cx.prompt_for_paths(PathPromptOptions {
                                    files: false,
                                    directories: true,
                                    multiple: false,
                                    prompt: Some("Select project folder".into()),
                                });

                                cx.spawn(async |this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                    if let Ok(Ok(Some(paths))) = receiver.await {
                                        if let Some(path) = paths.first() {
                                            let path = path.clone();
                                            this.update(cx, |view, cx| {
                                                view.add_project(path, cx);
                                            }).ok();
                                        }
                                    }
                                }).detach();
                            }
                            ProjectPanelEvent::ProjectSelected(idx, _path) => {
                                this.switch_project(*idx, cx);
                                this.project_panel.update(cx, |panel, cx| {
                                    panel.active_project = Some(*idx);
                                    cx.notify();
                                });
                            }
                            ProjectPanelEvent::ProjectClosed(idx) => {
                                let idx = *idx;
                                if this.project_states.len() <= 1 {
                                    return;
                                }
                                this.project_states.remove(idx);
                                this.project_panel.update(cx, |panel, cx| {
                                    panel.projects.remove(idx);
                                    // Remove from order and shift indices > idx
                                    panel.order.retain(|&i| i != idx);
                                    for v in panel.order.iter_mut() {
                                        if *v > idx { *v -= 1; }
                                    }
                                    cx.notify();
                                });
                                // Switch to a valid project
                                let new_idx = if idx >= this.project_states.len() {
                                    this.project_states.len() - 1
                                } else {
                                    idx
                                };
                                this.switch_project(new_idx, cx);
                                this.project_panel.update(cx, |panel, cx| {
                                    panel.active_project = Some(new_idx);
                                    cx.notify();
                                });
                                this.save_session(cx);
                            }
                            ProjectPanelEvent::ProjectReordered => {
                                this.save_session(cx);
                            }
                        }
                    });

                    // Subscribe to workspace update button
                    let workspace_sub = cx.subscribe(&workspace, |this: &mut AppView, _ws, event: &WorkspaceEvent, cx| {
                        match event {
                            WorkspaceEvent::UpdateClicked => {
                                let info = this.update_info.clone();
                                if let Some(info) = info {
                                    this.update_status = Some((0, "Preparing update...".to_string()));
                                    cx.notify();
                                    cx.spawn(async move |this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                        // Step 1: Get expected size
                                        log::info!("Updater: starting download of v{}", info.version);
                                        this.update(cx, |view, cx| {
                                            view.update_status = Some((2, "Preparing download...".to_string()));
                                            cx.notify();
                                        }).ok();

                                        let url = info.download_url.clone();
                                        let expected_size: Option<u64> = cx.background_executor().spawn(async move {
                                            updater::get_download_size(&url)
                                        }).await;

                                        // Step 2: Start download as child process
                                        let info_clone = info.clone();
                                        let child_result = cx.background_executor().spawn(async move {
                                            updater::start_download(&info_clone)
                                        }).await;

                                        let mut child = match child_result {
                                            Ok(c) => c,
                                            Err(e) => {
                                                log::error!("Updater: download failed: {}", e);
                                                this.update(cx, |view, cx| {
                                                    view.update_status = Some((0, format!("Error: {}", e)));
                                                    cx.notify();
                                                }).ok();
                                                cx.background_executor().timer(Duration::from_secs(3)).await;
                                                this.update(cx, |view, cx| {
                                                    view.update_status = None;
                                                    cx.notify();
                                                }).ok();
                                                return;
                                            }
                                        };

                                        // Step 3: Poll download progress
                                        loop {
                                            cx.background_executor().timer(Duration::from_millis(200)).await;
                                            let downloaded = updater::download_progress();
                                            let pct = if let Some(total) = expected_size {
                                                ((downloaded as f64 / total as f64) * 55.0) as u8 + 5
                                            } else {
                                                // No size info: animate between 5-50
                                                let t = (downloaded / (1024 * 100)) as u8;
                                                (t.min(45)) + 5
                                            };
                                            let size_mb = downloaded as f64 / (1024.0 * 1024.0);
                                            let msg = if let Some(total) = expected_size {
                                                let total_mb = total as f64 / (1024.0 * 1024.0);
                                                format!("Downloading... {:.1} / {:.1} MB", size_mb, total_mb)
                                            } else {
                                                format!("Downloading... {:.1} MB", size_mb)
                                            };
                                            this.update(cx, |view, cx| {
                                                view.update_status = Some((pct.min(58), msg));
                                                cx.notify();
                                            }).ok();

                                            // Check if curl finished
                                            match child.try_wait() {
                                                Ok(Some(status)) => {
                                                    if !status.success() {
                                                        log::error!("Updater: curl exited with {}", status);
                                                        this.update(cx, |view, cx| {
                                                            view.update_status = Some((0, "Error: download failed".to_string()));
                                                            cx.notify();
                                                        }).ok();
                                                        cx.background_executor().timer(Duration::from_secs(3)).await;
                                                        this.update(cx, |view, cx| {
                                                            view.update_status = None;
                                                            cx.notify();
                                                        }).ok();
                                                        return;
                                                    }
                                                    break;
                                                }
                                                Ok(None) => continue, // still running
                                                Err(e) => {
                                                    log::error!("Updater: wait error: {}", e);
                                                    break;
                                                }
                                            }
                                        }

                                        // Verify download
                                        if let Err(e) = updater::verify_download() {
                                            log::error!("Updater: verify failed: {}", e);
                                            this.update(cx, |view, cx| {
                                                view.update_status = Some((0, format!("Error: {}", e)));
                                                cx.notify();
                                            }).ok();
                                            cx.background_executor().timer(Duration::from_secs(3)).await;
                                            this.update(cx, |view, cx| {
                                                view.update_status = None;
                                                cx.notify();
                                            }).ok();
                                            return;
                                        }

                                        // Step 4: Install (60% -> 90%)
                                        log::info!("Updater: download complete, installing...");
                                        this.update(cx, |view, cx| {
                                            view.update_status = Some((60, "Installing update...".to_string()));
                                            cx.notify();
                                        }).ok();

                                        let install_result = cx.background_executor().spawn(async {
                                            updater::update_step_install()
                                        }).await;

                                        if let Err(e) = install_result {
                                            log::error!("Updater: install failed: {}", e);
                                            this.update(cx, |view, cx| {
                                                view.update_status = Some((0, format!("Error: {}", e)));
                                                cx.notify();
                                            }).ok();
                                            cx.background_executor().timer(Duration::from_secs(3)).await;
                                            this.update(cx, |view, cx| {
                                                view.update_status = None;
                                                cx.notify();
                                            }).ok();
                                            return;
                                        }

                                        // Step 5: Restart (100%)
                                        log::info!("Updater: install complete, restarting...");
                                        this.update(cx, |view, cx| {
                                            view.update_status = Some((100, "Restarting...".to_string()));
                                            cx.notify();
                                        }).ok();

                                        cx.background_executor().timer(Duration::from_millis(500)).await;
                                        updater::relaunch();
                                    }).detach();
                                }
                            }
                            WorkspaceEvent::SettingsClicked => {
                                this.settings_open = !this.settings_open;
                                cx.notify();
                            }
                            WorkspaceEvent::RunClicked => {
                                if let Some(idx) = this.active_project {
                                    let commit_panel = this.project_states[idx].right_panel.read(cx).commit_panel.clone();
                                    commit_panel.update(cx, |panel, cx| {
                                        panel.toggle_runner(cx);
                                    });
                                    let is_running = commit_panel.read(cx).is_running;
                                    this.workspace.update(cx, |ws, cx| {
                                        ws.is_running = is_running;
                                        cx.notify();
                                    });
                                }
                            }
                            WorkspaceEvent::LayoutChanged => {
                                this.save_session(cx);
                            }
                            WorkspaceEvent::PushClicked => {
                                if let Some(idx) = this.active_project {
                                    let commit_panel = this.project_states[idx].right_panel.read(cx).commit_panel.clone();
                                    let is_pushing = commit_panel.read(cx).is_pushing;
                                    if is_pushing {
                                        return;
                                    }
                                    commit_panel.update(cx, |panel, cx| {
                                        panel.is_pushing = true;
                                        cx.notify();
                                    });
                                    this.workspace.update(cx, |ws, cx| {
                                        ws.is_pushing = true;
                                        cx.notify();
                                    });

                                    let toast_id = this.add_toast("Push", "Committing & pushing...", ToastKind::Progress, Some(30), cx);
                                    let root = this.project_states[idx].path.clone();
                                    let ws = this.workspace.clone();
                                    let cp = commit_panel.clone();
                                    cx.spawn(async move |this_weak: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                        let result = cx.background_executor().spawn(async move {
                                            ide_git_panel::operations::one_button_commit_and_push(&root)
                                        }).await;

                                        cp.update(cx, |panel, cx| {
                                            panel.is_pushing = false;
                                            cx.notify();
                                        }).ok();
                                        ws.update(cx, |ws, cx| {
                                            ws.is_pushing = false;
                                            cx.notify();
                                        }).ok();

                                        this_weak.update(cx, |view, cx| {
                                            match result {
                                                Ok(msg) => view.update_toast(toast_id, &format!("Pushed: {}", msg), ToastKind::Success, Some(100), cx),
                                                Err(e) => view.update_toast(toast_id, &format!("{}", e), ToastKind::Error, None, cx),
                                            }
                                        }).ok();

                                        // Auto-dismiss after 4s
                                        cx.background_executor().timer(Duration::from_secs(4)).await;
                                        this_weak.update(cx, |view, cx| {
                                            view.dismiss_toast(toast_id, cx);
                                        }).ok();
                                    }).detach();
                                }
                            }
                            WorkspaceEvent::PullClicked => {
                                if let Some(idx) = this.active_project {
                                    let is_pulling = this.workspace.read(cx).is_pulling;
                                    if is_pulling {
                                        return;
                                    }
                                    this.workspace.update(cx, |ws, cx| {
                                        ws.is_pulling = true;
                                        cx.notify();
                                    });

                                    let toast_id = this.add_toast("Pull", "Pulling...", ToastKind::Progress, Some(50), cx);
                                    let root = this.project_states[idx].path.clone();
                                    let ws = this.workspace.clone();
                                    cx.spawn(async move |this_weak: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                        let result = cx.background_executor().spawn(async move {
                                            ide_git_panel::operations::pull(&root)
                                        }).await;

                                        ws.update(cx, |ws, cx| {
                                            ws.is_pulling = false;
                                            cx.notify();
                                        }).ok();

                                        this_weak.update(cx, |view, cx| {
                                            match result {
                                                Ok(()) => view.update_toast(toast_id, "Pull successful", ToastKind::Success, Some(100), cx),
                                                Err(e) => view.update_toast(toast_id, &format!("{}", e), ToastKind::Error, None, cx),
                                            }
                                        }).ok();

                                        // Auto-dismiss after 4s
                                        cx.background_executor().timer(Duration::from_secs(4)).await;
                                        this_weak.update(cx, |view, cx| {
                                            view.dismiss_toast(toast_id, cx);
                                        }).ok();
                                    }).detach();
                                }
                            }
                        }
                    });

                    // Check for updates on startup
                    let workspace_for_update = workspace.clone();
                    let update_task = cx.spawn(async |this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                        let ws = workspace_for_update;
                        cx.background_executor().timer(Duration::from_secs(3)).await;
                        let update_info = cx.background_executor().spawn(async {
                            updater::check_for_update()
                        }).await;

                        if let Some(info) = update_info {
                            let version = info.version.clone();
                            ws.update(cx, |ws, cx| {
                                ws.update_version = Some(version);
                                cx.notify();
                            }).ok();
                            this.update(cx, |view, cx| {
                                view.update_info = Some(info);
                                cx.notify();
                            }).ok();
                        }
                    });

                    // Load settings and apply theme + wallpaper
                    let saved_settings = settings::load();
                    theme::set_theme(saved_settings.theme);
                    theme::set_wallpaper(saved_settings.wallpaper.clone());
                    theme::set_wallpaper_opacity(saved_settings.wallpaper_opacity);

                    // Load image dimensions if wallpaper is set
                    let wallpaper_img_size = saved_settings.wallpaper.as_ref()
                        .and_then(|p| image_dimensions(std::path::Path::new(p)));

                    let mut app_view = AppView {
                        workspace,
                        project_panel,
                        project_states: Vec::new(),
                        active_project: None,
                        terminal_count: 0,
                        update_info: None,
                        update_status: None,
                        settings_open: false,
                        wallpaper_path: saved_settings.wallpaper,
                        wallpaper_opacity: saved_settings.wallpaper_opacity,
                        wallpaper_crop_x: saved_settings.wallpaper_crop_x,
                        wallpaper_crop_y: saved_settings.wallpaper_crop_y,
                        wallpaper_crop_zoom: saved_settings.wallpaper_crop_zoom,
                        wallpaper_img_size,
                        crop_picker_open: false,
                        crop_drag_start: None,
                        crop_drag_initial: (0.5, 0.5),
                        crop_dragging: false,
                        crop_preview_bounds: std::rc::Rc::new(std::cell::Cell::new((0.0, 0.0, 0.0, 0.0))),
                        toasts: Vec::new(),
                        next_toast_id: 0,
                        _project_subscription: project_sub,
                        _workspace_subscription: workspace_sub,
                        _update_task: update_task,
                    };

                    // Restore previous session
                    if let Some(saved) = session::load() {
                        // Restore layout dimensions
                        let layout = &saved.layout;
                        app_view.workspace.update(cx, |ws, cx| {
                            ws.left_dock.update(cx, |dock, _cx| dock.set_width(layout.left_dock_width));
                            ws.right_dock.update(cx, |dock, _cx| dock.set_width(layout.right_dock_width));
                            cx.notify();
                        });

                        for path in &saved.projects {
                            app_view.add_project_with_cmd(path.clone(), Some("ccc"), cx);
                        }
                        if saved.active < app_view.project_states.len() {
                            app_view.switch_project(saved.active, cx);
                            app_view.project_panel.update(cx, |panel, cx| {
                                panel.active_project = Some(saved.active);
                                cx.notify();
                            });
                        }

                        // Restore pane sidebar width on all project panes
                        let sidebar_w = layout.pane_sidebar_width;
                        let log_h = layout.log_height;
                        let log_exp = layout.log_expanded;
                        for state in &app_view.project_states {
                            state.pane.update(cx, |pane, _cx| {
                                pane.set_sidebar_width(sidebar_w);
                            });
                            state.right_panel.update(cx, |rp, cx| {
                                rp.log_height = log_h;
                                rp.log_expanded = log_exp;
                                rp.git_log.update(cx, |log, _| {
                                    log.visible_height = log_h - 28.0;
                                });
                            });
                        }
                    }

                    app_view
                })
            },
        )
        .unwrap();
    });
}
