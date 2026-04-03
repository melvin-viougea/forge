use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::file_tree::{build_file_tree, get_git_statuses, FileEntry, GitFileStatus};

/// File explorer panel for the right dock
pub struct FileExplorerPanel {
    root_path: PathBuf,
    entries: Vec<FlatEntry>,
    selected_index: Option<usize>,
    pub on_file_open: Option<Box<dyn Fn(&PathBuf) + Send + Sync>>,
}

/// Flattened entry for rendering (tree flattened to list with indentation)
struct FlatEntry {
    entry: FileEntry,
    children: Vec<usize>, // indices of children in the flat list
}

// Colors inlined
mod colors {
    use gpui::rgb;
    use gpui::Rgba;

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
}

impl FileExplorerPanel {
    pub fn new(root_path: PathBuf) -> Self {
        let mut panel = Self {
            root_path: root_path.clone(),
            entries: Vec::new(),
            selected_index: None,
            on_file_open: None,
        };
        panel.refresh();
        panel
    }

    pub fn refresh(&mut self) {
        let tree = build_file_tree(&self.root_path, 0, 0);
        let git_statuses = get_git_statuses(&self.root_path);

        self.entries = tree
            .into_iter()
            .map(|mut entry| {
                if let Some(status) = git_statuses.get(&entry.path) {
                    entry.git_status = status.clone();
                }
                FlatEntry {
                    entry,
                    children: Vec::new(),
                }
            })
            .collect();
    }

    fn toggle_expand(&mut self, idx: usize) {
        if idx >= self.entries.len() {
            return;
        }

        let entry = &self.entries[idx].entry;
        if !entry.is_dir {
            return;
        }

        let was_expanded = entry.expanded;
        self.entries[idx].entry.expanded = !was_expanded;

        if !was_expanded {
            // Load children
            let path = self.entries[idx].entry.path.clone();
            let depth = self.entries[idx].entry.depth + 1;
            let children = build_file_tree(&path, depth, depth);
            let git_statuses = get_git_statuses(&self.root_path);

            let child_entries: Vec<FlatEntry> = children
                .into_iter()
                .map(|mut entry| {
                    if let Some(status) = git_statuses.get(&entry.path) {
                        entry.git_status = status.clone();
                    }
                    FlatEntry {
                        entry,
                        children: Vec::new(),
                    }
                })
                .collect();

            let insert_at = idx + 1;
            for (i, child) in child_entries.into_iter().enumerate() {
                self.entries.insert(insert_at + i, child);
            }
        } else {
            // Remove children (all entries with greater depth after this one)
            let depth = self.entries[idx].entry.depth;
            let mut remove_count = 0;
            for i in (idx + 1)..self.entries.len() {
                if self.entries[i].entry.depth > depth {
                    remove_count += 1;
                } else {
                    break;
                }
            }
            self.entries.drain((idx + 1)..(idx + 1 + remove_count));
        }
    }

    fn status_color(status: &GitFileStatus) -> Rgba {
        match status {
            GitFileStatus::Modified => colors::yellow(),
            GitFileStatus::Added | GitFileStatus::Untracked => colors::green(),
            GitFileStatus::Deleted => colors::red(),
            GitFileStatus::Renamed => colors::blue(),
            GitFileStatus::Clean => colors::text(),
        }
    }

    fn status_indicator(status: &GitFileStatus) -> &'static str {
        match status {
            GitFileStatus::Modified => " M",
            GitFileStatus::Added => " A",
            GitFileStatus::Deleted => " D",
            GitFileStatus::Renamed => " R",
            GitFileStatus::Untracked => " ?",
            GitFileStatus::Clean => "",
        }
    }
}

impl Render for FileExplorerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .id("file-explorer-scroll")
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
                    .child("FILES")
                    .child(
                        div()
                            .id("refresh-files")
                            .cursor_pointer()
                            .hover(|d| d.text_color(colors::text()))
                            .child("R")
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.refresh();
                                cx.notify();
                            })),
                    ),
            )
            // File list
            .children(
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(idx, flat)| {
                        let entry = &flat.entry;
                        let indent = entry.depth as f32 * 16.0;
                        let is_selected = self.selected_index == Some(idx);
                        let status_color = Self::status_color(&entry.git_status);
                        let status_text = Self::status_indicator(&entry.git_status);
                        let icon = entry.icon();
                        let name = entry.name.clone();

                        div()
                            .id(ElementId::Name(format!("file-{}", idx).into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(22.))
                            .pl(px(8. + indent))
                            .pr(px(8.))
                            .cursor_pointer()
                            .when(is_selected, |d: Stateful<Div>| d.bg(colors::surface0()))
                            .hover(|d| d.bg(colors::surface0()))
                            .child(
                                div()
                                    .text_color(colors::overlay())
                                    .child(icon),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .text_color(status_color)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_color(status_color)
                                    .child(status_text),
                            )
                            .on_click(cx.listener(move |this, _ev, _window, cx| {
                                this.selected_index = Some(idx);
                                if this.entries[idx].entry.is_dir {
                                    this.toggle_expand(idx);
                                }
                                cx.notify();
                            }))
                    }),
            )
    }
}
