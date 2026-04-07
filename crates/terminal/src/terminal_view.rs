use gpui::*;
use gpui::prelude::*;
use std::time::Duration;

use crate::terminal::{CellStyle, TermColor, Terminal, TerminalLine};

// ── Layout dimensions (shared via Global) ───────────────────

/// Stores current layout widths so the terminal can compute its actual available space.
pub struct LayoutDimensions {
    pub left_dock_width: f32,
    pub right_dock_width: f32,
    pub pane_sidebar_width: f32,
}

impl Global for LayoutDimensions {}

// ── Selection model ─────────────────────────────────────────

#[derive(Clone, Debug)]
struct Selection {
    start: (usize, usize), // (row, col)
    end: (usize, usize),   // (row, col)
}

impl Selection {
    /// Return (start, end) in reading order
    fn ordered(&self) -> ((usize, usize), (usize, usize)) {
        if self.start.0 < self.end.0
            || (self.start.0 == self.end.0 && self.start.1 <= self.end.1)
        {
            (self.start, self.end)
        } else {
            (self.end, self.start)
        }
    }

    fn is_selected(&self, row: usize, col: usize) -> bool {
        let (start, end) = self.ordered();
        if row < start.0 || row > end.0 {
            return false;
        }
        if start.0 == end.0 {
            return col >= start.1 && col <= end.1;
        }
        if row == start.0 {
            return col >= start.1;
        }
        if row == end.0 {
            return col <= end.1;
        }
        true // middle row — fully selected
    }
}

pub enum TerminalViewEvent {
    TitleChanged(String),
}

impl gpui::EventEmitter<TerminalViewEvent> for TerminalView {}

pub struct TerminalView {
    pub terminal: Terminal,
    focus_handle: FocusHandle,
    needs_focus: bool,
    last_size: Option<(u16, u16)>,
    _poll_task: Task<()>,
    selection: Option<Selection>,
    is_selecting: bool,
    /// When true, terminal sizes itself for a narrow side panel (e.g. 280px).
    pub compact: bool,
    detected_title: Option<String>,
    scroll_acc: f32,
}

use ide_workspace::theme as colors;

impl TerminalView {
    pub fn new(title: String, cx: &mut Context<Self>) -> Self {
        Self::new_in(title, None, cx)
    }

    pub fn new_in(title: String, working_dir: Option<std::path::PathBuf>, cx: &mut Context<Self>) -> Self {
        let terminal = Terminal::new(title, 80, 24, working_dir).expect("Failed to create terminal");
        let focus_handle = cx.focus_handle();

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                let result = this.update(cx, |view, cx| {
                    if view.terminal.check_and_clear_new_data() {
                        // Check for OSC title changes from the terminal
                        if let Some(osc_title) = view.terminal.take_osc_title() {
                            // Strip user@host: prefix if present
                            let stripped = if let Some(colon_pos) = osc_title.find(':') {
                                let before = &osc_title[..colon_pos];
                                if before.contains('@') {
                                    osc_title[colon_pos + 1..].to_string()
                                } else {
                                    osc_title.clone()
                                }
                            } else {
                                osc_title.clone()
                            };
                            // Extract first word as process name
                            let process_name = stripped
                                .split_whitespace()
                                .next()
                                .unwrap_or(&stripped)
                                .to_string();
                            // Ignore shell prompt noise (symbols, single chars, paths)
                            let looks_like_process = process_name.len() >= 2
                                && process_name.chars().any(|c| c.is_ascii_alphabetic())
                                && !process_name.starts_with('/');
                            if looks_like_process {
                                let mut chars = process_name.chars();
                                let capitalized = match chars.next() {
                                    Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                                    None => process_name.clone(),
                                };
                                if view.detected_title.as_ref() != Some(&capitalized) {
                                    view.detected_title = Some(capitalized.clone());
                                    cx.emit(TerminalViewEvent::TitleChanged(capitalized));
                                }
                            }
                        }
                        cx.notify();
                    }
                });
                if result.is_err() {
                    break;
                }
            }
        });

        Self {
            terminal,
            focus_handle,
            needs_focus: true,
            last_size: None,
            _poll_task: poll_task,
            selection: None,
            is_selecting: false,
            compact: false,
            detected_title: None,
            scroll_acc: 0.0,
        }
    }
}

// ── Clipboard & selection helpers ────────────────────────────

/// Approximate character metrics for MesloLGS NF at text_sm
const CHAR_WIDTH: f32 = 8.4;
const LINE_HEIGHT: f32 = 18.0;
const CONTENT_PADDING: f32 = 8.0;

/// Layout offsets: left dock (200) + pane sidebar (240) for X, titlebar+divider (30) for Y
const LAYOUT_OFFSET_X: f32 = 200.0 + 240.0;
const LAYOUT_OFFSET_Y: f32 = 30.0;

impl TerminalView {
    /// Convert a window-relative mouse position to terminal cell (row, col).
    fn mouse_to_cell(&self, pos: Point<Pixels>) -> (usize, usize) {
        let x: f32 = pos.x.into();
        let y: f32 = pos.y.into();

        // Offsets differ between center pane and compact (right panel) mode
        let (off_x, off_y) = if self.compact {
            // Right panel: viewport_width - 280 (dock width) for x,
            // titlebar(29) + commit_panel(~50) + divider(1) + runner_header(28) for y
            // We approximate since we don't have the exact viewport here
            (0.0, 0.0) // Will be relative to the terminal div
        } else {
            (LAYOUT_OFFSET_X, LAYOUT_OFFSET_Y)
        };

        let col = ((x - off_x - CONTENT_PADDING) / CHAR_WIDTH).max(0.0) as usize;
        let row = ((y - off_y - CONTENT_PADDING) / LINE_HEIGHT).max(0.0) as usize;
        let max_row = self.terminal.rows.saturating_sub(1) as usize;
        let max_col = self.terminal.cols.saturating_sub(1) as usize;
        (row.min(max_row), col.min(max_col))
    }

    /// Extract text covered by the current selection from visible lines.
    fn get_selected_text(&self) -> String {
        let sel = match &self.selection {
            Some(s) => s,
            None => return String::new(),
        };
        let (start, end) = sel.ordered();
        let lines = self.terminal.get_visible_lines(200);
        let mut result = String::new();

        for (row_idx, line) in lines.iter().enumerate() {
            if row_idx < start.0 || row_idx > end.0 {
                continue;
            }
            let from = if row_idx == start.0 { start.1 } else { 0 };
            let to = if row_idx == end.0 {
                end.1 + 1
            } else {
                line.cells.len()
            };

            for col in from..to.min(line.cells.len()) {
                result.push(line.cells[col].ch);
            }
            // Add newline between selected rows (but not after the last)
            if row_idx < end.0 {
                // Trim trailing spaces before the newline
                let trimmed = result.trim_end_matches(' ');
                result.truncate(trimmed.len());
                result.push('\n');
            }
        }
        // Trim trailing spaces on the last line too
        let trimmed = result.trim_end_matches(' ');
        trimmed.to_string()
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Render a styled span as a GPUI element
fn render_span(text: String, style: &CellStyle, bold: bool, selected: bool) -> Div {
    let fg = style.fg.to_rgba(true);

    let mut el = div()
        .text_color(fg)
        .child(text);

    if bold {
        el = el.font_weight(FontWeight::BOLD);
    }
    if selected {
        el = el.bg(colors::selection());
    } else if style.bg != TermColor::Default {
        el = el.bg(style.bg.to_rgba(false));
    }

    el
}

/// Render a terminal line as a row of per-cell divs, applying selection highlight.
fn render_line_sel(line: &TerminalLine, row_idx: usize, selection: Option<&Selection>) -> Div {
    let mut row = div()
        .min_h(px(18.))
        .flex()
        .flex_row()
        .overflow_hidden();

    if line.cells.is_empty() {
        return row.child(render_span(" ".to_string(), &CellStyle::default(), false, false));
    }

    // Group consecutive cells with the same style + selection state into spans
    let mut text = String::new();
    let mut style = line.cells[0].style.clone();
    let mut sel_state = selection.map_or(false, |s| s.is_selected(row_idx, 0));

    for (col, cell) in line.cells.iter().enumerate() {
        let cell_sel = selection.map_or(false, |s| s.is_selected(row_idx, col));
        if cell.style.fg == style.fg
            && cell.style.bg == style.bg
            && cell.style.bold == style.bold
            && cell_sel == sel_state
        {
            text.push(cell.ch);
        } else {
            if !text.is_empty() {
                row = row.child(render_span(text.clone(), &style, style.bold, sel_state));
            }
            text.clear();
            text.push(cell.ch);
            style = cell.style.clone();
            sel_state = cell_sel;
        }
    }
    if !text.is_empty() {
        row = row.child(render_span(text, &style, style.bold, sel_state));
    }

    row
}

/// Render a line that contains the cursor, with optional selection highlight.
fn render_line_with_cursor_sel(
    line: &TerminalLine,
    row_idx: usize,
    cursor_col: usize,
    is_focused: bool,
    selection: Option<&Selection>,
) -> Div {
    let cells = &line.cells;
    let mut row = div()
        .min_h(px(18.))
        .flex()
        .flex_row()
        .overflow_hidden();

    if !is_focused || cells.is_empty() {
        return render_line_sel(line, row_idx, selection);
    }

    // Before cursor — render per-cell with selection
    if cursor_col > 0 {
        row = div()
            .min_h(px(18.))
            .flex()
            .flex_row()
            .overflow_hidden();

        for (col, cell) in cells.iter().enumerate().take(cursor_col.min(cells.len())) {
            let cell_sel = selection.map_or(false, |s| s.is_selected(row_idx, col));
            row = row.child(render_span(
                cell.ch.to_string(),
                &cell.style,
                cell.style.bold,
                cell_sel,
            ));
        }
    }

    // Cursor block
    let cursor_char = cells.get(cursor_col).map(|c| c.ch).unwrap_or(' ');
    let cursor_sel = selection.map_or(false, |s| s.is_selected(row_idx, cursor_col));
    row = row.child(
        div()
            .bg(if cursor_sel { colors::selection() } else { colors::cursor() })
            .text_color(colors::base())
            .min_w(px(8.))
            .child(cursor_char.to_string()),
    );

    // After cursor
    if cursor_col + 1 < cells.len() {
        for col in (cursor_col + 1)..cells.len() {
            let cell = &cells[col];
            let cell_sel = selection.map_or(false, |s| s.is_selected(row_idx, col));
            row = row.child(render_span(
                cell.ch.to_string(),
                &cell.style,
                cell.style.bold,
                cell_sel,
            ));
        }
    }

    row
}

impl Render for TerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_focus {
            self.needs_focus = false;
            cx.focus_self(window);
        }

        // Measure actual font advance width dynamically
        let char_advance = {
            let text_system = window.text_system();
            let f = font("MesloLGS NF");
            let font_id = text_system.resolve_font(&f);
            let font_size = px(0.875 * 16.0); // text_sm = 0.875rem = 14px
            text_system.advance(font_id, font_size, 'a')
                .map(|s| { let w: f32 = s.width.into(); w })
                .unwrap_or(CHAR_WIDTH)
        };

        // Resize terminal based on window viewport and actual layout dimensions
        let viewport = window.viewport_size();
        let available_w: f32 = viewport.width.into();
        let available_h: f32 = viewport.height.into();

        let (left_dock_w, right_dock_w, sidebar_w) = cx.try_global::<LayoutDimensions>()
            .map(|l| (l.left_dock_width, l.right_dock_width, l.pane_sidebar_width))
            .unwrap_or((200.0, 280.0, 240.0));

        let (term_w, term_h) = if self.compact {
            // Right panel: dock width minus padding/scrollbar
            let w = (right_dock_w - 26.0_f32).max(100.0);
            let h = ((available_h - 200.0) / 2.0).max(80.0);
            (w, h)
        } else {
            // Center pane: viewport minus docks, sidebar, padding(16), scrollbar(10), dividers(10)
            let deduction = left_dock_w + sidebar_w + right_dock_w + 36.0;
            let w = (available_w - deduction).max(200.0);
            let h = (available_h - 50.0).max(100.0);
            (w, h)
        };
        let new_cols = ((term_w / char_advance).floor() as u16).saturating_sub(5);
        let new_rows = (term_h / 18.0) as u16;
        let new_size = (new_cols, new_rows);
        if self.last_size != Some(new_size) {
            self.last_size = Some(new_size);
            self.terminal.resize(new_cols, new_rows);
        }

        let lines = self.terminal.get_visible_lines(200);
        let is_focused = self.focus_handle.is_focused(window);
        let (cursor_row, cursor_col) = self.terminal.cursor_position();
        let line_count = lines.len();
        let (scroll_offset, history_size, screen_lines) = self.terminal.scroll_info();
        let selection = self.selection.clone();


        div()
            .id("terminal-view")
            .flex()
            .flex_row()
            .size_full()
            .track_focus(&self.focus_handle)
            // ── Mouse: selection ──────────────────────────────
            .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, window, cx| {
                this.focus_handle.focus(window);
                let (row, col) = this.mouse_to_cell(ev.position);
                this.selection = Some(Selection {
                    start: (row, col),
                    end: (row, col),
                });
                this.is_selecting = true;
                cx.notify();
            }))
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _window, cx| {
                if this.is_selecting {
                    let (row, col) = this.mouse_to_cell(ev.position);
                    if let Some(ref mut sel) = this.selection {
                        sel.end = (row, col);
                    }
                    cx.notify();
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, ev: &MouseUpEvent, _window, cx| {
                if this.is_selecting {
                    let (row, col) = this.mouse_to_cell(ev.position);
                    if let Some(ref mut sel) = this.selection {
                        sel.end = (row, col);
                    }
                    this.is_selecting = false;
                    // Click without drag → clear selection
                    if let Some(ref sel) = this.selection {
                        if sel.start == sel.end {
                            this.selection = None;
                        }
                    }
                    cx.notify();
                }
            }))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                let delta = ev.delta.pixel_delta(px(1.0));
                let dy: f32 = delta.y.into();
                this.scroll_acc += dy;
                let lines = (this.scroll_acc / 8.0) as i32;
                if lines != 0 {
                    this.scroll_acc -= lines as f32 * 8.0;
                    this.terminal.scroll(lines);
                    cx.notify();
                }
            }))
            // ── Keyboard: clipboard + input ───────────────────
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                // Clipboard shortcuts (Cmd+C / Cmd+V / Cmd+X / Cmd+A)
                if ev.keystroke.modifiers.platform {
                    match ev.keystroke.key.as_str() {
                        "v" => {
                            if let Some(item) = cx.read_from_clipboard() {
                                if let Some(text) = item.text() {
                                    // Text paste — use bracket paste when mode is enabled
                                    this.terminal.paste(text.as_bytes());
                                } else {
                                    // Image in clipboard — send empty bracket paste
                                    // so CLI apps (e.g. Claude Code) detect the paste
                                    // event and read the clipboard image themselves.
                                    this.terminal.paste(b"");
                                }
                                this.terminal.scroll_to_bottom();
                            }
                            this.selection = None;
                            cx.notify();
                            return;
                        }
                        "c" | "x" => {
                            let text = this.get_selected_text();
                            if !text.is_empty() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                            }
                            this.selection = None;
                            cx.notify();
                            return;
                        }
                        "a" => {
                            let lines = this.terminal.get_visible_lines(200);
                            if !lines.is_empty() {
                                let last_row = lines.len().saturating_sub(1);
                                let last_col = lines[last_row]
                                    .cells
                                    .len()
                                    .saturating_sub(1);
                                this.selection = Some(Selection {
                                    start: (0, 0),
                                    end: (last_row, last_col),
                                });
                            }
                            cx.notify();
                            return;
                        }
                        _ => { return; }
                    }
                }

                // Any regular key press clears the selection
                if this.selection.is_some() {
                    this.selection = None;
                }

                let handled = match ev.keystroke.key.as_str() {
                    "enter" => { this.terminal.write_input(b"\r"); true }
                    "backspace" => { this.terminal.write_input(b"\x7f"); true }
                    "tab" => { this.terminal.write_input(b"\t"); true }
                    "escape" => { this.terminal.write_input(b"\x1b"); true }
                    "space" => { this.terminal.write_input(b" "); true }
                    "up" => { this.terminal.write_input(b"\x1b[A"); true }
                    "down" => { this.terminal.write_input(b"\x1b[B"); true }
                    "left" => { this.terminal.write_input(b"\x1b[D"); true }
                    "right" => { this.terminal.write_input(b"\x1b[C"); true }
                    "delete" => { this.terminal.write_input(b"\x1b[3~"); true }
                    "home" => { this.terminal.write_input(b"\x1b[H"); true }
                    "end" => { this.terminal.write_input(b"\x1b[F"); true }
                    _ => {
                        if ev.keystroke.modifiers.control {
                            let ch = ev.keystroke.key.chars().next().unwrap_or('c');
                            if ch.is_ascii_alphabetic() {
                                let ctrl_byte = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                                this.terminal.write_input(&[ctrl_byte]);
                                true
                            } else {
                                false
                            }
                        } else if let Some(key_char) = &ev.keystroke.key_char {
                            if !ev.keystroke.modifiers.platform {
                                this.terminal.write_input(key_char.as_bytes());
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                };
                if handled {
                    this.terminal.scroll_to_bottom();
                    cx.notify();
                }
            }))
            // Terminal content
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .p(px(8.))
                    .text_sm()
                    .font_family("MesloLGS NF")
                    .children(
                        lines.iter().enumerate().map(move |(idx, line)| {
                            let sel_ref = selection.as_ref();
                            if idx == cursor_row && is_focused {
                                render_line_with_cursor_sel(line, idx, cursor_col, true, sel_ref)
                            } else {
                                render_line_sel(line, idx, sel_ref)
                            }
                        }),
                    ),
            )
    }
}
