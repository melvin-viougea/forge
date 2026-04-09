use gpui::*;
use gpui::prelude::*;
use std::path::PathBuf;

use crate::file_tree::{build_file_tree, get_git_statuses, FileEntry, GitFileStatus};

pub enum FileExplorerEvent {
    FileOpened(PathBuf),
    EditFile(PathBuf),
}

impl gpui::EventEmitter<FileExplorerEvent> for FileExplorerPanel {}

/// File explorer panel for the right dock
pub struct FileExplorerPanel {
    root_path: PathBuf,
    entries: Vec<FlatEntry>,
    selected_index: Option<usize>,
    pub on_file_open: Option<Box<dyn Fn(&PathBuf) + Send + Sync>>,
    context_menu: Option<ContextMenuState>,
    _poll_task: Task<()>,
    drop_target_idx: Option<usize>,
    focus_handle: FocusHandle,
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

use ide_workspace::theme as colors;

impl FileExplorerPanel {
    pub fn new(root_path: PathBuf, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
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
            drop_target_idx: None,
            focus_handle,
        }
    }

    pub fn refresh(&mut self) {
        // Save expanded directories
        let expanded: std::collections::HashSet<PathBuf> = self.entries
            .iter()
            .filter(|f| f.entry.is_dir && f.entry.expanded)
            .map(|f| f.entry.path.clone())
            .collect();

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

        // Re-expand previously expanded directories
        let mut idx = 0;
        while idx < self.entries.len() {
            if self.entries[idx].entry.is_dir && expanded.contains(&self.entries[idx].entry.path) {
                self.entries[idx].entry.expanded = true;
                let path = self.entries[idx].entry.path.clone();
                let depth = self.entries[idx].entry.depth + 1;
                let children = build_file_tree(&path, depth, depth);
                let child_entries: Vec<FlatEntry> = children
                    .into_iter()
                    .map(|mut entry| {
                        if let Some(status) = git_statuses.get(&entry.path) {
                            entry.git_status = status.clone();
                        }
                        FlatEntry { entry, children: Vec::new() }
                    })
                    .collect();
                for (i, child) in child_entries.into_iter().enumerate() {
                    self.entries.insert(idx + 1 + i, child);
                }
            }
            idx += 1;
        }
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
            GitFileStatus::Untracked => " A",
            GitFileStatus::Clean => "",
        }
    }

    fn execute_action(&mut self, action: &str, idx: usize, cx: &mut Context<Self>) {
        let path = self.entries[idx].entry.path.clone();
        let is_dir = self.entries[idx].entry.is_dir;
        let parent = if is_dir {
            path.clone()
        } else {
            path.parent().unwrap_or(&self.root_path).to_path_buf()
        };

        match action {
            "edit_md" => {
                let abs_path = self.root_path.join(&path);
                cx.emit(FileExplorerEvent::EditFile(abs_path));
            }
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
                let abs_parent = self.root_path.join(&parent);
                // Try reading file URLs from pasteboard (Finder copy)
                let file_paths = Self::get_clipboard_file_paths();
                if !file_paths.is_empty() {
                    for src_path in &file_paths {
                        if let Some(file_name) = src_path.file_name() {
                            let dest = abs_parent.join(file_name);
                            if dest == *src_path { continue; }
                            if src_path.is_dir() {
                                let _ = std::process::Command::new("cp")
                                    .args(["-r", &src_path.display().to_string(), &dest.display().to_string()])
                                    .output();
                            } else {
                                let _ = std::fs::copy(src_path, &dest);
                            }
                        }
                    }
                    self.refresh();
                } else {
                    // Fallback: try text path from pbpaste
                    if let Ok(out) = std::process::Command::new("pbpaste").output() {
                        let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        let src_path = PathBuf::from(&src);
                        if src_path.exists() {
                            if let Some(dest_name) = src_path.file_name() {
                                let dest = abs_parent.join(dest_name);
                                if dest != src_path {
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

    /// Read file paths from macOS pasteboard (Finder Cmd+C) or save clipboard image data.
    /// Returns a list of file paths ready to be copied to the destination.
    fn get_clipboard_file_paths() -> Vec<PathBuf> {
        // Use JXA to inspect pasteboard and extract file URLs or save image data
        let script = r#"
ObjC.import("AppKit");
ObjC.import("Foundation");
var pb = $.NSPasteboard.generalPasteboard;
var items = pb.pasteboardItems;
var result = [];

for (var i = 0; i < items.count; i++) {
    var item = items.objectAtIndex(i);
    // Try file URL first (Finder copy)
    var urlStr = item.stringForType("public.file-url");
    if (urlStr) {
        var url = $.NSURL.URLWithString(urlStr);
        if (url && url.isFileURL) {
            result.push(url.path.js);
            continue;
        }
    }
    // Try image data — save to temp file
    var imageTypes = ["public.png", "public.tiff", "public.jpeg"];
    var exts = ["png", "tiff", "jpg"];
    for (var j = 0; j < imageTypes.length; j++) {
        var data = item.dataForType(imageTypes[j]);
        if (data && data.length > 0) {
            var tmpPath = "/tmp/forge-paste-" + Date.now() + "." + exts[j];
            var nsPath = $.NSString.stringWithString(tmpPath);
            data.writeToFileAtomically(nsPath, true);
            result.push(tmpPath);
            break;
        }
    }
}
result.join("\n");
"#;
        let output = std::process::Command::new("osascript")
            .args(["-l", "JavaScript", "-e", script])
            .output();
        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                text.trim()
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| PathBuf::from(l.trim()))
                    .filter(|p| p.exists())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    /// Paste clipboard files to project root
    fn paste_to_root(&mut self) {
        let dest_dir = self.root_path.clone();
        let file_paths = Self::get_clipboard_file_paths();
        if !file_paths.is_empty() {
            for src_path in &file_paths {
                if let Some(file_name) = src_path.file_name() {
                    let dest = dest_dir.join(file_name);
                    if dest == *src_path { continue; }
                    if src_path.is_dir() {
                        let _ = std::process::Command::new("cp")
                            .args(["-r", &src_path.display().to_string(), &dest.display().to_string()])
                            .output();
                    } else {
                        let _ = std::fs::copy(src_path, &dest);
                    }
                }
            }
            self.refresh();
        } else if let Ok(out) = std::process::Command::new("pbpaste").output() {
            let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let src_path = PathBuf::from(&src);
            if src_path.exists() {
                if let Some(dest_name) = src_path.file_name() {
                    let dest = dest_dir.join(dest_name);
                    if dest != src_path {
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
        }
    }

    /// Get the target directory for a drop at the given entry index
    fn drop_target_dir(&self, idx: usize) -> PathBuf {
        let entry = &self.entries[idx].entry;
        if entry.is_dir {
            self.root_path.join(&entry.path)
        } else {
            let parent = entry.path.parent().unwrap_or(&self.root_path);
            self.root_path.join(parent)
        }
    }

    /// Handle external file drop at the given entry index
    fn handle_file_drop(&mut self, idx: usize, paths: &ExternalPaths) {
        let dest_dir = self.drop_target_dir(idx);
        for src in paths.paths() {
            if let Some(file_name) = src.file_name() {
                let dest = dest_dir.join(file_name);
                if dest == *src { continue; }
                // Move (rename) if on same volume, otherwise copy
                if std::fs::rename(src, &dest).is_err() {
                    if src.is_dir() {
                        let _ = std::process::Command::new("cp")
                            .args(["-r", &src.display().to_string(), &dest.display().to_string()])
                            .output();
                    } else {
                        let _ = std::fs::copy(src, &dest);
                    }
                }
            }
        }
        self.refresh();
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
            this.execute_action(action, target_idx, cx);
            cx.notify();
        }))
}

impl Focusable for FileExplorerPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FileExplorerPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let context_menu = self.context_menu.as_ref().map(|m| (m.position, m.target_idx));

        div()
            .flex()
            .flex_col()
            .size_full()
            .id("file-explorer-scroll")
            .overflow_y_scroll()
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev: &MouseDownEvent, window, cx| {
                this.focus_handle.focus(window);
                this.selected_index = None;
                this.context_menu = None;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                if ev.keystroke.modifiers.platform {
                    match ev.keystroke.key.as_str() {
                        // Cmd+Backspace → move to trash
                        "backspace" => {
                            if let Some(idx) = this.selected_index {
                                if idx < this.entries.len() {
                                    this.execute_action("trash", idx, cx);
                                    this.selected_index = None;
                                    this.refresh();
                                    cx.notify();
                                }
                            }
                        }
                        // Cmd+V → paste
                        "v" => {
                            if let Some(idx) = this.selected_index {
                                if idx < this.entries.len() {
                                    this.execute_action("paste", idx, cx);
                                    cx.notify();
                                }
                            } else {
                                // No selection → paste at root
                                this.paste_to_root();
                                cx.notify();
                            }
                        }
                        _ => {}
                    }
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                // Drop on empty area → project root
                let root = this.root_path.clone();
                for src in paths.paths() {
                    if let Some(file_name) = src.file_name() {
                        let dest = root.join(file_name);
                        if dest == *src { continue; }
                        if std::fs::rename(src, &dest).is_err() {
                            if src.is_dir() {
                                let _ = std::process::Command::new("cp")
                                    .args(["-r", &src.display().to_string(), &dest.display().to_string()])
                                    .output();
                            } else {
                                let _ = std::fs::copy(src, &dest);
                            }
                        }
                    }
                }
                this.refresh();
                cx.notify();
            }))
            .drag_over::<ExternalPaths>(|d, _, _, _| {
                d.bg(colors::surface0())
            })
            // File list
            .children(
                self.entries
                    .iter()
                    .enumerate()
                    .map(|(idx, flat)| {
                        let entry = &flat.entry;
                        let indent = entry.depth as f32 * 16.0;
                        let is_selected = self.selected_index == Some(idx);
                        let is_drop_target = self.drop_target_idx == Some(idx);
                        let status_color = Self::status_color(&entry.git_status);
                        let status_text = Self::status_indicator(&entry.git_status);
                        let icon_svg = entry.icon_svg();
                        let name = entry.name.clone();

                        div()
                            .id(ElementId::Name(format!("file-{}", idx).into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.))
                            .w_full()
                            .h(px(22.))
                            .pl(px(8. + indent))
                            .pr(px(8.))
                            .cursor_pointer()
                            .when(is_selected, |d: Stateful<Div>| d.bg(colors::surface0()))
                            .when(is_drop_target, |d| d.bg(colors::surface1()).border_t_1().border_color(colors::blue()))
                            .hover(|d| d.bg(colors::surface0()))
                            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _window, cx| {
                                this.handle_file_drop(idx, paths);
                                this.drop_target_idx = None;
                                cx.notify();
                            }))
                            .drag_over::<ExternalPaths>(move |d, _, _, _| {
                                d.bg(colors::surface1()).border_t_1().border_color(colors::blue())
                            })
                            .child(
                                svg()
                                    .path(icon_svg)
                                    .size(px(14.))
                                    .flex_shrink_0()
                                    .text_color(colors::subtext()),
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
                            .on_click(cx.listener(move |this, _ev, window, cx| {
                                this.focus_handle.focus(window);
                                this.context_menu = None;
                                this.selected_index = Some(idx);
                                if this.entries[idx].entry.is_dir {
                                    this.toggle_expand(idx);
                                } else {
                                    let abs_path = this.root_path.join(&this.entries[idx].entry.path);
                                    cx.emit(FileExplorerEvent::FileOpened(abs_path));
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
            // Context menu with backdrop
            .when_some(context_menu, |d: Stateful<Div>, (pos, target_idx)| {
                let is_md = target_idx < self.entries.len()
                    && !self.entries[target_idx].entry.is_dir
                    && self.entries[target_idx].entry.name.ends_with(".md");
                d.child(
                    deferred(
                        div()
                            .id("ctx-backdrop")
                            .absolute()
                            .top_0()
                            .left_0()
                            .size_full()
                            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                this.context_menu = None;
                                cx.notify();
                            }))
                            .on_mouse_down(MouseButton::Right, cx.listener(|this, _ev: &MouseDownEvent, _window, cx| {
                                this.context_menu = None;
                                cx.notify();
                            }))
                            .child(
                                anchored()
                                    .position(pos)
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .w(px(180.))
                                            .bg(colors::surface0())
                                            .border_1()
                                            .border_color(colors::surface1())
                                            .rounded(px(6.))
                                            .py(px(4.))
                                            .text_sm()
                                            .shadow_lg()
                                            .on_mouse_down(MouseButton::Left, |_ev: &MouseDownEvent, _window, cx| {
                                                cx.stop_propagation();
                                            })
                                            .when(is_md, |d: Div| {
                                                d.child(render_menu_item("ctx-edit-md", "Edit File", "edit_md", target_idx, cx, colors::blue()))
                                                 .child(div().w_full().h(px(1.)).my(px(4.)).bg(colors::surface1()))
                                            })
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
                    ),
                )
            })
    }
}
