mod updater;
mod session;

use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

use ide_file_explorer::FileExplorerPanel;
use ide_git_panel::{CommitPanel, GitChangesPanel, GitLogPanel, RunnerEvent};
use ide_terminal::{TerminalView, TerminalViewEvent};
use ide_workspace::{IdeWorkspace, Pane, PaneEvent, WorkspaceEvent};

// ── Project Panel (left sidebar) ─────────────────────────────

struct ProjectPanel {
    projects: Vec<ProjectEntry>,
    active_project: Option<usize>,
}

impl EventEmitter<ProjectPanelEvent> for ProjectPanel {}

enum ProjectPanelEvent {
    AddProjectRequested,
    ProjectSelected(usize, PathBuf),
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

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn mantle() -> Rgba { rgb(0x0d1117) }
    pub fn surface0() -> Rgba { rgb(0x161b22) }
    pub fn surface1() -> Rgba { rgb(0x21262d) }
    pub fn text() -> Rgba { rgb(0xc9d1d9) }
    pub fn subtext() -> Rgba { rgb(0x8b949e) }
    pub fn blue() -> Rgba { rgb(0x58a6ff) }
    pub fn overlay() -> Rgba { rgb(0x484f58) }
}

impl ProjectPanel {
    fn new() -> Self {
        Self {
            projects: Vec::new(),
            active_project: None,
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
            .bg(colors::mantle())
            .text_xs()
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
                            .text_xs()
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
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p(px(6.))
                    .gap(px(2.))
                    .children(self.projects.iter().enumerate().map(|(idx, project)| {
                        let is_active = active == Some(idx);
                        let path = project.path.clone();
                        div()
                            .id(ElementId::Name(format!("project-{}", idx).into()))
                            .flex()
                            .flex_col()
                            .w_full()
                            .flex()
                            .flex_col()
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
                            // Project name
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(if is_active { gpui::rgb(0xffffff) } else { colors::text() })
                                    .child(project.name.clone()),
                            )
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.active_project = Some(idx);
                                cx.emit(ProjectPanelEvent::ProjectSelected(idx, path.clone()));
                                cx.notify();
                            }))
                    })),
            )
    }
}

// ── Right Panel (git + files per project) ────────────────────

struct RightPanel {
    pub commit_panel: Entity<CommitPanel>,
    file_explorer: Entity<FileExplorerPanel>,
    git_changes: Entity<GitChangesPanel>,
    git_log: Entity<GitLogPanel>,
    show_files: bool,
    runner_terminal: Option<Entity<TerminalView>>,
    runner_expanded: bool,
    log_expanded: bool,
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
            show_files: false,
            runner_terminal: None,
            runner_expanded: true,
            log_expanded: false,
        }
    }

    fn set_runner(&mut self, terminal: Entity<TerminalView>) {
        self.runner_terminal = Some(terminal);
        self.runner_expanded = true;
    }

    fn clear_runner(&mut self) {
        self.runner_terminal = None;
    }
}

impl Render for RightPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let change_count = self.git_changes.read(cx).change_count();
        let has_runner = self.runner_terminal.is_some();
        let runner_expanded = self.runner_expanded;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .child(self.commit_panel.clone())
            .child(div().w_full().h(px(1.)).flex_shrink_0().bg(colors::surface1()))
            // ── Runner section (collapsible) ──────────────────
            .when(has_runner, |d: Div| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .w_full()
                        .when(runner_expanded, |d: Div| d.flex_1().min_h(px(80.)))
                        .child(
                            // Header bar
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
                                .bg(colors::mantle())
                                .border_b_1()
                                .border_color(colors::surface1())
                                .cursor_pointer()
                                .hover(|d| d.bg(colors::surface0()))
                                .text_xs()
                                .text_color(colors::subtext())
                                .child(if runner_expanded { "▼ " } else { "▶ " })
                                .child("Runner")
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.runner_expanded = !this.runner_expanded;
                                    cx.notify();
                                })),
                        )
                        .when_some(
                            if runner_expanded { self.runner_terminal.clone() } else { None },
                            |d: Div, terminal| {
                                d.child(
                                    div()
                                        .flex_1()
                                        .w_full()
                                        .overflow_hidden()
                                        .child(terminal),
                                )
                            },
                        ),
                )
                .child(div().w_full().h(px(1.)).flex_shrink_0().bg(colors::surface1()))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .h(px(28.))
                    .min_h(px(28.))
                    .flex_shrink_0()
                    .bg(colors::mantle())
                    .border_b_1()
                    .border_color(colors::surface1())
                    .child(
                        div()
                            .id("tab-changes")
                            .w(px(140.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xs()
                            .when(!self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::text()).border_b_2().border_color(colors::blue())
                            })
                            .when(self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::subtext()).hover(|d| d.text_color(colors::text()))
                            })
                            .child(format!("Changes ({})", change_count))
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.show_files = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("tab-files")
                            .w(px(140.))
                            .h_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xs()
                            .when(self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::text()).border_b_2().border_color(colors::blue())
                            })
                            .when(!self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::subtext()).hover(|d| d.text_color(colors::text()))
                            })
                            .child("Files")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.show_files = true;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .when(!self.show_files, |d: Div| d.child(self.git_changes.clone()))
                    .when(self.show_files, |d: Div| d.child(self.file_explorer.clone())),
            )
            // ── Git Log section (collapsible, bottom) ────────────
            .child(div().w_full().h(px(1.)).flex_shrink_0().bg(colors::surface1()))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .when(self.log_expanded, |d: Div| d.flex_1().min_h(px(80.)))
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
                            .text_xs()
                            .text_color(colors::subtext())
                            .child(if self.log_expanded { "▼ " } else { "▲ " })
                            .child("Git Log")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.log_expanded = !this.log_expanded;
                                cx.notify();
                            })),
                    )
                    .when(self.log_expanded, |d: Div| {
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
    _project_subscription: Subscription,
    _workspace_subscription: Subscription,
    _update_task: Task<()>,
}

impl AppView {
    fn save_session(&self) {
        let paths: Vec<PathBuf> = self.project_states.iter().map(|s| s.path.clone()).collect();
        let active = self.active_project.unwrap_or(0);
        session::save(&paths, active);
    }
}

impl AppView {
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
                    let state = &this.project_states[project_idx];
                    // Send Ctrl+C to stop the running process
                    if let Some(ref terminal) = state.runner_terminal {
                        terminal.update(cx, |view, _cx| {
                            view.terminal.write_input(&[3]); // Ctrl+C
                        });
                    }
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
        };

        self.project_states.push(state);
        let idx = self.project_states.len() - 1;

        // Add to project panel
        let panel_path = self.project_states[idx].path.clone();
        let display_path = shorten_path(&panel_path);
        self.project_panel.update(cx, |panel, cx| {
            panel.projects.push(ProjectEntry { name, path: panel_path, display_path });
            panel.active_project = Some(idx);
            cx.notify();
        });

        // Switch to this project
        self.switch_project(idx, cx);
        self.save_session();
    }

    fn switch_project(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx >= self.project_states.len() {
            return;
        }
        self.active_project = Some(idx);

        let state = &self.project_states[idx];
        let pane = state.pane.clone();
        let right_panel = state.right_panel.clone();

        self.workspace.update(cx, |ws, cx| {
            ws.center_pane = pane;
            ws.right_dock.update(cx, |dock, _cx| {
                dock.set_view(AnyView::from(right_panel));
            });
            cx.notify();
        });

        cx.notify();
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let base = div().size_full().child(self.workspace.clone());

        if let Some((percent, message)) = &self.update_status {
            let pct = *percent;
            let msg = message.clone();
            base.child(
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
                                    .text_color(rgb(0xc9d1d9))
                                    .text_size(px(16.))
                                    .child(msg),
                            )
                            .child(
                                // Progress bar container
                                div()
                                    .w(px(300.))
                                    .h(px(6.))
                                    .rounded(px(3.))
                                    .bg(rgb(0x21262d))
                                    .child(
                                        // Progress bar fill
                                        div()
                                            .h_full()
                                            .rounded(px(3.))
                                            .bg(rgb(0x58a6ff))
                                            .w(px(300. * pct as f32 / 100.)),
                                    ),
                            )
                            .child(
                                div()
                                    .text_color(rgb(0x8b949e))
                                    .text_size(px(13.))
                                    .child(format!("{}%", pct)),
                            ),
                    ),
            )
        } else {
            base
        }
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
                                // TODO: open settings panel
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

                    let mut app_view = AppView {
                        workspace,
                        project_panel,
                        project_states: Vec::new(),
                        active_project: None,
                        terminal_count: 0,
                        update_info: None,
                        update_status: None,
                        _project_subscription: project_sub,
                        _workspace_subscription: workspace_sub,
                        _update_task: update_task,
                    };

                    // Restore previous session
                    if let Some(saved) = session::load() {
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
                    }

                    app_view
                })
            },
        )
        .unwrap();
    });
}
