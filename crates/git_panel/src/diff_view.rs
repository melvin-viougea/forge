use gpui::prelude::*;
use gpui::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::status::{get_changes, ChangeStatus, GitFileChange};
use ide_workspace::theme as colors;
use ide_workspace::GitGutterMarker;

#[derive(Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

#[derive(Clone, PartialEq)]
pub enum DiffLineKind {
    Context,
    Addition,
    Deletion,
    Hunk,
}

// ── Side-by-side model ──────────────────────────────────────

#[derive(Clone)]
struct SbsRow {
    left_num: Option<u32>,
    left_text: String,
    left_kind: SbsKind,
    right_num: Option<u32>,
    right_text: String,
    right_kind: SbsKind,
}

#[derive(Clone, Copy, PartialEq)]
enum SbsKind {
    Context,
    Changed,
    Empty,
    Hunk,
}

fn to_side_by_side(lines: &[DiffLine]) -> Vec<SbsRow> {
    let mut rows = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        match lines[i].kind {
            DiffLineKind::Context => {
                rows.push(SbsRow {
                    left_num: lines[i].old_line,
                    left_text: lines[i].content.clone(),
                    left_kind: SbsKind::Context,
                    right_num: lines[i].new_line,
                    right_text: lines[i].content.clone(),
                    right_kind: SbsKind::Context,
                });
                i += 1;
            }
            DiffLineKind::Hunk => {
                rows.push(SbsRow {
                    left_num: None,
                    left_text: lines[i].content.clone(),
                    left_kind: SbsKind::Hunk,
                    right_num: None,
                    right_text: lines[i].content.clone(),
                    right_kind: SbsKind::Hunk,
                });
                i += 1;
            }
            DiffLineKind::Deletion | DiffLineKind::Addition => {
                let mut dels = Vec::new();
                let mut adds = Vec::new();
                while i < lines.len() && lines[i].kind == DiffLineKind::Deletion {
                    dels.push(&lines[i]);
                    i += 1;
                }
                while i < lines.len() && lines[i].kind == DiffLineKind::Addition {
                    adds.push(&lines[i]);
                    i += 1;
                }
                let max_len = dels.len().max(adds.len());
                for j in 0..max_len {
                    rows.push(SbsRow {
                        left_num: dels.get(j).and_then(|l| l.old_line),
                        left_text: dels.get(j).map(|l| l.content.clone()).unwrap_or_default(),
                        left_kind: if j < dels.len() {
                            SbsKind::Changed
                        } else {
                            SbsKind::Empty
                        },
                        right_num: adds.get(j).and_then(|l| l.new_line),
                        right_text: adds.get(j).map(|l| l.content.clone()).unwrap_or_default(),
                        right_kind: if j < adds.len() {
                            SbsKind::Changed
                        } else {
                            SbsKind::Empty
                        },
                    });
                }
            }
        }
    }
    rows
}

// ── Rendering ───────────────────────────────────────────────

const LINE_H: f32 = 20.0;

fn render_side_by_side(lines: &[DiffLine]) -> Vec<Div> {
    let base = colors::base_solid();
    let divider = colors::surface1_solid();
    let rows = to_side_by_side(lines);
    rows.into_iter()
        .map(|row| {
            if row.left_kind == SbsKind::Hunk {
                return div().w_full().h(px(1.)).bg(divider);
            }

            div()
                .flex()
                .flex_row()
                .w_full()
                .h(px(LINE_H))
                .child(render_pane_half(
                    row.left_kind,
                    row.left_num,
                    &row.left_text,
                    true,
                    base,
                ))
                .child(div().w(px(1.)).h_full().bg(divider))
                .child(render_pane_half(
                    row.right_kind,
                    row.right_num,
                    &row.right_text,
                    false,
                    base,
                ))
        })
        .collect()
}

fn render_pane_half(
    kind: SbsKind,
    line_num: Option<u32>,
    text: &str,
    is_left: bool,
    base: Rgba,
) -> Div {
    let bg = match kind {
        SbsKind::Changed if is_left => rgb(0x2a1518),
        SbsKind::Changed => rgb(0x152a18),
        SbsKind::Empty | SbsKind::Hunk | SbsKind::Context => base,
    };

    let num_color = match kind {
        SbsKind::Changed if is_left => colors::red(),
        SbsKind::Changed => colors::green(),
        _ => colors::overlay(),
    };

    let num_str = line_num.map(|n| format!("{}", n)).unwrap_or_default();
    let content = if text.is_empty() {
        " ".to_string()
    } else {
        text.replace(' ', "\u{00A0}")
    };

    let marker_color = match kind {
        SbsKind::Changed if is_left => Some(colors::red()),
        SbsKind::Changed => Some(colors::green()),
        _ => None,
    };

    div()
        .flex_1()
        .min_w(px(0.))
        .h(px(LINE_H))
        .bg(bg)
        .flex()
        .flex_row()
        .overflow_hidden()
        // Color marker (3px bar)
        .child(
            div()
                .w(px(3.))
                .h(px(LINE_H))
                .flex_shrink_0()
                .when_some(marker_color, |d: Div, c| d.bg(c)),
        )
        // Line number gutter (same pattern as file_view)
        .child(
            div()
                .w(px(45.))
                .flex_shrink_0()
                .text_right()
                .pr(px(12.))
                .text_color(num_color)
                .child(num_str),
        )
        // Content
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .pl(px(4.))
                .overflow_hidden()
                .text_color(colors::text())
                .child(content),
        )
}


// ── DiffView (side-by-side with virtualization) ─────────────

pub struct DiffView {
    pub file_path: Option<PathBuf>,
    root_path: PathBuf,
    additions: usize,
    deletions: usize,
    scroll_y: f32,
    rows: Vec<SbsRow>,
    scroll_handle: ScrollHandle,
}

impl DiffView {
    pub fn new_for_file(root_path: PathBuf, file_path: PathBuf, _cx: &mut Context<Self>) -> Self {
        let diff_lines = generate_head_to_workdir_diff(&root_path, &file_path);
        let additions = diff_lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Addition)
            .count();
        let deletions = diff_lines
            .iter()
            .filter(|l| l.kind == DiffLineKind::Deletion)
            .count();

        let mut rows = to_side_by_side(&diff_lines);
        // Remove leading hunk separator
        if rows.first().map(|r| r.left_kind == SbsKind::Hunk).unwrap_or(false) {
            rows.remove(0);
        }

        Self {
            file_path: Some(file_path),
            root_path,
            additions,
            deletions,
            scroll_y: 0.0,
            rows,
            scroll_handle: ScrollHandle::new(),
        }
    }
}


impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filename = self
            .file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "No file".to_string());

        let rel_path = self
            .file_path
            .as_ref()
            .and_then(|p| p.strip_prefix(&self.root_path).ok())
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let additions = self.additions;
        let deletions = self.deletions;

        // Compute row heights: hunk separators = 1px, normal rows = LINE_H
        let total_rows = self.rows.len();
        let hunk_h = 1.0_f32;

        // Compute content height
        let content_height: f32 = self.rows.iter()
            .map(|r| if r.left_kind == SbsKind::Hunk { hunk_h } else { LINE_H })
            .sum();

        let viewport_h: f32 = {
            let h: f32 = self.scroll_handle.bounds().size.height.into();
            if h > 0.0 { h } else { 600.0 }
        };
        let scroll_y = self.scroll_y;
        let needs_scroll = content_height > viewport_h;

        // Viewport virtualization: find visible row range
        let buffer = 20_usize;
        let (first, last) = {
            let mut y = 0.0_f32;
            let mut first_row = 0;
            let mut last_row = total_rows;
            for (i, row) in self.rows.iter().enumerate() {
                let h = if row.left_kind == SbsKind::Hunk { hunk_h } else { LINE_H };
                if y + h <= scroll_y {
                    first_row = i + 1;
                }
                if y > scroll_y + viewport_h {
                    last_row = i;
                    break;
                }
                y += h;
            }
            let f = first_row.saturating_sub(buffer);
            let l = (last_row + buffer).min(total_rows);
            (f, l)
        };

        // Spacer heights
        let top_spacer: f32 = self.rows[..first].iter()
            .map(|r| if r.left_kind == SbsKind::Hunk { hunk_h } else { LINE_H })
            .sum();
        let bottom_spacer: f32 = if last < total_rows {
            self.rows[last..].iter()
                .map(|r| if r.left_kind == SbsKind::Hunk { hunk_h } else { LINE_H })
                .sum()
        } else {
            0.0
        };

        // Render only visible rows
        let divider = colors::surface1_solid();
        let base = colors::base_solid();
        let visible_rows: Vec<AnyElement> = self.rows[first..last].iter().map(|row| {
            if row.left_kind == SbsKind::Hunk {
                return div().w_full().h(px(hunk_h)).bg(divider).into_any_element();
            }
            div()
                .flex()
                .flex_row()
                .w_full()
                .h(px(LINE_H))
                .child(render_pane_half(row.left_kind, row.left_num, &row.left_text, true, base))
                .child(div().w(px(1.)).h_full().bg(divider))
                .child(render_pane_half(row.right_kind, row.right_num, &row.right_text, false, base))
                .into_any_element()
        }).collect();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::base_solid())
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Title bar
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(30.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .bg(colors::mantle_solid())
                    .border_b_1()
                    .border_color(colors::surface1_solid())
                    .gap(px(8.))
                    .child(
                        div()
                            .text_color(colors::text())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(filename),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_xs()
                            .text_color(colors::overlay())
                            .truncate()
                            .child(rel_path),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .gap(px(6.))
                            .text_xs()
                            .child(div().text_color(colors::green()).child(format!("+{}", additions)))
                            .child(div().text_color(colors::red()).child(format!("-{}", deletions))),
                    ),
            )
            // Virtualized side-by-side diff
            .child(
                div()
                    .id("diff-scroll")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .track_scroll(&self.scroll_handle)
                    .on_scroll_wheel(cx.listener(
                        move |this, ev: &ScrollWheelEvent, _window, cx| {
                            let dy: f32 = match &ev.delta {
                                ScrollDelta::Lines(d) => d.y * 20.0,
                                ScrollDelta::Pixels(d) => d.y.into(),
                            };
                            let vh: f32 = {
                                let h: f32 = this.scroll_handle.bounds().size.height.into();
                                if h > 0.0 { h } else { 600.0 }
                            };
                            let ch: f32 = this.rows.iter()
                                .map(|r| if r.left_kind == SbsKind::Hunk { 1.0 } else { LINE_H })
                                .sum();
                            let ms = (ch - vh).max(0.0);
                            this.scroll_y = (this.scroll_y - dy).clamp(0.0, ms);
                            cx.notify();
                        },
                    ))
                    .child(
                        div()
                            .mt(px(-scroll_y))
                            .w_full()
                            .child(div().h(px(top_spacer)).flex_shrink_0())
                            .children(visible_rows)
                            .child(div().h(px(bottom_spacer)).flex_shrink_0())
                            .when(!needs_scroll, |d| {
                                d.child(div().w_full().flex_1().flex().pt(px(4.)).child(
                                    div().w_full().h(px(1.)).bg(colors::surface1_solid()),
                                ))
                            }),
                    ),
            )
    }
}

// ── Diff generation ─────────────────────────────────────────

fn generate_head_to_workdir_diff(root: &Path, file_path: &Path) -> Vec<DiffLine> {
    let mut diff_lines = Vec::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return diff_lines,
    };

    let rel_path = file_path.strip_prefix(root).unwrap_or(file_path);

    // For untracked files, show entire file as additions
    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            if let Some(p) = entry.path() {
                if PathBuf::from(p) == rel_path && entry.status().is_wt_new() {
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        diff_lines.push(DiffLine {
                            kind: DiffLineKind::Hunk,
                            content: format!("@@ -0,0 +1,{} @@ new file", content.lines().count()),
                            old_line: None,
                            new_line: None,
                        });
                        for (i, line) in content.lines().enumerate() {
                            diff_lines.push(DiffLine {
                                kind: DiffLineKind::Addition,
                                content: line.to_string(),
                                old_line: None,
                                new_line: Some(i as u32 + 1),
                            });
                        }
                    }
                    return diff_lines;
                }
            }
        }
    }

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(rel_path.to_string_lossy().as_ref());

    let diff = match repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts)) {
        Ok(d) => d,
        Err(_) => return diff_lines,
    };

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let delta_path = delta.new_file().path().or_else(|| delta.old_file().path());
        if let Some(dp) = delta_path {
            if dp != rel_path {
                return true;
            }
        }
        let content = String::from_utf8_lossy(line.content()).to_string();
        let kind = match line.origin() {
            '+' => DiffLineKind::Addition,
            '-' => DiffLineKind::Deletion,
            'H' | 'F' => DiffLineKind::Hunk,
            _ => DiffLineKind::Context,
        };
        diff_lines.push(DiffLine {
            kind,
            content,
            old_line: line.old_lineno(),
            new_line: line.new_lineno(),
        });
        true
    })
    .ok();

    diff_lines
}

// ── Git Gutter Markers ─────────────────────────────────────

/// Compute git gutter markers for a file by diffing working copy against HEAD.
/// Returns a map of 0-based line numbers → marker type.
pub fn compute_git_gutter(root: &Path, file_path: &Path) -> HashMap<usize, GitGutterMarker> {
    let mut markers: HashMap<usize, GitGutterMarker> = HashMap::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return markers,
    };

    let rel_path = file_path.strip_prefix(root).unwrap_or(file_path);

    // Untracked files: all lines are Added
    if let Ok(statuses) = repo.statuses(None) {
        for entry in statuses.iter() {
            if let Some(p) = entry.path() {
                if PathBuf::from(p) == rel_path && entry.status().is_wt_new() {
                    if let Ok(content) = std::fs::read_to_string(file_path) {
                        for i in 0..content.lines().count() {
                            markers.insert(i, GitGutterMarker::Added);
                        }
                    }
                    return markers;
                }
            }
        }
    }

    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = git2::DiffOptions::new();
    opts.pathspec(rel_path.to_string_lossy().as_ref());

    let diff = match repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts)) {
        Ok(d) => d,
        Err(_) => return markers,
    };

    // Collect diff lines with their origin and new-file line number
    struct DiffEntry {
        origin: char, // '+', '-', or ' '
        new_lineno: Option<u32>,
    }
    let mut entries: Vec<DiffEntry> = Vec::new();

    diff.foreach(
        &mut |_delta, _progress| true,
        None,
        None,
        Some(&mut |_delta, _hunk, line| {
            let origin = line.origin();
            if matches!(origin, '+' | '-' | ' ') {
                entries.push(DiffEntry {
                    origin,
                    new_lineno: line.new_lineno(),
                });
            }
            true
        }),
    )
    .ok();

    // Process entries into change groups: consecutive +/- lines (no context between them)
    let mut i = 0;
    while i < entries.len() {
        if entries[i].origin == ' ' {
            i += 1;
            continue;
        }

        // Collect a change group: consecutive - and + lines
        let group_start = i;
        let mut deletions = 0usize;
        let mut additions: Vec<usize> = Vec::new(); // 0-based line numbers of added lines
        let mut last_new_before = None; // track last new-file line before group for deletion marker

        // Find context line just before this group to get deletion marker position
        if group_start > 0 {
            for j in (0..group_start).rev() {
                if entries[j].origin == ' ' {
                    if let Some(n) = entries[j].new_lineno {
                        last_new_before = Some(n as usize - 1); // 0-based
                    }
                    break;
                }
            }
        }

        while i < entries.len() && entries[i].origin != ' ' {
            if entries[i].origin == '-' {
                deletions += 1;
            } else if entries[i].origin == '+' {
                if let Some(n) = entries[i].new_lineno {
                    additions.push(n as usize - 1); // 0-based
                }
            }
            i += 1;
        }

        if deletions > 0 && !additions.is_empty() {
            // Modified (blue): both deletions and additions in this group
            for line in &additions {
                markers.insert(*line, GitGutterMarker::Modified);
            }
        } else if deletions == 0 && !additions.is_empty() {
            // Pure addition (green)
            for line in &additions {
                markers.insert(*line, GitGutterMarker::Added);
            }
        } else if deletions > 0 && additions.is_empty() {
            // Pure deletion (red triangle)
            if let Some(prev) = last_new_before {
                markers.insert(prev, GitGutterMarker::Deleted);
            } else {
                // Deletion at the very start of the file
                markers.insert(0, GitGutterMarker::DeletedAbove);
            }
        }
    }

    markers
}

// ── Commit Diff View ────────────────────────────────────────

struct CommitFileDiff {
    path: String,
    filename: String,
    additions: usize,
    deletions: usize,
    lines: Vec<DiffLine>,
}

pub struct CommitDiffView {
    hash: String,
    message: String,
    files: Vec<CommitFileDiff>,
    expanded: std::collections::HashSet<usize>,
}

impl CommitDiffView {
    pub fn new(root_path: &Path, hash: &str, message: &str) -> Self {
        let files = generate_commit_diff_by_file(root_path, hash);
        Self {
            hash: hash.to_string(),
            message: message.to_string(),
            files,
            expanded: std::collections::HashSet::new(),
        }
    }
}

fn generate_commit_diff_by_file(root: &Path, hash: &str) -> Vec<CommitFileDiff> {
    let mut file_map: Vec<(String, Vec<DiffLine>)> = Vec::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let commit = match repo.revparse_single(hash).and_then(|o| o.peel_to_commit()) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let tree = match commit.tree() {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let parent_tree = commit.parents().next().and_then(|p| p.tree().ok());
    let diff = match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let file_path = delta.new_file().path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.display().to_string())
            .unwrap_or_default();

        let content = String::from_utf8_lossy(line.content()).to_string();
        let kind = match line.origin() {
            '+' => DiffLineKind::Addition,
            '-' => DiffLineKind::Deletion,
            'H' => DiffLineKind::Hunk,
            'F' => return true, // skip file headers
            _ => DiffLineKind::Context,
        };

        if file_map.is_empty() || file_map.last().unwrap().0 != file_path {
            file_map.push((file_path, Vec::new()));
        }
        file_map.last_mut().unwrap().1.push(DiffLine {
            kind, content, old_line: line.old_lineno(), new_line: line.new_lineno(),
        });
        true
    }).ok();

    file_map.into_iter().map(|(path, lines)| {
        let additions = lines.iter().filter(|l| l.kind == DiffLineKind::Addition).count();
        let deletions = lines.iter().filter(|l| l.kind == DiffLineKind::Deletion).count();
        let filename = Path::new(&path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone());
        CommitFileDiff { path, filename, additions, deletions, lines }
    }).collect()
}

impl Render for CommitDiffView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let total_add: usize = self.files.iter().map(|f| f.additions).sum();
        let total_del: usize = self.files.iter().map(|f| f.deletions).sum();
        let file_count = self.files.len();

        let mut file_entries: Vec<AnyElement> = Vec::new();
        for (idx, file) in self.files.iter().enumerate() {
            let is_expanded = self.expanded.contains(&idx);
            let arrow = if is_expanded { "▼" } else { "▶" };

            // File header row
            file_entries.push(
                div()
                    .id(ElementId::Name(format!("cfile-{}", idx).into()))
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(36.))
                    .px(px(12.))
                    .cursor_pointer()
                    .bg(colors::mantle_solid())
                    .border_b_1()
                    .border_color(colors::surface0_solid())
                    .hover(|d| d.bg(colors::surface0_solid()))
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        if this.expanded.contains(&idx) {
                            this.expanded.remove(&idx);
                        } else {
                            this.expanded.insert(idx);
                        }
                        cx.notify();
                    }))
                    .child(
                        div().w(px(16.)).text_xs().text_color(colors::overlay()).child(arrow),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(colors::text())
                            .font_weight(FontWeight::MEDIUM)
                            .child(format!("{} ", file.filename)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_xs()
                            .text_color(colors::overlay())
                            .child(file.path.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .ml(px(6.))
                            .flex()
                            .flex_row()
                            .gap(px(4.))
                            .text_xs()
                            .child(div().text_color(colors::green()).child(format!("+{}", file.additions)))
                            .child(div().text_color(colors::red()).child(format!("-{}", file.deletions))),
                    )
                    .into_any_element(),
            );

            // Inline diff (when expanded)
            if is_expanded {
                let diff_rows = render_side_by_side(&file.lines);
                file_entries.push(
                    div()
                        .w_full()
                        .bg(colors::base_solid())
                        .children(diff_rows)
                        .into_any_element(),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::base_solid())
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Title bar with hash, message, stats
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(30.))
                    .flex_shrink_0()
                    .px(px(12.))
                    .bg(colors::mantle_solid())
                    .border_b_1()
                    .border_color(colors::surface1_solid())
                    .gap(px(8.))
                    .child(
                        div()
                            .text_color(colors::blue())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(self.hash.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(colors::text())
                            .truncate()
                            .child(self.message.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .flex()
                            .flex_row()
                            .gap(px(6.))
                            .text_xs()
                            .child(div().text_color(colors::overlay()).child(format!("{} file{}", file_count, if file_count != 1 { "s" } else { "" })))
                            .child(div().text_color(colors::green()).child(format!("+{}", total_add)))
                            .child(div().text_color(colors::red()).child(format!("-{}", total_del))),
                    ),
            )
            // Scrollable file list with inline diffs
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .id("commit-diff-scroll")
                    .overflow_y_scroll()
                    .children(file_entries),
            )
    }
}

// ── Changes Review View (file list for push review) ─────────

pub enum ChangesReviewEvent {
    FileClicked(PathBuf),
    PushConfirmed,
}

impl gpui::EventEmitter<ChangesReviewEvent> for ChangesReviewView {}

pub struct ChangesReviewView {
    root_path: PathBuf,
    changes: Vec<GitFileChange>,
    selected_index: Option<usize>,
    is_pushing: bool,
    push_done: bool,
    push_result: Option<String>,
}

impl ChangesReviewView {
    pub fn new(root_path: PathBuf) -> Self {
        let changes = get_changes(&root_path);
        Self {
            root_path,
            changes,
            selected_index: None,
            is_pushing: false,
            push_done: false,
            push_result: None,
        }
    }

    fn status_color(status: &ChangeStatus) -> Rgba {
        match status {
            ChangeStatus::Modified => colors::yellow(),
            ChangeStatus::Added | ChangeStatus::Untracked => colors::green(),
            ChangeStatus::Deleted => colors::red(),
            ChangeStatus::Renamed => colors::blue(),
        }
    }

    fn total_stats(&self) -> (usize, usize) {
        self.changes.iter().fold((0, 0), |(ins, del), c| {
            (ins + c.insertions, del + c.deletions)
        })
    }
}

impl Render for ChangesReviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (total_ins, total_del) = self.total_stats();
        let file_count = self.changes.len();
        let selected = self.selected_index;

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::base_solid())
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Header
            .child(
                div()
                    .px(px(12.))
                    .py(px(10.))
                    .bg(colors::mantle_solid())
                    .border_b_1()
                    .border_color(colors::surface1())
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_base()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(colors::text())
                                    .child("Changes to push"),
                            )
                            .child(div().text_xs().text_color(colors::overlay()).child(format!(
                                "{} file{}",
                                file_count,
                                if file_count != 1 { "s" } else { "" }
                            )))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(6.))
                                    .text_xs()
                                    .child(
                                        div()
                                            .text_color(colors::green())
                                            .child(format!("+{}", total_ins)),
                                    )
                                    .child(
                                        div()
                                            .text_color(colors::red())
                                            .child(format!("-{}", total_del)),
                                    ),
                            ),
                    )
                    // Push button
                    .child(
                        div()
                            .id("review-push-btn")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(28.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(!self.is_pushing && !self.push_done, |d: Stateful<Div>| {
                                d.bg(colors::blue())
                                    .text_color(gpui::rgb(0xffffff))
                                    .hover(|d| d.opacity(0.85))
                                    .child("Push")
                            })
                            .when(self.is_pushing, |d: Stateful<Div>| {
                                d.bg(colors::surface1())
                                    .text_color(colors::overlay())
                                    .child("Pushing...")
                            })
                            .when(self.push_done, |d: Stateful<Div>| {
                                d.bg(colors::surface0()).text_color(colors::green()).child(
                                    self.push_result.as_deref().unwrap_or("Pushed!").to_string(),
                                )
                            })
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                if this.is_pushing || this.push_done {
                                    return;
                                }
                                this.is_pushing = true;
                                cx.notify();
                                cx.emit(ChangesReviewEvent::PushConfirmed);

                                let root = this.root_path.clone();
                                cx.spawn(
                                    async |this: WeakEntity<ChangesReviewView>,
                                           cx: &mut AsyncApp| {
                                        let result = cx
                                            .background_executor()
                                            .spawn(async move {
                                                crate::operations::one_button_commit_and_push(&root)
                                            })
                                            .await;
                                        this.update(cx, |view, cx| {
                                            view.is_pushing = false;
                                            view.push_done = true;
                                            view.push_result = Some(match result {
                                                Ok(msg) => msg,
                                                Err(e) => format!("Error: {}", e),
                                            });
                                            cx.notify();
                                        })
                                        .ok();
                                    },
                                )
                                .detach();
                            })),
                    ),
            )
            // Scrollable file list
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .min_h(px(0.))
                    .id("changes-review-scroll")
                    .overflow_y_scroll()
                    .children(self.changes.iter().enumerate().map(|(idx, change)| {
                        let is_selected = selected == Some(idx);
                        let color = Self::status_color(&change.status);
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

                        div()
                            .id(ElementId::Name(format!("review-file-{}", idx).into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .w_full()
                            .h(px(30.))
                            .px(px(12.))
                            .cursor_pointer()
                            .when(is_selected, |d: Stateful<Div>| d.bg(colors::surface0()))
                            .hover(|d| d.bg(colors::surface0()))
                            .child(div().w(px(20.)).text_color(color).child(label))
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_color(colors::text())
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(format!("{} ", filename)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .truncate()
                                    .text_xs()
                                    .text_color(colors::overlay())
                                    .child(dir),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .ml(px(8.))
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(4.))
                                    .text_xs()
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
                                this.selected_index = Some(idx);
                                let abs_path = this.root_path.join(&this.changes[idx].path);
                                cx.emit(ChangesReviewEvent::FileClicked(abs_path));
                                cx.notify();
                            }))
                    })),
            )
    }
}
