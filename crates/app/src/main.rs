mod updater;

use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

use ide_file_explorer::FileExplorerPanel;
use ide_git_panel::{CommitPanel, GitChangesPanel, RunnerEvent};
use ide_terminal::TerminalView;
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
}

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn mantle() -> Rgba { rgb(0x181825) }
    pub fn surface0() -> Rgba { rgb(0x313244) }
    pub fn surface1() -> Rgba { rgb(0x45475a) }
    pub fn text() -> Rgba { rgb(0xcdd6f4) }
    pub fn subtext() -> Rgba { rgb(0xa6adc8) }
    pub fn blue() -> Rgba { rgb(0x89b4fa) }
    pub fn overlay() -> Rgba { rgb(0x6c7086) }
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
            // Add project button (top)
            .child(
                div()
                    .p(px(8.))
                    .child(
                        div()
                            .id("add-project")
                            .flex()
                            .items_center()
                            .justify_center()
                            .w_full()
                            .h(px(28.))
                            .bg(colors::surface0())
                            .rounded(px(4.))
                            .cursor_pointer()
                            .text_color(colors::subtext())
                            .hover(|d| d.bg(colors::surface1()).text_color(colors::text()))
                            .child("+ Add Project")
                            .on_click(cx.listener(|_this, _ev, _window, cx| {
                                cx.emit(ProjectPanelEvent::AddProjectRequested);
                            })),
                    ),
            )
            // Project list (below button)
            .children(self.projects.iter().enumerate().map(|(idx, project)| {
                let is_active = active == Some(idx);
                let path = project.path.clone();
                div()
                    .id(ElementId::Name(format!("project-{}", idx).into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(28.))
                    .px(px(12.))
                    .gap(px(6.))
                    .cursor_pointer()
                    .when(is_active, |d: Stateful<Div>| d.bg(colors::surface0()).border_l_2().border_color(colors::blue()))
                    .hover(|d| d.bg(colors::surface0()))
                    .child(
                        div()
                            .text_color(if is_active { colors::blue() } else { colors::overlay() })
                            .child("> "),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(if is_active { colors::text() } else { colors::subtext() })
                            .child(project.name.clone()),
                    )
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        this.active_project = Some(idx);
                        cx.emit(ProjectPanelEvent::ProjectSelected(idx, path.clone()));
                        cx.notify();
                    }))
            }))
    }
}

// ── Right Panel (git + files per project) ────────────────────

struct RightPanel {
    pub commit_panel: Entity<CommitPanel>,
    file_explorer: Entity<FileExplorerPanel>,
    git_changes: Entity<GitChangesPanel>,
    show_files: bool,
}

impl RightPanel {
    fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let commit_panel = cx.new(|_cx| CommitPanel::new(root_path.clone()));
        let file_explorer = cx.new(|cx| FileExplorerPanel::new(root_path.clone(), cx));
        let git_changes = cx.new(|cx| GitChangesPanel::new(root_path, cx));

        Self {
            commit_panel,
            file_explorer,
            git_changes,
            show_files: false,
        }
    }
}

impl Render for RightPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let change_count = self.git_changes.read(cx).change_count();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .child(self.commit_panel.clone())
            .child(div().w_full().h(px(1.)).flex_shrink_0().bg(colors::surface1()))
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
    }
}

// ── Per-project state ────────────────────────────────────────

struct ProjectState {
    path: PathBuf,
    pane: Entity<Pane>,
    right_panel: Entity<RightPanel>,
    runner_terminal: Option<Entity<TerminalView>>,
    runner_tab_id: Option<usize>,
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
    _project_subscription: Subscription,
    _workspace_subscription: Subscription,
    _update_task: Task<()>,
}

impl AppView {
    fn add_project(&mut self, path: PathBuf, cx: &mut Context<Self>) {
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
                    let title = format!("Terminal {}", this.terminal_count);
                    if let Some(idx) = this.active_project {
                        let project_path = this.project_states[idx].path.clone();
                        let terminal_view = cx.new(|cx| TerminalView::new_in(title.clone(), Some(project_path), cx));
                        this.project_states[idx].pane.update(cx, |pane, _cx| {
                            pane.add_tab(title, "> ", AnyView::from(terminal_view), true);
                        });
                    }
                    cx.notify();
                }
            }
        });

        // Open a terminal in this project directory
        self.terminal_count += 1;
        let project_path = path.clone();
        let terminal_view = cx.new(|cx| TerminalView::new_in(name.clone(), Some(project_path), cx));
        pane.update(cx, |p, _cx| {
            p.add_tab(name.clone(), "> ", AnyView::from(terminal_view), true);
        });

        // Subscribe to runner events from this project's commit panel
        let project_idx = self.project_states.len();
        let commit_panel_entity = right_panel.read(cx).commit_panel.clone();
        let runner_sub = cx.subscribe(&commit_panel_entity, move |this: &mut AppView, _panel, event: &RunnerEvent, cx| {
            match event {
                RunnerEvent::Start(cmd) => {
                    let state = &mut this.project_states[project_idx];
                    // Close existing runner tab if any
                    if let Some(tab_id) = state.runner_tab_id.take() {
                        state.pane.update(cx, |pane, _cx| pane.close_tab(tab_id));
                        state.runner_terminal = None;
                    }
                    // Create runner terminal tab
                    this.terminal_count += 1;
                    let project_path = state.path.clone();
                    let tv = cx.new(|cx| TerminalView::new_in("Runner".to_string(), Some(project_path), cx));
                    let tab_id = state.pane.update(cx, |pane, _cx| {
                        pane.add_tab("Runner".to_string(), "> ", AnyView::from(tv.clone()), true)
                    });
                    state.runner_tab_id = Some(tab_id);
                    state.runner_terminal = Some(tv.clone());
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
            runner_tab_id: None,
            _pane_sub: pane_sub,
            _runner_sub: runner_sub,
        };

        self.project_states.push(state);
        let idx = self.project_states.len() - 1;

        // Add to project panel
        let panel_path = self.project_states[idx].path.clone();
        self.project_panel.update(cx, |panel, cx| {
            panel.projects.push(ProjectEntry { name, path: panel_path });
            panel.active_project = Some(idx);
            cx.notify();
        });

        // Switch to this project
        self.switch_project(idx, cx);
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
        div()
            .size_full()
            .child(self.workspace.clone())
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
                                if let Some(info) = &this.update_info {
                                    let info = info.clone();
                                    cx.spawn(async |_this: WeakEntity<AppView>, cx: &mut AsyncApp| {
                                        let result = cx.background_executor().spawn(async move {
                                            updater::download_and_install(&info)
                                        }).await;
                                        match result {
                                            Ok(()) => updater::relaunch(),
                                            Err(e) => {
                                                log::error!("Auto-update failed: {}. Retrying with redownload...", e);
                                                // Retry once more
                                                let info2 = updater::check_for_update();
                                                if let Some(info2) = info2 {
                                                    match updater::download_and_install(&info2) {
                                                        Ok(()) => updater::relaunch(),
                                                        Err(e2) => {
                                                            log::error!("Retry failed: {}. Opening releases page.", e2);
                                                            let _ = std::process::Command::new("open")
                                                                .arg("https://github.com/melvin-viougea/forge/releases/latest")
                                                                .spawn();
                                                        }
                                                    }
                                                }
                                            }
                                        }
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

                    AppView {
                        workspace,
                        project_panel,
                        project_states: Vec::new(),
                        active_project: None,
                        terminal_count: 0,
                        update_info: None,
                        _project_subscription: project_sub,
                        _workspace_subscription: workspace_sub,
                        _update_task: update_task,
                    }
                })
            },
        )
        .unwrap();
    });
}
