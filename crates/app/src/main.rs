use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use ide_agent::{AgentManager, AgentToolbar};
use ide_file_explorer::FileExplorerPanel;
use ide_git_panel::{CommitPanel, GitChangesPanel};
use ide_terminal::TerminalView;
use ide_workspace::{Dock, DockPosition, IdeWorkspace, Pane, PaneEvent};

/// Project entry in the left sidebar
struct ProjectPanel {
    projects: Vec<ProjectEntry>,
    active_project: usize,
}

struct ProjectEntry {
    name: String,
    path: PathBuf,
}

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
    pub fn overlay() -> Rgba { rgb(0x6c7086) }
    pub fn lavender() -> Rgba { rgb(0xb4befe) }
}

impl ProjectPanel {
    fn new() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let name = cwd
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Project".to_string());

        Self {
            projects: vec![ProjectEntry {
                name,
                path: cwd,
            }],
            active_project: 0,
        }
    }

    fn active_path(&self) -> &PathBuf {
        &self.projects[self.active_project].path
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
            // Header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px(px(8.))
                    .py(px(8.))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(colors::blue())
                            .child("CLAUDE IDE"),
                    ),
            )
            // Section title
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .text_color(colors::subtext())
                    .child("PROJECTS"),
            )
            // Project list
            .children(self.projects.iter().enumerate().map(|(idx, project)| {
                let is_active = idx == active;
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
                        this.active_project = idx;
                        cx.notify();
                    }))
            }))
            // Add project button
            .child(
                div()
                    .mt_auto()
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
                            .child("+ Add Project"),
                    ),
            )
    }
}

/// Right sidebar: combines commit panel (top) and file/changes panel (bottom)
struct RightPanel {
    commit_panel: Entity<CommitPanel>,
    file_explorer: Entity<FileExplorerPanel>,
    git_changes: Entity<GitChangesPanel>,
    show_files: bool, // true = files, false = git changes
}

impl RightPanel {
    fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let commit_panel = cx.new(|_cx| CommitPanel::new(root_path.clone()));
        let file_explorer = cx.new(|_cx| FileExplorerPanel::new(root_path.clone()));
        let git_changes = cx.new(|_cx| GitChangesPanel::new(root_path));

        Self {
            commit_panel,
            file_explorer,
            git_changes,
            show_files: true,
        }
    }
}

impl Render for RightPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            // Top: Commit panel
            .child(self.commit_panel.clone())
            // Separator
            .child(
                div()
                    .w_full()
                    .h(px(1.))
                    .bg(colors::surface1()),
            )
            // Toggle bar: Files | Changes
            .child(
                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .h(px(28.))
                    .bg(colors::mantle())
                    .border_b_1()
                    .border_color(colors::surface1())
                    .child(
                        div()
                            .id("tab-files")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xs()
                            .when(self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::text())
                                    .border_b_2()
                                    .border_color(colors::blue())
                            })
                            .when(!self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::subtext())
                                    .hover(|d| d.text_color(colors::text()))
                            })
                            .child("Files")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.show_files = true;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("tab-changes")
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_xs()
                            .when(!self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::text())
                                    .border_b_2()
                                    .border_color(colors::blue())
                            })
                            .when(self.show_files, |d: Stateful<Div>| {
                                d.text_color(colors::subtext())
                                    .hover(|d| d.text_color(colors::text()))
                            })
                            .child("Changes")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.show_files = false;
                                cx.notify();
                            })),
                    ),
            )
            // Bottom: File explorer or Git changes
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .when(self.show_files, |d: Div| {
                        d.child(self.file_explorer.clone())
                    })
                    .when(!self.show_files, |d: Div| {
                        d.child(self.git_changes.clone())
                    }),
            )
    }
}

/// Root application view
struct AppView {
    workspace: Entity<IdeWorkspace>,
    terminal_count: usize,
    _pane_subscription: Subscription,
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(self.workspace.clone())
    }
}

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1400.), px(900.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Claude IDE".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_window: &mut Window, cx: &mut App| {
                let root_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));

                // Create workspace entity (IdeWorkspace::new needs Context<IdeWorkspace>)
                let workspace = cx.new(|cx| IdeWorkspace::new(cx));

                // Setup left dock: Project panel
                let project_panel = cx.new(|_cx| ProjectPanel::new());
                workspace.update(cx, |ws, cx| {
                    ws.left_dock.update(cx, |dock, _cx| {
                        dock.add_panel(
                            "Projects".to_string(),
                            "> ",
                            AnyView::from(project_panel),
                        );
                    });
                });

                // Setup center pane: Default terminal tab
                let terminal_view = cx.new(|cx| TerminalView::new("Terminal".to_string(), cx));
                workspace.update(cx, |ws, cx| {
                    ws.center_pane.update(cx, |pane, _cx| {
                        pane.add_tab(
                            "Terminal".to_string(),
                            "> ",
                            AnyView::from(terminal_view),
                            true,
                        );
                    });
                });

                // Setup right dock: combined commit + files/changes
                let right_panel = cx.new(|cx| RightPanel::new(root_path.clone(), cx));
                workspace.update(cx, |ws, cx| {
                    ws.right_dock.update(cx, |dock, _cx| {
                        dock.add_panel(
                            "Git & Files".to_string(),
                            "* ",
                            AnyView::from(right_panel),
                        );
                    });
                });

                // Setup status bar
                let branch = ide_git_panel::get_branch_name(&root_path);
                workspace.update(cx, |ws, cx| {
                    ws.status_bar.update(cx, |bar, _cx| {
                        bar.set_branch(branch);
                        bar.set_status("Ready".to_string());
                    });
                });

                // Subscribe to pane "+" button events
                let center_pane = workspace.read(cx).center_pane.clone();
                cx.new(|cx| {
                    let pane_sub = cx.subscribe(&center_pane, |this: &mut AppView, _pane, event: &PaneEvent, cx| {
                        match event {
                            PaneEvent::NewTabRequested => {
                                this.terminal_count += 1;
                                let title = format!("Terminal {}", this.terminal_count);
                                let terminal_view = cx.new(|cx| TerminalView::new(title.clone(), cx));
                                this.workspace.update(cx, |ws, cx| {
                                    ws.center_pane.update(cx, |pane, _cx| {
                                        pane.add_tab(
                                            title,
                                            "> ",
                                            AnyView::from(terminal_view),
                                            true,
                                        );
                                    });
                                });
                                cx.notify();
                            }
                        }
                    });

                    AppView {
                        workspace,
                        terminal_count: 1,
                        _pane_subscription: pane_sub,
                    }
                })
            },
        )
        .unwrap();
    });
}
