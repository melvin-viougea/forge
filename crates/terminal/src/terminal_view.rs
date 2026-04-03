use gpui::*;
use gpui::prelude::*;
use std::time::Duration;

use crate::terminal::{CellStyle, TermColor, Terminal, TerminalLine};

pub struct TerminalView {
    pub terminal: Terminal,
    focus_handle: FocusHandle,
    needs_focus: bool,
    _poll_task: Task<()>,
}

mod colors {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn base() -> Rgba { rgb(0x1e1e2e) }
    pub fn text() -> Rgba { rgb(0xcdd6f4) }
    pub fn green() -> Rgba { rgb(0xa6e3a1) }
    pub fn surface1() -> Rgba { rgb(0x45475a) }
}

impl TerminalView {
    pub fn new(title: String, cx: &mut Context<Self>) -> Self {
        Self::new_in(title, None, cx)
    }

    pub fn new_in(title: String, working_dir: Option<std::path::PathBuf>, cx: &mut Context<Self>) -> Self {
        let terminal = Terminal::new(title, 120, 40, working_dir).expect("Failed to create terminal");
        let focus_handle = cx.focus_handle();

        let poll_task = cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(Duration::from_millis(50)).await;
                let result = this.update(cx, |view, cx| {
                    if view.terminal.check_and_clear_new_data() {
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
            _poll_task: poll_task,
        }
    }
}

impl Focusable for TerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Render a styled span as a GPUI element
fn render_span(text: String, style: &CellStyle, bold: bool) -> Div {
    let fg = style.fg.to_rgba(true);
    let has_bg = style.bg != TermColor::Default;
    let bg = style.bg.to_rgba(false);

    let mut el = div()
        .text_color(fg)
        .child(text);

    if bold {
        el = el.font_weight(FontWeight::BOLD);
    }
    if has_bg {
        el = el.bg(bg);
    }

    el
}

/// Render a terminal line as a row of styled spans
fn render_line(line: &TerminalLine) -> Div {
    let spans = line.to_spans();
    let mut row = div()
        .w_full()
        .min_h(px(18.))
        .flex()
        .flex_row()
        .overflow_hidden();

    for span in &spans {
        row = row.child(render_span(span.text.clone(), &span.style, span.style.bold));
    }

    row
}

/// Render the last line with an inline cursor
fn render_line_with_cursor(line: &TerminalLine, cursor_col: usize, is_focused: bool) -> Div {
    let cells = &line.cells;
    let mut row = div()
        .w_full()
        .min_h(px(18.))
        .flex()
        .flex_row()
        .overflow_hidden();

    if !is_focused || cells.is_empty() {
        // No cursor, just render normally
        return render_line(line);
    }

    // Build spans for text before cursor
    if cursor_col > 0 {
        let before_line = TerminalLine {
            cells: cells[..cursor_col.min(cells.len())].to_vec(),
        };
        for span in before_line.to_spans() {
            row = row.child(render_span(span.text, &span.style, span.style.bold));
        }
    }

    // Cursor block
    let cursor_char = cells
        .get(cursor_col)
        .map(|c| c.ch)
        .unwrap_or(' ');

    row = row.child(
        div()
            .bg(colors::green())
            .text_color(colors::base())
            .min_w(px(8.))
            .child(cursor_char.to_string()),
    );

    // Text after cursor
    if cursor_col + 1 < cells.len() {
        let after_line = TerminalLine {
            cells: cells[cursor_col + 1..].to_vec(),
        };
        for span in after_line.to_spans() {
            row = row.child(render_span(span.text, &span.style, span.style.bold));
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

        let lines = self.terminal.get_visible_lines(200);
        let is_focused = self.focus_handle.is_focused(window);
        let (cursor_row, cursor_col) = self.terminal.cursor_position();
        let line_count = lines.len();

        div()
            .id("terminal-view")
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::base())
            .p(px(8.))
            .overflow_y_scroll()
            .text_sm()
            .font_family("MesloLGS NF")
            .track_focus(&self.focus_handle)
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _ev, window, _cx| {
                this.focus_handle.focus(window);
            }))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _window, cx| {
                // Handle special keys FIRST, before key_char
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
                        // Ctrl+letter (C, D, Z, etc.) — always handle, even without key_char
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
                    cx.notify();
                }
            }))
            .children(
                lines.iter().enumerate().map(move |(idx, line)| {
                    if idx == cursor_row && is_focused {
                        render_line_with_cursor(line, cursor_col, true)
                    } else {
                        render_line(line)
                    }
                }),
            )
    }
}
