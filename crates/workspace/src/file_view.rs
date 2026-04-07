use gpui::*;
use gpui::prelude::*;
use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::theme;

pub enum FileViewEvent {
    TitleChanged(String),
}

impl gpui::EventEmitter<FileViewEvent> for FileView {}

pub struct FileView {
    path: PathBuf,
    lines: Vec<String>,
    filename: String,
    cursor_row: usize,
    cursor_col: usize,
    modified: bool,
    focus_handle: FocusHandle,
    selection: Option<(usize, usize, usize, usize)>, // (start_row, start_col, end_row, end_col)
    selecting: bool,
    content_origin: Rc<Cell<(f32, f32)>>, // (x, y) of content area in window coords
}

impl FileView {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let content = std::fs::read_to_string(&path).unwrap_or_else(|_| String::new());
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }

        let focus_handle = cx.focus_handle();

        Self {
            path,
            lines,
            filename,
            cursor_row: 0,
            cursor_col: 0,
            modified: false,
            focus_handle,
            selection: None,
            selecting: false,
            content_origin: Rc::new(Cell::new((0.0, 0.0))),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    fn mark_modified(&mut self, cx: &mut Context<Self>) {
        if !self.modified {
            self.modified = true;
            cx.emit(FileViewEvent::TitleChanged(format!("{} ●", self.filename)));
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let content = self.lines.join("\n");
        if std::fs::write(&self.path, &content).is_ok() {
            self.modified = false;
            cx.emit(FileViewEvent::TitleChanged(self.filename.clone()));
        }
    }

    fn insert_char(&mut self, ch: &str, cx: &mut Context<Self>) {
        self.delete_selection();
        self.ensure_cursor_bounds();
        let line = &mut self.lines[self.cursor_row];
        // Pad line if cursor is beyond current length
        while line.len() < self.cursor_col {
            line.push(' ');
        }
        line.insert_str(self.cursor_col, ch);
        self.cursor_col += ch.len();
        self.mark_modified(cx);
    }

    fn insert_newline(&mut self, cx: &mut Context<Self>) {
        self.delete_selection();
        self.ensure_cursor_bounds();
        let line = &mut self.lines[self.cursor_row];
        let rest = line[self.cursor_col..].to_string();
        line.truncate(self.cursor_col);
        self.cursor_row += 1;
        self.lines.insert(self.cursor_row, rest);
        self.cursor_col = 0;
        self.mark_modified(cx);
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.delete_selection() {
            self.mark_modified(cx);
            return;
        }
        self.ensure_cursor_bounds();
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            // Handle multi-byte chars
            let byte_pos = self.cursor_col;
            if byte_pos <= line.len() {
                let prev_char_start = line[..byte_pos].char_indices().last().map(|(i, _)| i).unwrap_or(0);
                line.drain(prev_char_start..byte_pos);
                self.cursor_col = prev_char_start;
            }
            self.mark_modified(cx);
        } else if self.cursor_row > 0 {
            let current_line = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
            self.lines[self.cursor_row].push_str(&current_line);
            self.mark_modified(cx);
        }
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.delete_selection() {
            self.mark_modified(cx);
            return;
        }
        self.ensure_cursor_bounds();
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            let next_char_end = line[self.cursor_col..].char_indices().nth(1).map(|(i, _)| self.cursor_col + i).unwrap_or(line.len());
            self.lines[self.cursor_row].drain(self.cursor_col..next_char_end);
            self.mark_modified(cx);
        } else if self.cursor_row + 1 < self.lines.len() {
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            self.mark_modified(cx);
        }
    }

    fn insert_tab(&mut self, cx: &mut Context<Self>) {
        self.insert_char("    ", cx);
    }

    fn ensure_cursor_bounds(&mut self) {
        if self.cursor_row >= self.lines.len() {
            self.cursor_row = self.lines.len().saturating_sub(1);
        }
        let line_len = self.lines[self.cursor_row].len();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    fn move_left(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }

        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_row];
            self.cursor_col = line[..self.cursor_col].char_indices().last().map(|(i, _)| i).unwrap_or(0);
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].len();
        }

        if shift { self.extend_selection(); }
    }

    fn move_right(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }

        self.ensure_cursor_bounds();
        let line = &self.lines[self.cursor_row];
        if self.cursor_col < line.len() {
            self.cursor_col = line[self.cursor_col..].char_indices().nth(1).map(|(i, _)| self.cursor_col + i).unwrap_or(line.len());
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }

        if shift { self.extend_selection(); }
    }

    fn move_up(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }

        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.ensure_cursor_bounds();
        }

        if shift { self.extend_selection(); }
    }

    fn move_down(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }

        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.ensure_cursor_bounds();
        }

        if shift { self.extend_selection(); }
    }

    fn move_home(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }
        self.cursor_col = 0;
        if shift { self.extend_selection(); }
    }

    fn move_end(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); }
        else { self.selection = None; }
        self.ensure_cursor_bounds();
        self.cursor_col = self.lines[self.cursor_row].len();
        if shift { self.extend_selection(); }
    }

    // Selection helpers
    fn start_or_extend_selection(&mut self) {
        if self.selection.is_none() {
            self.selection = Some((self.cursor_row, self.cursor_col, self.cursor_row, self.cursor_col));
        }
    }

    fn extend_selection(&mut self) {
        if let Some((sr, sc, _, _)) = self.selection {
            self.selection = Some((sr, sc, self.cursor_row, self.cursor_col));
        }
    }

    fn select_all(&mut self) {
        let last_row = self.lines.len().saturating_sub(1);
        let last_col = self.lines[last_row].len();
        self.selection = Some((0, 0, last_row, last_col));
        self.cursor_row = last_row;
        self.cursor_col = last_col;
    }

    fn get_selected_text(&self) -> String {
        let (sr, sc, er, ec) = match self.selection {
            Some(sel) => self.normalize_selection(sel),
            None => return String::new(),
        };
        if sr == er {
            return self.lines[sr][sc..ec].to_string();
        }
        let mut result = self.lines[sr][sc..].to_string();
        result.push('\n');
        for row in (sr + 1)..er {
            result.push_str(&self.lines[row]);
            result.push('\n');
        }
        result.push_str(&self.lines[er][..ec]);
        result
    }

    fn delete_selection(&mut self) -> bool {
        let sel = match self.selection {
            Some(sel) => sel,
            None => return false,
        };
        let (sr, sc, er, ec) = self.normalize_selection(sel);
        self.selection = None;

        if sr == er {
            self.lines[sr].drain(sc..ec);
        } else {
            let end_rest = self.lines[er][ec..].to_string();
            self.lines[sr].truncate(sc);
            self.lines[sr].push_str(&end_rest);
            self.lines.drain((sr + 1)..=er);
        }
        self.cursor_row = sr;
        self.cursor_col = sc;
        true
    }

    fn normalize_selection(&self, (sr, sc, er, ec): (usize, usize, usize, usize)) -> (usize, usize, usize, usize) {
        if sr < er || (sr == er && sc <= ec) {
            (sr, sc, er, ec)
        } else {
            (er, ec, sr, sc)
        }
    }

    fn is_in_selection(&self, row: usize, col: usize) -> bool {
        let (sr, sc, er, ec) = match self.selection {
            Some(sel) => self.normalize_selection(sel),
            None => return false,
        };
        if row < sr || row > er { return false; }
        if row == sr && row == er { return col >= sc && col < ec; }
        if row == sr { return col >= sc; }
        if row == er { return col < ec; }
        true
    }

    fn x_to_col(&self, row: usize, rel_x: f32, line_num_width: f32) -> usize {
        if row >= self.lines.len() { return 0; }
        let text_x = rel_x - line_num_width - 12.0 - 4.0; // 4px padding
        if text_x < 0.0 { return 0; }
        let char_width = 7.5_f32;
        let col = (text_x / char_width).round() as usize;
        col.min(self.lines[row].len())
    }
}

impl Focusable for FileView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FileView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ext = self.path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();
        let line_count = self.lines.len();
        let line_num_width = format!("{}", line_count).len().max(3) as f32 * 7.5;
        let is_focused = self.focus_handle.is_focused(window);
        let cursor_row = self.cursor_row;
        let cursor_col = self.cursor_col;
        let modified = self.modified;

        let title_display = if modified {
            format!("{} ●", self.filename)
        } else {
            self.filename.clone()
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .track_focus(&self.focus_handle)
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                if this.selecting {
                    this.selecting = false;
                    if let Some((sr, sc, er, ec)) = this.selection {
                        if sr == er && sc == ec {
                            this.selection = None;
                        }
                    }
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                let shift = ev.keystroke.modifiers.shift;

                // Cmd shortcuts
                if ev.keystroke.modifiers.platform {
                    match ev.keystroke.key.as_str() {
                        "s" => { this.save(cx); cx.notify(); return; }
                        "a" => { this.select_all(); cx.notify(); return; }
                        "c" | "x" => {
                            let text = this.get_selected_text();
                            if !text.is_empty() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                if ev.keystroke.key.as_str() == "x" {
                                    this.delete_selection();
                                    this.mark_modified(cx);
                                }
                            }
                            cx.notify();
                            return;
                        }
                        "v" => {
                            if let Some(item) = cx.read_from_clipboard() {
                                if let Some(text) = item.text() {
                                    this.delete_selection();
                                    for (i, chunk) in text.split('\n').enumerate() {
                                        if i > 0 { this.insert_newline(cx); }
                                        if !chunk.is_empty() { this.insert_char(chunk, cx); }
                                    }
                                }
                            }
                            cx.notify();
                            return;
                        }
                        "z" => {
                            // undo — not implemented yet
                            return;
                        }
                        _ => { return; }
                    }
                }

                let handled = match ev.keystroke.key.as_str() {
                    "enter" => { this.insert_newline(cx); true }
                    "backspace" => { this.backspace(cx); true }
                    "delete" => { this.delete_forward(cx); true }
                    "tab" => { this.insert_tab(cx); true }
                    "space" => { this.insert_char(" ", cx); true }
                    "left" => { this.move_left(shift); true }
                    "right" => { this.move_right(shift); true }
                    "up" => { this.move_up(shift); true }
                    "down" => { this.move_down(shift); true }
                    "home" => { this.move_home(shift); true }
                    "end" => { this.move_end(shift); true }
                    _ => false,
                };
                if !handled {
                    // Try key_char for regular character input (letters, numbers, symbols)
                    if let Some(key_char) = &ev.keystroke.key_char {
                        if !ev.keystroke.modifiers.platform {
                            this.delete_selection();
                            this.insert_char(key_char, cx);
                        }
                    } else if ev.keystroke.key.len() == 1 {
                        // Single char key name (fallback for keys not in key_char)
                        let ch = ev.keystroke.key.clone();
                        this.delete_selection();
                        this.insert_char(&ch, cx);
                    } else {
                        return; // truly unhandled
                    }
                }
                cx.notify();
            }))
            // File header
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .w_full()
                    .h(px(28.))
                    .min_h(px(28.))
                    .flex_shrink_0()
                    .px(px(10.))
                    .border_b_1()
                    .border_color(theme::surface1())
                    .child(
                        div()
                            .text_color(theme::text())
                            .child(title_display),
                    )
                    .child(
                        div()
                            .ml(px(8.))
                            .text_xs()
                            .text_color(theme::overlay())
                            .child(self.path.to_string_lossy().to_string()),
                    ),
            )
            // Content
            .child({
                let origin_ref = self.content_origin.clone();
                div()
                    .id("file-content-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .p(px(4.))
                            // Track content origin for mouse coordinate conversion
                            .child(
                                canvas(
                                    move |bounds, _window, _cx| {
                                        let x: f32 = bounds.origin.x.into();
                                        let y: f32 = bounds.origin.y.into();
                                        origin_ref.set((x, y));
                                    },
                                    |_, _: (), _, _| {},
                                )
                                .absolute()
                                .size_0()
                            )
                            .children(
                                (0..line_count).map(|row| {
                                    let line = &self.lines[row];
                                    let is_cursor_row = row == cursor_row && is_focused;

                                    let mut line_el = div()
                                        .id(ElementId::Name(format!("line-{}", row).into()))
                                        .flex()
                                        .flex_row()
                                        .w_full()
                                        .min_h(px(20.))
                                        .when(is_cursor_row, |d| d.bg(theme::surface0()))
                                        .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                                            this.focus_handle.focus(window);
                                            let x: f32 = ev.position.x.into();
                                            let (ox, _) = this.content_origin.get();
                                            let col = this.x_to_col(row, x - ox, line_num_width);
                                            this.cursor_row = row;
                                            this.cursor_col = col;
                                            if ev.modifiers.shift {
                                                this.extend_selection();
                                            } else {
                                                this.selection = Some((row, col, row, col));
                                                this.selecting = true;
                                            }
                                            cx.notify();
                                        }))
                                        .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _window, cx| {
                                            if this.selecting && ev.pressed_button == Some(MouseButton::Left) {
                                                let x: f32 = ev.position.x.into();
                                                let (ox, _) = this.content_origin.get();
                                                let col = this.x_to_col(row, x - ox, line_num_width);
                                                this.cursor_row = row;
                                                this.cursor_col = col;
                                                this.extend_selection();
                                                cx.notify();
                                            }
                                        }));

                                    // Line number
                                    line_el = line_el.child(
                                        div()
                                            .w(px(line_num_width + 12.))
                                            .flex_shrink_0()
                                            .text_right()
                                            .pr(px(12.))
                                            .text_color(if is_cursor_row { theme::text() } else { theme::overlay() })
                                            .child(format!("{}", row + 1)),
                                    );

                                    // Line content with cursor
                                    let mut content_children: Vec<Div> = Vec::new();

                                    if line.is_empty() {
                                        // Empty line — just show cursor if needed
                                        if is_cursor_row {
                                            content_children.push(
                                                div()
                                                    .w(px(2.))
                                                    .h(px(16.))
                                                    .bg(theme::blue())
                                            );
                                        }
                                    } else {
                                        // Render char by char with selection/cursor
                                        let chars: Vec<(usize, char)> = line.char_indices().collect();
                                        let mut i = 0;
                                        while i < chars.len() {
                                            let (byte_idx, _) = chars[i];

                                            // Check if cursor is here
                                            if is_cursor_row && byte_idx == cursor_col {
                                                content_children.push(
                                                    div()
                                                        .w(px(2.))
                                                        .h(px(16.))
                                                        .flex_shrink_0()
                                                        .bg(theme::blue())
                                                );
                                            }

                                            // Build a run of chars with same selection state
                                            let in_sel = self.is_in_selection(row, byte_idx);
                                            let mut run_end = i + 1;
                                            while run_end < chars.len() {
                                                let (next_byte, _) = chars[run_end];
                                                if self.is_in_selection(row, next_byte) != in_sel { break; }
                                                if is_cursor_row && next_byte == cursor_col { break; }
                                                run_end += 1;
                                            }

                                            let start_byte = chars[i].0;
                                            let end_byte = if run_end < chars.len() { chars[run_end].0 } else { line.len() };
                                            let text_chunk = &line[start_byte..end_byte];

                                            // Replace spaces with NBSP for display (prevents whitespace collapse)
                                            let display_chunk = text_chunk.replace(' ', "\u{00A0}");
                                            let chunk_el = div()
                                                .text_color(if in_sel { theme::base() } else { theme::text() })
                                                .when(in_sel, |d| d.bg(theme::blue()))
                                                .child(display_chunk);
                                            content_children.push(chunk_el);
                                            i = run_end;
                                        }

                                        // Cursor at end of line
                                        if is_cursor_row && cursor_col >= line.len() {
                                            content_children.push(
                                                div()
                                                    .w(px(2.))
                                                    .h(px(16.))
                                                    .flex_shrink_0()
                                                    .bg(theme::blue())
                                            );
                                        }
                                    }

                                    line_el = line_el.child(
                                        div()
                                            .flex_1()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .children(content_children),
                                    );

                                    line_el
                                }).collect::<Vec<_>>()
                            ),
                    )
            })
    }
}
