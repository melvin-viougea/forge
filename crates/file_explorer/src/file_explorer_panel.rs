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
    context_menu: Option<ContextMenuState>,
    _poll_task: Task<()>,
}

struct ContextMenuState {
    position: Point<Pixels>,
    target_idx: usize,
}

/// Flattened entry for rendering
struct FlatEntry {
    entry: FileEntry,
    children: Vec<usize>,
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
    pub fn green() -> Rgba { rgb(0xa6e3a1) }
    pub fn red() -> Rgba { rgb(0xf38ba8) }
    pub fn yellow() -> Rgba { rgb(0xf9e2af) }
    pub fn overlay() -> Rgba { rgb(0x6c7086) }
}

impl FileExplorerPanel {
    pub fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let tree = build_file_tree(&root_path, 0, 0);
        let git_statuses = get_git_statuses(&root_path);
        let entries = tree
            .into_iter()
            .map(|mut entry| {
                if let Some(status) = git_statuses.get(&entry.path) {
                    entry.git_status = status.clone();
                }
                FlatEntry { entry, children: Vec::new() }
            })
            .collect();

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(std::time::Duration::from_secs(2)).await;
                let result = this.update(cx, |view, cx| {
                    let git_statuses = get_git_statuses(&view.root_path);
                    let mut changed = false;
                    for flat in &mut view.entries {
                        let new_status = git_statuses
                            .get(&flat.entry.path)
                            .cloned()
                            .unwrap_or(GitFileStatus::Clean);
                        if flat.entry.git_status != new_status {
                            flat.entry.git_status = new_status;
                            changed = true;
                        }
                    }
                    let current_tree = build_file_tree(&view.root_path, 0, 0);
                    if current_tree.len() != view.entries.iter().filter(|e| e.entry.depth == 0).count() {
                        view.refresh();
                        changed = true;
                    }
                    if changed {
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
            entries,
            selected_index: None,
            on_file_open: None,
            context_menu: None,
            _poll_task: poll_task,
        }
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
                FlatEntry { entry, children: Vec::new() }
            })
            .collect();
    }

    fn toggle_expand(&mut self, idx: usize) {
        if idx >= self.entries.len() || !self.entries[idx].entry.is_dir {
            return;
        }

        let was_expanded = self.entries[idx].entry.expanded;
        self.entries[idx].entry.expanded = !was_expanded;

        if !was_expanded {
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
                    FlatEntry { entry, children: Vec::new() }
                })
                .collect();
            let insert_at = idx + 1;
            for (i, child) in child_entries.into_iter().enumerate() {
                self.entries.insert(insert_at + i, child);
            }
        } else {
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

    fn execute_action(&mut self, action: &str, idx: usize) {
        let path = self.entries[idx].entry.path.clone();
        let is_dir = self.entries[idx].entry.is_dir;
        let parent = if is_dir {
            path.clone()
        } else {
            path.parent().unwrap_or(&self.root_path).to_path_buf()
        };

        match action {
            "new_file" => {
                let new_path = parent.join("untitled");
                let _ = std::fs::write(&new_path, "");
                self.refresh();
            }
            "new_folder" => {
                let new_path = parent.join("new_folder");
                let _ = std::fs::create_dir(&new_path);
                self.refresh();
            }
            "copy" => {
                // Store path in clipboard via shell
                let _ = std::process::Command::new("bash")
                    .args(["-c", &format!("echo -n '{}' | pbcopy", path.display())])
                    .output();
            }
            "cut" => {
                let _ = std::process::Command::new("bash")
                    .args(["-c", &format!("echo -n '{}' | pbcopy", path.display())])
                    .output();
            }
            "paste" => {
                let output = std::process::Command::new("pbpaste").output();
                if let Ok(out) = output {
                    let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let src_path = PathBuf::from(&src);
                    if src_path.exists() {
                        let dest_name = src_path.file_name().unwrap_or_default();
                        let dest = parent.join(dest_name);
                        if src_path.is_dir() {
                            let _ = std::process::Command::new("cp")
                                .args(["-r", &src, &dest.display().to_string()])
                                .output();
                        } else {
                            let _ = std::fs::copy(&src_path, &dest);
                        }
                        self.refresh();
                    }
                }
            }
            "rename" => {
                // For now, rename to a simple prompt-based approach isn't possible
                // without a text input. We'll use a basic suffix approach.
                // TODO: implement inline rename with text input
            }
            "trash" => {
                // Move to macOS trash
                let _ = std::process::Command::new("osascript")
                    .args([
                        "-e",
                        &format!(
                            "tell application \"Finder\" to delete POSIX file \"{}\"",
                            path.display()
                        ),
                    ])
                    .output();
                self.refresh();
            }
            "delete" => {
                if is_dir {
                    let _ = std::fs::remove_dir_all(&path);
                } else {
                    let _ = std::fs::remove_file(&path);
                }
                self.refresh();
            }
            _ => {}
        }
        self.context_menu = None;
    }
}

fn render_menu_item(
    id: &str,
    label: &str,
    action: &'static str,
    target_idx: usize,
    cx: &mut Context<FileExplorerPanel>,
    text_color: Rgba,
) -> Stateful<Div> {
    let label_owned = label.to_string();
    div()
        .id(ElementId::Name(id.to_string().into()))
        .flex()
        .items_center()
        .w_full()
        .h(px(26.))
        .px(px(12.))
        .cursor_pointer()
        .text_color(text_color)
        .hover(|d| d.bg(colors::surface1()))
        .child(label_owned)
        .on_click(cx.listener(move |this, _ev, _window, cx| {
            this.execute_action(action, target_idx);
            cx.notify();
        }))
}

impl Render for FileExplorerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let context_menu = self.context_menu.as_ref().map(|m| (m.position, m.target_idx));

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::mantle())
            .id("file-explorer-scroll")
            .overflow_y_scroll()
            .text_xs()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
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
                                this.context_menu = None;
                                this.selected_index = Some(idx);
                                if this.entries[idx].entry.is_dir {
                                    this.toggle_expand(idx);
                                }
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
                    }),
            )
            // Context menu overlay
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
                                    .child(render_menu_item("ctx-new-file", "New File", "new_file", target_idx, cx, colors::text()))
                                    .child(render_menu_item("ctx-new-folder", "New Folder", "new_folder", target_idx, cx, colors::text()))
                                    .child(div().w_full().h(px(1.)).my(px(4.)).bg(colors::surface1()))
                                    .child(render_menu_item("ctx-copy", "Copy", "copy", target_idx, cx, colors::text()))
                                    .child(render_menu_item("ctx-cut", "Cut", "cut", target_idx, cx, colors::text()))
                                    .child(render_menu_item("ctx-paste", "Paste", "paste", target_idx, cx, colors::text()))
                                    .child(div().w_full().h(px(1.)).my(px(4.)).bg(colors::surface1()))
                                    .child(render_menu_item("ctx-rename", "Rename", "rename", target_idx, cx, colors::text()))
                                    .child(div().w_full().h(px(1.)).my(px(4.)).bg(colors::surface1()))
                                    .child(render_menu_item("ctx-trash", "Move to Trash", "trash", target_idx, cx, colors::text()))
                                    .child(render_menu_item("ctx-delete", "Delete Permanently", "delete", target_idx, cx, colors::red())),
                            ),
                    ),
                )
            })
    }
}
