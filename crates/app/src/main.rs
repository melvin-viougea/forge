mod updater;
mod session;
mod settings;

use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

use ide_file_explorer::FileExplorerPanel;
use ide_git_panel::{CommitPanel, GitChangesPanel, GitLogPanel, RunnerEvent};
use ide_workspace::theme::{self, ThemeName};
use ide_terminal::{LayoutDimensions, TerminalView, TerminalViewEvent};
use ide_workspace::{IdeWorkspace, Pane, PaneEvent, WorkspaceEvent};

// ── Project Panel (left sidebar) ─────────────────────────────

struct ProjectPanel {
    projects: Vec<ProjectEntry>,
    active_project: Option<usize>,
    order: Vec<usize>,
    drop_indicator: Option<usize>,
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
            .bg(colors::mantle_bg())
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
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w(px(0.))
                                            .truncate()
                                            .text_sm()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(if is_active { gpui::rgb(0xffffff) } else { colors::text() })
                                            .child(project_name),
                                    )
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
    Runner,
}

struct RightPanel {
    pub commit_panel: Entity<CommitPanel>,
    file_explorer: Entity<FileExplorerPanel>,
    git_changes: Entity<GitChangesPanel>,
    git_log: Entity<GitLogPanel>,
    active_tab: RightTab,
    runner_terminal: Option<Entity<TerminalView>>,
    pub log_expanded: bool,
    pub log_height: f32,
    dragging_log: bool,
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
            dragging_log: false,
            drag_start_y: 0.,
            drag_start_height: 0.,
        }
    }

    fn set_runner(&mut self, terminal: Entity<TerminalView>) {
        self.runner_terminal = Some(terminal);
    }

    fn clear_runner(&mut self) {
        self.runner_terminal = None;
        if matches!(self.active_tab, RightTab::Runner) {
            self.active_tab = RightTab::Changes;
        }
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

        let is_changes = matches!(self.active_tab, RightTab::Changes);
        let is_files = matches!(self.active_tab, RightTab::Files);
        let is_runner = matches!(self.active_tab, RightTab::Runner);

        let is_dragging_log = self.dragging_log;
        let log_height = self.log_height;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle_bg())
            .when(is_dragging_log, |d| {
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
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                if this.dragging_log {
                    this.dragging_log = false;
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
                    .bg(colors::mantle())
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
                    )
                    // Runner tab
                    .when(has_runner, |d: Div| {
                        d.child(
                            div()
                                .id("tab-runner")
                                .flex_1()
                                .h_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_sm()
                                .when(is_runner, |d: Stateful<Div>| {
                                    d.text_color(colors::text())
                                        .border_b_2()
                                        .border_color(colors::blue())
                                })
                                .when(!is_runner, |d: Stateful<Div>| {
                                    d.text_color(colors::subtext())
                                        .hover(|d| d.text_color(colors::text()))
                                })
                                .child("Runner")
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.active_tab = RightTab::Runner;
                                    cx.notify();
                                })),
                        )
                    }),
            )
            // ── Content area ─────────────────────────────────
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .when(is_changes, |d: Div| d.child(self.git_changes.clone()))
                    .when(is_files, |d: Div| d.child(self.file_explorer.clone()))
                    .when_some(
                        if is_runner { self.runner_terminal.clone() } else { None },
                        |d: Div, terminal| d.child(terminal),
                    ),
            )
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
                            .bg(colors::mantle())
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
}

// ── AppView ──────────────────────────────────────────────────

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
    _project_subscription: Subscription,
    _workspace_subscription: Subscription,
    _update_task: Task<()>,
}

impl AppView {
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
        settings::save(name, self.wallpaper_path.as_deref());
        self.notify_all(cx);
    }

    fn apply_wallpaper(&mut self, path: Option<String>, cx: &mut Context<Self>) {
        theme::set_wallpaper(path.clone());
        self.wallpaper_path = path;
        settings::save(theme::current_name(), self.wallpaper_path.as_deref());
        self.notify_all(cx);
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
                            pane.add_tab(title, "> ", detail, AnyView::from(terminal_view.clone()), true)
                        });
                        // Subscribe to OSC title changes
                        let pane_entity = this.project_states[idx].pane.clone();
                        cx.subscribe(&terminal_view, move |_this: &mut AppView, _tv, event: &TerminalViewEvent, cx| {
                            match event {
                                TerminalViewEvent::TitleChanged(new_title) => {
                                    pane_entity.update(cx, |pane, cx| {
                                        pane.set_tab_title(tab_id, new_title.clone());
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
        let term_title = format!("zsh {}", self.terminal_count);
        let terminal_view = cx.new(|cx| TerminalView::new_in(term_title.clone(), Some(project_path), cx));
        let tab_id = pane.update(cx, |p, _cx| {
            p.add_tab(term_title, "> ", detail, AnyView::from(terminal_view.clone()), true)
        });
        // Subscribe to OSC title changes
        let pane_for_title = pane.clone();
        cx.subscribe(&terminal_view, move |_this: &mut AppView, _tv, event: &TerminalViewEvent, cx| {
            match event {
                TerminalViewEvent::TitleChanged(new_title) => {
                    pane_for_title.update(cx, |pane, cx| {
                        pane.set_tab_title(tab_id, new_title.clone());
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

        let state = ProjectState {
            path,
            pane,
            right_panel,
            runner_terminal: None,
            _pane_sub: pane_sub,
            _runner_sub: runner_sub,
            _right_panel_sub: right_panel_sub,
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        // Wallpaper layer
        if let Some(ref wp) = self.wallpaper_path {
            let path = std::path::PathBuf::from(wp);
            if path.exists() {
                base = base.child(
                    img(path)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                );
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
                            .w(px(420.))
                            .max_h(px(500.))
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
                                                            .map(|p| shorten_path(&PathBuf::from(p)))
                                                            .unwrap_or_else(|| "None".to_string())
                                                    ),
                                            ),
                                    )
                                    .when(self.wallpaper_path.is_some(), |d: Div| {
                                        d.child(
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

        base
    }
}

// ── Main ─────────────────────────────────────────────────────

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
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
                                        // Step 1: Download (0% -> 60%)
                                        this.update(cx, |view, cx| {
                                            view.update_status = Some((10, "Downloading update...".to_string()));
                                            cx.notify();
                                        }).ok();

                                        let info_clone = info.clone();
                                        let dl_result = cx.background_executor().spawn(async move {
                                            updater::update_step_download(&info_clone)
                                        }).await;

                                        if let Err(e) = dl_result {
                                            log::error!("Download failed: {}", e);
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

                                        // Step 2: Install (60% -> 90%)
                                        this.update(cx, |view, cx| {
                                            view.update_status = Some((60, "Installing update...".to_string()));
                                            cx.notify();
                                        }).ok();

                                        let install_result = cx.background_executor().spawn(async {
                                            updater::update_step_install()
                                        }).await;

                                        if let Err(e) = install_result {
                                            log::error!("Install failed: {}", e);
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

                                        // Step 3: Restart (100%)
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

                                    let root = this.project_states[idx].path.clone();
                                    let ws = this.workspace.clone();
                                    let cp = commit_panel.clone();
                                    cx.spawn(async move |_this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                        let result = cx.background_executor().spawn(async move {
                                            ide_git_panel::operations::one_button_commit_and_push(&root)
                                        }).await;
                                        let _ = result;

                                        cp.update(cx, |panel, cx| {
                                            panel.is_pushing = false;
                                            cx.notify();
                                        }).ok();
                                        ws.update(cx, |ws, cx| {
                                            ws.is_pushing = false;
                                            cx.notify();
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
