use gpui::*;
use gpui::prelude::*;
use std::path::{Path, PathBuf};

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn base() -> Rgba { rgb(0x0a0e14) }
    pub fn mantle() -> Rgba { rgb(0x0d1117) }
    pub fn surface0() -> Rgba { rgb(0x161b22) }
    pub fn surface1() -> Rgba { rgb(0x21262d) }
    pub fn text() -> Rgba { rgb(0xc9d1d9) }
    pub fn subtext() -> Rgba { rgb(0x8b949e) }
    pub fn green() -> Rgba { rgb(0x3fb950) }
    pub fn red() -> Rgba { rgb(0xf85149) }
    pub fn overlay() -> Rgba { rgb(0x484f58) }
}

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

pub struct DiffView {
    pub file_path: Option<PathBuf>,
    pub lines: Vec<DiffLine>,
    root_path: PathBuf,
}

impl DiffView {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            file_path: None,
            lines: Vec::new(),
            root_path,
        }
    }

    pub fn show_diff(&mut self, file_path: PathBuf) {
        self.file_path = Some(file_path.clone());
        self.lines = generate_diff(&self.root_path, &file_path);
    }

    pub fn clear(&mut self) {
        self.file_path = None;
        self.lines.clear();
    }
}

fn generate_diff(root: &Path, file_path: &Path) -> Vec<DiffLine> {
    let mut diff_lines = Vec::new();

    let repo = match git2::Repository::discover(root) {
        Ok(r) => r,
        Err(_) => return diff_lines,
    };

    let diff = match repo.diff_index_to_workdir(None, None) {
        Ok(d) => d,
        Err(_) => return diff_lines,
    };

    let rel_path = file_path
        .strip_prefix(root)
        .unwrap_or(file_path);

    diff.print(git2::DiffFormat::Patch, |delta, _hunk, line| {
        let delta_path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path());

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

impl Render for DiffView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::base())
            .id("diff-view-scroll")
            .overflow_y_scroll()
            .text_xs()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            // Header
            .child(
                div()
                    .px(px(8.))
                    .py(px(4.))
                    .bg(colors::mantle())
                    .border_b_1()
                    .border_color(colors::surface1())
                    .text_color(colors::subtext())
                    .child(
                        self.file_path
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "No file selected".to_string()),
                    ),
            )
            // Diff lines
            .children(self.lines.iter().map(|line| {
                let (bg, text_color, prefix) = match line.kind {
                    DiffLineKind::Addition => (Some(rgb(0x0d2818)), colors::green(), "+"),
                    DiffLineKind::Deletion => (Some(rgb(0x2d1214)), colors::red(), "-"),
                    DiffLineKind::Hunk => (Some(colors::surface0()), colors::overlay(), "@"),
                    DiffLineKind::Context => (None, colors::text(), " "),
                };

                let line_num = match (&line.old_line, &line.new_line) {
                    (Some(old), Some(new)) => format!("{:>4} {:>4}", old, new),
                    (Some(old), None) => format!("{:>4}     ", old),
                    (None, Some(new)) => format!("     {:>4}", new),
                    _ => "         ".to_string(),
                };

                div()
                    .flex()
                    .flex_row()
                    .w_full()
                    .h(px(18.))
                    .when_some(bg, |d: Div, bg_color| d.bg(bg_color))
                    .child(
                        div()
                            .w(px(72.))
                            .text_color(colors::overlay())
                            .px(px(4.))
                            .child(line_num),
                    )
                    .child(
                        div()
                            .w(px(12.))
                            .text_color(text_color)
                            .child(prefix),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(text_color)
                            .child(line.content.clone()),
                    )
            }))
    }
}
