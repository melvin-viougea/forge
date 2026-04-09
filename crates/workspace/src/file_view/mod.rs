pub mod buffer;
pub mod highlight;

use gpui::*;
use gpui::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::theme;
use buffer::Buffer;
use highlight::{LangDef, Token, TokenKind, lang_for_ext, lang_name, token_color, tokenize_line};

pub enum FileViewEvent {
    TitleChanged(String),
}

impl gpui::EventEmitter<FileViewEvent> for FileView {}

#[derive(Clone, Copy, PartialEq)]
pub enum DiffLineMarker {
    Added,
    Deleted,
    Fold,
}

pub struct FileView {
    path: PathBuf,
    buffer: Buffer,
    filename: String,
    ext: String,
    cursor_row: usize,
    cursor_col: usize,
    modified: bool,
    focus_handle: FocusHandle,
    selection: Option<(usize, usize, usize, usize)>,
    selecting: bool,
    // Syntax
    lang: Option<&'static LangDef>,
    token_cache: Vec<Option<Vec<Token>>>,
    block_comment_state: Vec<bool>,
    // Find
    find_open: bool,
    find_query: String,
    find_matches: Vec<(usize, usize, usize)>, // (row, start_col, end_col)
    find_current: usize,
    find_focus: FocusHandle,
    // Bracket matching
    matched_bracket: Option<(usize, usize)>,
    // Scroll
    scroll_handle: ScrollHandle,
    pub scroll_x: f32,
    // Auto-scroll during selection drag
    auto_scroll_speed: f32,
    auto_scroll_task: Option<Task<()>>,
    // Diff support
    pub readonly: bool,
    pub compact: bool,
    pub diff_markers: HashMap<usize, DiffLineMarker>,
    pub line_numbers: Option<Vec<u32>>, // custom line numbers (0 = hidden)
}

impl FileView {
    pub fn new(path: PathBuf, cx: &mut Context<Self>) -> Self {
        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let ext = path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let buffer = Buffer::new(&content);
        let line_count = buffer.line_count();

        let lang = lang_for_ext(&ext);

        // Precompute token cache + block comment state
        let mut token_cache: Vec<Option<Vec<Token>>> = Vec::with_capacity(line_count);
        let mut block_comment_state = vec![false; line_count + 1];
        if let Some(lang_def) = lang {
            let mut in_bc = false;
            for row in 0..line_count {
                block_comment_state[row] = in_bc;
                let (tokens, new_bc) = tokenize_line(buffer.line(row), lang_def, in_bc);
                in_bc = new_bc;
                token_cache.push(Some(tokens));
            }
            block_comment_state.push(in_bc); // sentinel
        } else {
            for _ in 0..line_count {
                token_cache.push(None);
            }
        }

        let focus_handle = cx.focus_handle();
        let find_focus = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();

        Self {
            path,
            buffer,
            filename,
            ext,
            cursor_row: 0,
            cursor_col: 0,
            modified: false,
            focus_handle,
            selection: None,
            selecting: false,
            lang,
            token_cache,
            block_comment_state,
            find_open: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_current: 0,
            find_focus,
            matched_bracket: None,
            scroll_handle,
            scroll_x: 0.0,
            auto_scroll_speed: 0.0,
            auto_scroll_task: None,
            readonly: false,
            compact: false,
            diff_markers: HashMap::new(),
            line_numbers: None,
        }
    }

    /// Create a FileView from string content (e.g. HEAD version for diff).
    pub fn new_with_content(path: PathBuf, content: String, readonly: bool, compact: bool, cx: &mut Context<Self>) -> Self {
        let filename = path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = path.extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let buffer = Buffer::new(&content);
        let line_count = buffer.line_count();
        let lang = lang_for_ext(&ext);

        let mut token_cache: Vec<Option<Vec<Token>>> = Vec::with_capacity(line_count);
        let mut block_comment_state = vec![false; line_count + 1];
        if let Some(lang_def) = lang {
            let mut in_bc = false;
            for row in 0..line_count {
                block_comment_state[row] = in_bc;
                let (tokens, new_bc) = tokenize_line(buffer.line(row), lang_def, in_bc);
                in_bc = new_bc;
                token_cache.push(Some(tokens));
            }
            block_comment_state.push(in_bc);
        } else {
            for _ in 0..line_count {
                token_cache.push(None);
            }
        }

        let focus_handle = cx.focus_handle();
        let find_focus = cx.focus_handle();
        let scroll_handle = ScrollHandle::new();

        Self {
            path,
            buffer,
            filename,
            ext,
            cursor_row: 0,
            cursor_col: 0,
            modified: false,
            focus_handle,
            selection: None,
            selecting: false,
            lang,
            token_cache,
            block_comment_state,
            find_open: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_current: 0,
            find_focus,
            matched_bracket: None,
            scroll_handle,
            scroll_x: 0.0,
            auto_scroll_speed: 0.0,
            auto_scroll_task: None,
            readonly,
            compact,
            diff_markers: HashMap::new(),
            line_numbers: None,
        }
    }

    pub fn path(&self) -> &PathBuf { &self.path }
    pub fn filename(&self) -> &str { &self.filename }

    // ── Token cache ────────────────────────────────

    fn invalidate_tokens(&mut self, from_row: usize) {
        if self.lang.is_none() { return; }
        // Resize cache to match buffer
        self.token_cache.resize(self.buffer.line_count(), None);
        self.block_comment_state.resize(self.buffer.line_count() + 1, false);
        // Invalidate from `from_row` onward
        for i in from_row..self.token_cache.len() {
            self.token_cache[i] = None;
        }
    }

    fn ensure_tokens(&mut self, row: usize) {
        if row >= self.buffer.line_count() { return; }
        if self.token_cache.len() <= row {
            self.token_cache.resize(row + 1, None);
        }
        if self.block_comment_state.len() <= row + 1 {
            self.block_comment_state.resize(row + 2, false);
        }
        if self.token_cache[row].is_some() { return; }

        let lang_def = match self.lang {
            Some(l) => l,
            None => return,
        };

        // Need to recalculate block_comment_state from last known point
        let mut start = row;
        while start > 0 && self.token_cache[start - 1].is_none() {
            start -= 1;
        }

        for r in start..=row {
            if self.token_cache.len() <= r { self.token_cache.push(None); }
            if self.block_comment_state.len() <= r + 1 { self.block_comment_state.push(false); }

            let in_bc = self.block_comment_state[r];
            let (tokens, new_bc) = tokenize_line(self.buffer.line(r), lang_def, in_bc);
            self.token_cache[r] = Some(tokens);
            self.block_comment_state[r + 1] = new_bc;
        }
    }

    fn get_tokens(&mut self, row: usize) -> Option<Vec<Token>> {
        self.ensure_tokens(row);
        self.token_cache.get(row).cloned().flatten()
    }

    // ── Modifications ──────────────────────────────

    fn mark_modified(&mut self, cx: &mut Context<Self>) {
        if !self.modified {
            self.modified = true;
            cx.emit(FileViewEvent::TitleChanged(format!("{} \u{25cf}", self.filename)));
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let content = self.buffer.content();
        if std::fs::write(&self.path, &content).is_ok() {
            self.modified = false;
            cx.emit(FileViewEvent::TitleChanged(self.filename.clone()));
        }
    }

    fn ensure_cursor_bounds(&mut self) {
        if self.cursor_row >= self.buffer.line_count() {
            self.cursor_row = self.buffer.line_count().saturating_sub(1);
        }
        let line_len = self.buffer.line(self.cursor_row).len();
        if self.cursor_col > line_len {
            self.cursor_col = line_len;
        }
    }

    fn insert_char(&mut self, ch: &str, cx: &mut Context<Self>) {
        self.delete_selection();
        self.ensure_cursor_bounds();
        self.buffer.insert_text(self.cursor_row, self.cursor_col, ch);
        self.cursor_col += ch.len();
        self.invalidate_tokens(self.cursor_row);
        self.mark_modified(cx);
    }

    fn insert_newline(&mut self, cx: &mut Context<Self>) {
        self.delete_selection();
        self.ensure_cursor_bounds();
        let (r, c) = self.buffer.insert_newline_with_indent(self.cursor_row, self.cursor_col);
        self.cursor_row = r;
        self.cursor_col = c;
        self.invalidate_tokens(self.cursor_row.saturating_sub(1));
        self.mark_modified(cx);
    }

    fn backspace(&mut self, cx: &mut Context<Self>) {
        if self.delete_selection() {
            self.mark_modified(cx);
            return;
        }
        self.ensure_cursor_bounds();
        if self.cursor_col > 0 {
            let line = self.buffer.line(self.cursor_row);
            let prev = line[..self.cursor_col].char_indices().last().map(|(i, _)| i).unwrap_or(0);
            self.buffer.delete_range(self.cursor_row, prev, self.cursor_row, self.cursor_col);
            self.cursor_col = prev;
            self.invalidate_tokens(self.cursor_row);
            self.mark_modified(cx);
        } else if self.cursor_row > 0 {
            let prev_len = self.buffer.line(self.cursor_row - 1).len();
            self.buffer.delete_range(self.cursor_row - 1, prev_len, self.cursor_row, 0);
            self.cursor_row -= 1;
            self.cursor_col = prev_len;
            self.invalidate_tokens(self.cursor_row);
            self.mark_modified(cx);
        }
    }

    fn delete_forward(&mut self, cx: &mut Context<Self>) {
        if self.delete_selection() {
            self.mark_modified(cx);
            return;
        }
        self.ensure_cursor_bounds();
        let line = self.buffer.line(self.cursor_row);
        if self.cursor_col < line.len() {
            let next = line[self.cursor_col..].char_indices().nth(1).map(|(i, _)| self.cursor_col + i).unwrap_or(line.len());
            self.buffer.delete_range(self.cursor_row, self.cursor_col, self.cursor_row, next);
            self.invalidate_tokens(self.cursor_row);
            self.mark_modified(cx);
        } else if self.cursor_row + 1 < self.buffer.line_count() {
            self.buffer.delete_range(self.cursor_row, self.cursor_col, self.cursor_row + 1, 0);
            self.invalidate_tokens(self.cursor_row);
            self.mark_modified(cx);
        }
    }

    fn insert_tab(&mut self, cx: &mut Context<Self>) {
        // Multi-line selection → indent
        if let Some((sr, _, er, _)) = self.selection.map(|s| self.normalize_sel(s)) {
            if sr != er {
                self.indent_lines(sr, er, cx);
                return;
            }
        }
        self.insert_char("    ", cx);
    }

    fn indent_lines(&mut self, sr: usize, er: usize, cx: &mut Context<Self>) {
        for row in sr..=er {
            self.buffer.insert_text(row, 0, "    ");
        }
        if let Some((s_r, s_c, e_r, e_c)) = self.selection {
            self.selection = Some((s_r, s_c + 4, e_r, e_c + 4));
        }
        self.cursor_col += 4;
        self.invalidate_tokens(sr);
        self.mark_modified(cx);
    }

    fn dedent_lines(&mut self, cx: &mut Context<Self>) {
        let (sr, _, er, _) = match self.selection.map(|s| self.normalize_sel(s)) {
            Some(s) if s.0 != s.2 => s,
            _ => {
                // Single line dedent
                let row = self.cursor_row;
                let line = self.buffer.line(row);
                let spaces = line.chars().take(4).take_while(|c| *c == ' ').count();
                if spaces > 0 {
                    self.buffer.delete_range(row, 0, row, spaces);
                    self.cursor_col = self.cursor_col.saturating_sub(spaces);
                    self.invalidate_tokens(row);
                    self.mark_modified(cx);
                }
                return;
            }
        };
        for row in sr..=er {
            let line = self.buffer.line(row);
            let spaces = line.chars().take(4).take_while(|c| *c == ' ').count();
            if spaces > 0 {
                self.buffer.delete_range(row, 0, row, spaces);
            }
        }
        self.cursor_col = self.cursor_col.saturating_sub(4);
        self.invalidate_tokens(sr);
        self.mark_modified(cx);
    }

    fn comment_toggle(&mut self, cx: &mut Context<Self>) {
        let prefix = match self.ext.as_str() {
            "rs" | "js" | "jsx" | "ts" | "tsx" | "c" | "cpp" | "go" | "java" | "swift" | "mjs" | "cjs" => "//",
            "py" | "rb" | "sh" | "bash" | "zsh" | "yaml" | "yml" | "toml" => "#",
            _ => "//",
        };

        let (sr, er) = if let Some(sel) = self.selection {
            let n = self.normalize_sel(sel);
            (n.0, n.2)
        } else {
            (self.cursor_row, self.cursor_row)
        };

        // Check if all lines are commented
        let all_commented = (sr..=er).all(|r| {
            self.buffer.line(r).trim_start().starts_with(prefix)
        });

        let prefix_space = format!("{} ", prefix);
        for row in sr..=er {
            if all_commented {
                // Remove comment
                let line = self.buffer.line(row);
                let ws: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                let rest = line[ws.len()..].to_string();
                if rest.starts_with(&prefix_space) {
                    self.buffer.delete_range(row, ws.len(), row, ws.len() + prefix_space.len());
                } else if rest.starts_with(prefix) {
                    self.buffer.delete_range(row, ws.len(), row, ws.len() + prefix.len());
                }
            } else {
                // Add comment
                let line = self.buffer.line(row);
                let ws_len: usize = line.chars().take_while(|c| c.is_whitespace()).count();
                self.buffer.insert_text(row, ws_len, &prefix_space);
            }
        }
        self.invalidate_tokens(sr);
        self.mark_modified(cx);
    }

    fn duplicate_line(&mut self, cx: &mut Context<Self>) {
        self.buffer.duplicate_line(self.cursor_row);
        self.cursor_row += 1;
        self.invalidate_tokens(self.cursor_row.saturating_sub(1));
        self.mark_modified(cx);
    }

    // ── Movement ───────────────────────────────────

    fn move_left(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        if self.cursor_col > 0 {
            let line = self.buffer.line(self.cursor_row);
            self.cursor_col = line[..self.cursor_col].char_indices().last().map(|(i, _)| i).unwrap_or(0);
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.buffer.line(self.cursor_row).len();
        }
        if shift { self.extend_selection(); }
    }

    fn move_right(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        self.ensure_cursor_bounds();
        let line = self.buffer.line(self.cursor_row);
        if self.cursor_col < line.len() {
            self.cursor_col = line[self.cursor_col..].char_indices().nth(1).map(|(i, _)| self.cursor_col + i).unwrap_or(line.len());
        } else if self.cursor_row + 1 < self.buffer.line_count() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
        if shift { self.extend_selection(); }
    }

    fn move_up(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        if self.cursor_row > 0 { self.cursor_row -= 1; self.ensure_cursor_bounds(); }
        if shift { self.extend_selection(); }
    }

    fn move_down(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        if self.cursor_row + 1 < self.buffer.line_count() { self.cursor_row += 1; self.ensure_cursor_bounds(); }
        if shift { self.extend_selection(); }
    }

    fn move_home(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        // Smart home: go to first non-whitespace, or to 0 if already there
        let line = self.buffer.line(self.cursor_row);
        let first_non_ws = line.chars().take_while(|c| c.is_whitespace()).count();
        self.cursor_col = if self.cursor_col == first_non_ws { 0 } else { first_non_ws };
        if shift { self.extend_selection(); }
    }

    fn move_end(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        self.ensure_cursor_bounds();
        self.cursor_col = self.buffer.line(self.cursor_row).len();
        if shift { self.extend_selection(); }
    }

    fn move_word_left(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        if self.cursor_col == 0 {
            if self.cursor_row > 0 {
                self.cursor_row -= 1;
                self.cursor_col = self.buffer.line(self.cursor_row).len();
            }
        } else {
            let line = self.buffer.line(self.cursor_row);
            let bytes = line.as_bytes();
            let mut c = self.cursor_col;
            // Skip whitespace backwards
            while c > 0 && bytes[c - 1].is_ascii_whitespace() { c -= 1; }
            // Skip word chars backwards
            if c > 0 && (bytes[c - 1].is_ascii_alphanumeric() || bytes[c - 1] == b'_') {
                while c > 0 && (bytes[c - 1].is_ascii_alphanumeric() || bytes[c - 1] == b'_') { c -= 1; }
            } else if c > 0 {
                c -= 1; // Skip single punctuation
            }
            self.cursor_col = c;
        }
        if shift { self.extend_selection(); }
    }

    fn move_word_right(&mut self, shift: bool) {
        if shift { self.start_or_extend_selection(); } else { self.selection = None; }
        let line = self.buffer.line(self.cursor_row);
        if self.cursor_col >= line.len() {
            if self.cursor_row + 1 < self.buffer.line_count() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
        } else {
            let bytes = line.as_bytes();
            let mut c = self.cursor_col;
            // Skip word chars
            if c < bytes.len() && (bytes[c].is_ascii_alphanumeric() || bytes[c] == b'_') {
                while c < bytes.len() && (bytes[c].is_ascii_alphanumeric() || bytes[c] == b'_') { c += 1; }
            } else if c < bytes.len() {
                c += 1;
            }
            // Skip whitespace
            while c < bytes.len() && bytes[c].is_ascii_whitespace() { c += 1; }
            self.cursor_col = c;
        }
        if shift { self.extend_selection(); }
    }

    fn delete_word_left(&mut self, cx: &mut Context<Self>) {
        if self.delete_selection() { self.mark_modified(cx); return; }
        let start_col = self.cursor_col;
        self.move_word_left(false);
        if self.cursor_col < start_col {
            self.buffer.delete_range(self.cursor_row, self.cursor_col, self.cursor_row, start_col);
            self.invalidate_tokens(self.cursor_row);
            self.mark_modified(cx);
        }
    }

    // ── Selection ──────────────────────────────────

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
        let last_row = self.buffer.line_count().saturating_sub(1);
        let last_col = self.buffer.line(last_row).len();
        self.selection = Some((0, 0, last_row, last_col));
        self.cursor_row = last_row;
        self.cursor_col = last_col;
    }

    fn get_selected_text(&self) -> String {
        let (sr, sc, er, ec) = match self.selection {
            Some(sel) => self.normalize_sel(sel),
            None => return String::new(),
        };
        if sr == er { return self.buffer.line(sr)[sc..ec].to_string(); }
        let mut result = self.buffer.line(sr)[sc..].to_string();
        result.push('\n');
        for row in (sr + 1)..er {
            result.push_str(self.buffer.line(row));
            result.push('\n');
        }
        result.push_str(&self.buffer.line(er)[..ec]);
        result
    }

    fn delete_selection(&mut self) -> bool {
        let sel = match self.selection {
            Some(sel) => sel,
            None => return false,
        };
        let (sr, sc, er, ec) = self.normalize_sel(sel);
        self.selection = None;
        self.buffer.delete_range(sr, sc, er, ec);
        self.cursor_row = sr;
        self.cursor_col = sc;
        self.invalidate_tokens(sr);
        true
    }

    fn normalize_sel(&self, (sr, sc, er, ec): (usize, usize, usize, usize)) -> (usize, usize, usize, usize) {
        if sr < er || (sr == er && sc <= ec) { (sr, sc, er, ec) } else { (er, ec, sr, sc) }
    }

    fn is_in_selection(&self, row: usize, col: usize) -> bool {
        let (sr, sc, er, ec) = match self.selection {
            Some(sel) => self.normalize_sel(sel),
            None => return false,
        };
        if row < sr || row > er { return false; }
        if row == sr && row == er { return col >= sc && col < ec; }
        if row == sr { return col >= sc; }
        if row == er { return col < ec; }
        true
    }

    // ── Bracket matching ───────────────────────────

    fn update_bracket_match(&mut self) {
        self.matched_bracket = None;
        self.ensure_cursor_bounds();
        let line = self.buffer.line(self.cursor_row);
        if self.cursor_col >= line.len() { return; }

        let ch = line.as_bytes()[self.cursor_col];
        let (forward, open, close) = match ch {
            b'{' => (true, b'{', b'}'),
            b'(' => (true, b'(', b')'),
            b'[' => (true, b'[', b']'),
            b'}' => (false, b'{', b'}'),
            b')' => (false, b'(', b')'),
            b']' => (false, b'[', b']'),
            _ => return,
        };

        if forward {
            let mut depth = 0i32;
            for row in self.cursor_row..self.buffer.line_count() {
                let l = self.buffer.line(row).as_bytes();
                let start = if row == self.cursor_row { self.cursor_col } else { 0 };
                for col in start..l.len() {
                    if l[col] == open { depth += 1; }
                    if l[col] == close { depth -= 1; }
                    if depth == 0 { self.matched_bracket = Some((row, col)); return; }
                }
            }
        } else {
            let mut depth = 0i32;
            for row in (0..=self.cursor_row).rev() {
                let l = self.buffer.line(row).as_bytes();
                let end = if row == self.cursor_row { self.cursor_col } else { l.len().saturating_sub(1) };
                for col in (0..=end).rev() {
                    if col >= l.len() { continue; }
                    if l[col] == close { depth += 1; }
                    if l[col] == open { depth -= 1; }
                    if depth == 0 { self.matched_bracket = Some((row, col)); return; }
                }
            }
        }
    }

    // ── Find ───────────────────────────────────────

    fn find_update_matches(&mut self) {
        self.find_matches.clear();
        if self.find_query.is_empty() { return; }
        let query_lower = self.find_query.to_lowercase();
        for row in 0..self.buffer.line_count() {
            let line_lower = self.buffer.line(row).to_lowercase();
            let mut start = 0;
            while let Some(pos) = line_lower[start..].find(&query_lower) {
                let abs = start + pos;
                self.find_matches.push((row, abs, abs + self.find_query.len()));
                start = abs + 1;
            }
        }
        if self.find_current >= self.find_matches.len() {
            self.find_current = 0;
        }
    }

    fn find_goto_match(&mut self) {
        if let Some(&(row, col, _)) = self.find_matches.get(self.find_current) {
            self.cursor_row = row;
            self.cursor_col = col;
            self.scroll_to_cursor();
        }
    }

    fn find_next(&mut self) {
        if self.find_matches.is_empty() { return; }
        self.find_current = (self.find_current + 1) % self.find_matches.len();
        self.find_goto_match();
    }

    fn find_prev(&mut self) {
        if self.find_matches.is_empty() { return; }
        self.find_current = if self.find_current == 0 { self.find_matches.len() - 1 } else { self.find_current - 1 };
        self.find_goto_match();
    }

    fn is_find_match(&self, row: usize, col: usize) -> Option<bool> {
        // Returns Some(true) for current match, Some(false) for other matches, None for no match
        for (i, &(mr, ms, me)) in self.find_matches.iter().enumerate() {
            if mr == row && col >= ms && col < me {
                return Some(i == self.find_current);
            }
        }
        None
    }

    // ── Mouse helpers ──────────────────────────────

    fn x_to_col(&self, row: usize, text_x: f32) -> usize {
        if row >= self.buffer.line_count() { return 0; }
        if text_x < 0.0 { return 0; }
        let char_width = 7.5_f32;
        let col = (text_x / char_width).round() as usize;
        col.min(self.buffer.line(row).len())
    }

    // ── Scroll to cursor ───────────────────────────

    fn scroll_to_cursor(&self) {
        let line_height = 20.0_f32;
        let padding = 4.0_f32;
        let cursor_y = self.cursor_row as f32 * line_height + padding;
        let offset = self.scroll_handle.offset();
        let viewport_h: f32 = self.scroll_handle.bounds().size.height.into();
        let scroll_top = -f32::from(offset.y);

        if cursor_y < scroll_top {
            // Cursor is above viewport
            self.scroll_handle.set_offset(point(px(0.), px(-cursor_y)));
        } else if cursor_y + line_height > scroll_top + viewport_h {
            // Cursor is below viewport
            self.scroll_handle.set_offset(point(px(0.), px(-(cursor_y + line_height - viewport_h))));
        }
    }

    // ── Auto-scroll during selection drag ──────────

    fn start_auto_scroll(&mut self, cx: &mut Context<Self>) {
        if self.auto_scroll_task.is_some() { return; }
        let task = cx.spawn(async |entity: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(Duration::from_millis(40)).await;
                let should_continue = entity.update(cx, |this, cx| {
                    if !this.selecting || this.auto_scroll_speed == 0.0 {
                        return false;
                    }
                    let lines = this.auto_scroll_speed.abs().ceil() as usize;
                    let max_row = this.buffer.line_count().saturating_sub(1);

                    if this.auto_scroll_speed > 0.0 {
                        this.cursor_row = (this.cursor_row + lines).min(max_row);
                        this.cursor_col = this.buffer.line(this.cursor_row).len();
                    } else {
                        this.cursor_row = this.cursor_row.saturating_sub(lines);
                        this.cursor_col = 0;
                    }
                    this.extend_selection();
                    this.scroll_to_cursor();
                    cx.notify();
                    true
                }).unwrap_or(false);
                if !should_continue { break; }
            }
        });
        self.auto_scroll_task = Some(task);
    }

    fn stop_auto_scroll(&mut self) {
        self.auto_scroll_speed = 0.0;
        self.auto_scroll_task = None;
    }

    // ── Line rendering ─────────────────────────────

    fn render_line(&mut self, row: usize, line_num_width: f32, is_focused: bool, cx: &mut Context<Self>) -> Stateful<Div> {
        // Fold separator — thin line instead of a full row
        if self.diff_markers.get(&row) == Some(&DiffLineMarker::Fold) {
            return div()
                .id(ElementId::Name(format!("line-{}", row).into()))
                .w_full()
                .h(px(6.))
                .flex().items_center()
                .child(div().w_full().h(px(1.)).bg(theme::surface1()));
        }

        let line = self.buffer.line(row).to_string();
        let is_cursor_row = row == self.cursor_row && is_focused && !self.readonly;
        let cursor_col = self.cursor_col;

        // Get tokens
        let tokens = self.get_tokens(row);

        let diff_marker = self.diff_markers.get(&row).copied();
        let diff_bg = match diff_marker {
            Some(DiffLineMarker::Deleted) => Some(rgb(0x2a1518)),
            Some(DiffLineMarker::Added)   => Some(rgb(0x152a18)),
            _ => None,
        };

        let scroll_x = self.scroll_x;
        let mut line_el = div()
            .id(ElementId::Name(format!("line-{}", row).into()))
            .flex()
            .flex_row()
            .w_full()
            .h(px(20.))
            .when_some(diff_bg, |d: Stateful<Div>, bg| d.bg(bg))
            .when(is_cursor_row && !self.readonly, |d| d.bg(theme::surface0()));

        // Mouse handlers (disabled in readonly mode)
        if !self.readonly {
        line_el = line_el
            .on_mouse_down(MouseButton::Left, cx.listener(move |this, ev: &MouseDownEvent, window, cx| {
                this.focus_handle.focus(window);
                let x: f32 = ev.position.x.into();
                // Approximate: line_num_width + 12px padding + 4px content padding
                let text_x = x - line_num_width - 16.0;
                let col = this.x_to_col(row, text_x);
                this.cursor_row = row;
                this.cursor_col = col;
                if ev.modifiers.shift {
                    this.extend_selection();
                } else {
                    this.selection = Some((row, col, row, col));
                    this.selecting = true;
                }
                this.update_bracket_match();
                cx.notify();
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _window, cx| {
                if this.selecting && ev.pressed_button == Some(MouseButton::Left) {
                    let x: f32 = ev.position.x.into();
                    let text_x = x - line_num_width - 16.0;
                    let col = this.x_to_col(row, text_x);
                    this.cursor_row = row;
                    this.cursor_col = col;
                    this.extend_selection();
                    cx.notify();
                }
            }));
        }

        // Diff marker bar (3px)
        if matches!(diff_marker, Some(DiffLineMarker::Added) | Some(DiffLineMarker::Deleted)) {
            let marker_color = match diff_marker {
                Some(DiffLineMarker::Deleted) => theme::red(),
                Some(DiffLineMarker::Added)   => theme::green(),
                _ => unreachable!(),
            };
            line_el = line_el.child(
                div().w(px(3.)).h(px(20.)).flex_shrink_0().bg(marker_color),
            );
        }

        // Line number (use custom map if available)
        let display_num = if let Some(ref nums) = self.line_numbers {
            let n = nums.get(row).copied().unwrap_or(0);
            if n > 0 { format!("{}", n) } else { String::new() }
        } else {
            format!("{}", row + 1)
        };
        let num_color = match diff_marker {
            Some(DiffLineMarker::Deleted) => theme::red(),
            Some(DiffLineMarker::Added)   => theme::green(),
            _ => if is_cursor_row { theme::text() } else { theme::overlay() },
        };
        let num_width = if diff_marker.is_some() { line_num_width + 9. } else { line_num_width + 12. };
        line_el = line_el.child(
            div()
                .w(px(num_width))
                .flex_shrink_0()
                .text_right()
                .pr(px(12.))
                .text_color(num_color)
                .child(display_num),
        );

        // Line content
        let mut content_children: Vec<AnyElement> = Vec::new();

        if line.is_empty() {
            if is_cursor_row {
                content_children.push(
                    div().w(px(2.)).h(px(16.)).bg(theme::blue()).into_any_element()
                );
            }
        } else {
            let chars: Vec<(usize, char)> = line.char_indices().collect();
            let mut i = 0;
            while i < chars.len() {
                let (byte_idx, _) = chars[i];

                // Cursor
                if is_cursor_row && byte_idx == cursor_col {
                    content_children.push(
                        div().w(px(2.)).h(px(16.)).flex_shrink_0().bg(theme::blue()).into_any_element()
                    );
                }

                // Build a run of chars with same styling
                let in_sel = self.is_in_selection(row, byte_idx);
                let tok_kind = Self::token_kind_at(&tokens, byte_idx);
                let find_match = if self.find_open { self.is_find_match(row, byte_idx) } else { None };
                let is_bracket = self.matched_bracket == Some((row, byte_idx))
                    || (is_cursor_row && byte_idx == cursor_col && self.matched_bracket.is_some());

                let mut run_end = i + 1;
                while run_end < chars.len() {
                    let (next_byte, _) = chars[run_end];
                    if self.is_in_selection(row, next_byte) != in_sel { break; }
                    if Self::token_kind_at(&tokens, next_byte) != tok_kind { break; }
                    if is_cursor_row && next_byte == cursor_col { break; }
                    if self.find_open && self.is_find_match(row, next_byte) != find_match { break; }
                    let next_bracket = self.matched_bracket == Some((row, next_byte));
                    if next_bracket != is_bracket { break; }
                    run_end += 1;
                }

                let start_byte = chars[i].0;
                let end_byte = if run_end < chars.len() { chars[run_end].0 } else { line.len() };
                let display = line[start_byte..end_byte].replace(' ', "\u{00A0}");

                let text_color = if in_sel {
                    theme::base()
                } else {
                    token_color(tok_kind)
                };

                let mut chunk = div()
                    .text_color(text_color)
                    .when(in_sel, |d| d.bg(theme::selection()))
                    .when(is_bracket, |d| d.bg(theme::surface1()).rounded(px(2.)));

                // Find match highlight
                if let Some(is_current) = find_match {
                    chunk = chunk.bg(if is_current {
                        theme::yellow()
                    } else {
                        theme::surface1()
                    });
                    if is_current {
                        chunk = chunk.text_color(theme::base());
                    }
                }

                content_children.push(chunk.child(display).into_any_element());
                i = run_end;
            }

            // Cursor at end of line
            if is_cursor_row && cursor_col >= line.len() {
                content_children.push(
                    div().w(px(2.)).h(px(16.)).flex_shrink_0().bg(theme::blue()).into_any_element()
                );
            }
        }

        line_el = line_el.child(
            div()
                .flex_1().min_w(px(0.)).overflow_hidden()
                .child(
                    div()
                        .flex_shrink_0().flex().flex_row().items_center()
                        .pl(px(4.))
                        .ml(px(-scroll_x))
                        .children(content_children),
                ),
        );

        line_el
    }

    fn token_kind_at(tokens: &Option<Vec<Token>>, byte_idx: usize) -> TokenKind {
        if let Some(toks) = tokens {
            for t in toks {
                if byte_idx >= t.start && byte_idx < t.end {
                    return t.kind;
                }
            }
        }
        TokenKind::Plain
    }
}

impl Focusable for FileView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FileView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let line_count = self.buffer.line_count();
        let max_line_num = if let Some(ref nums) = self.line_numbers {
            nums.iter().copied().max().unwrap_or(line_count as u32) as usize
        } else {
            line_count
        };
        let line_num_width = format!("{}", max_line_num).len().max(3) as f32 * 9.0;
        let is_focused = self.focus_handle.is_focused(window);
        let cursor_row = self.cursor_row;
        let cursor_col = self.cursor_col;
        let modified = self.modified;

        let title_display = if modified {
            format!("{} \u{25cf}", self.filename)
        } else {
            self.filename.clone()
        };

        let lang_label = lang_name(&self.ext);

        // Find bar state
        let find_open = self.find_open;
        let find_query = self.find_query.clone();
        let find_count = self.find_matches.len();
        let find_current = self.find_current;

        self.update_bracket_match();

        let compact = self.compact;

        div()
            .flex()
            .flex_col()
            .when(!compact, |d: Div| d.size_full())
            .when(compact, |d: Div| d.w_full())
            .text_sm()
            .font_family("Berkeley Mono, SF Mono, Menlo, monospace")
            .track_focus(&self.focus_handle)
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _ev: &MouseUpEvent, _window, cx| {
                if this.selecting {
                    this.selecting = false;
                    this.stop_auto_scroll();
                    if let Some((sr, sc, er, ec)) = this.selection {
                        if sr == er && sc == ec { this.selection = None; }
                    }
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(move |this, ev: &MouseMoveEvent, _window, cx| {
                if this.selecting && ev.pressed_button == Some(MouseButton::Left) {
                    let mouse_y: f32 = ev.position.y.into();
                    let scroll_bounds = this.scroll_handle.bounds();
                    let top: f32 = scroll_bounds.top().into();
                    let bottom: f32 = scroll_bounds.bottom().into();
                    let zone = 80.0_f32;

                    if mouse_y > bottom - zone {
                        let distance = (mouse_y - (bottom - zone)).max(0.0);
                        let ratio = (distance / zone).min(1.0);
                        this.auto_scroll_speed = 1.0 + ratio * 9.0;
                        this.start_auto_scroll(cx);
                    } else if mouse_y < top + zone {
                        let distance = ((top + zone) - mouse_y).max(0.0);
                        let ratio = (distance / zone).min(1.0);
                        this.auto_scroll_speed = -(1.0 + ratio * 9.0);
                        this.start_auto_scroll(cx);
                    } else {
                        this.stop_auto_scroll();
                    }
                }
            }))
            .on_key_down(cx.listener(move |this, ev: &KeyDownEvent, _window, cx| {
                // If find bar is focused, handle find keys
                if this.find_open && this.find_focus.is_focused(_window) {
                    match ev.keystroke.key.as_str() {
                        "escape" => {
                            this.find_open = false;
                            this.find_matches.clear();
                            this.focus_handle.focus(_window);
                            cx.notify();
                            return;
                        }
                        "enter" => {
                            if ev.keystroke.modifiers.shift {
                                this.find_prev();
                            } else {
                                this.find_next();
                            }
                            cx.notify();
                            return;
                        }
                        "backspace" => {
                            this.find_query.pop();
                            this.find_update_matches();
                            this.find_goto_match();
                            cx.notify();
                            return;
                        }
                        _ => {
                            if let Some(key_char) = &ev.keystroke.key_char {
                                if !ev.keystroke.modifiers.platform {
                                    this.find_query.push_str(key_char);
                                    this.find_update_matches();
                                    this.find_goto_match();
                                    cx.notify();
                                    return;
                                }
                            }
                        }
                    }
                    // Pass through Cmd shortcuts even when find is focused
                    if !ev.keystroke.modifiers.platform { return; }
                }

                let shift = ev.keystroke.modifiers.shift;
                let alt = ev.keystroke.modifiers.alt;

                // Cmd shortcuts
                if ev.keystroke.modifiers.platform {
                    match ev.keystroke.key.as_str() {
                        "s" => { if !this.readonly { this.save(cx); } cx.notify(); return; }
                        "a" => { this.select_all(); cx.notify(); return; }
                        "d" => {
                            if this.readonly { return; }
                            this.duplicate_line(cx);
                            this.scroll_to_cursor();
                            cx.notify();
                            return;
                        }
                        "/" => {
                            if this.readonly { return; }
                            this.comment_toggle(cx);
                            cx.notify();
                            return;
                        }
                        "f" => {
                            this.find_open = true;
                            this.find_focus.focus(_window);
                            // Pre-fill with selected text
                            let sel_text = this.get_selected_text();
                            if !sel_text.is_empty() && !sel_text.contains('\n') {
                                this.find_query = sel_text;
                                this.find_update_matches();
                            }
                            cx.notify();
                            return;
                        }
                        "g" => {
                            if this.find_open {
                                if shift { this.find_prev(); } else { this.find_next(); }
                                cx.notify();
                            }
                            return;
                        }
                        "z" => {
                            if this.readonly { return; }
                            if shift {
                                if let Some((r, c)) = this.buffer.redo() {
                                    this.cursor_row = r;
                                    this.cursor_col = c;
                                    this.selection = None;
                                    this.invalidate_tokens(0);
                                    this.scroll_to_cursor();
                                }
                            } else {
                                if let Some((r, c)) = this.buffer.undo() {
                                    this.cursor_row = r;
                                    this.cursor_col = c;
                                    this.selection = None;
                                    this.invalidate_tokens(0);
                                    this.scroll_to_cursor();
                                }
                            }
                            // Update modified state
                            this.modified = this.buffer.content() != std::fs::read_to_string(&this.path).unwrap_or_default();
                            if this.modified {
                                cx.emit(FileViewEvent::TitleChanged(format!("{} \u{25cf}", this.filename)));
                            } else {
                                cx.emit(FileViewEvent::TitleChanged(this.filename.clone()));
                            }
                            cx.notify();
                            return;
                        }
                        "c" | "x" => {
                            let text = this.get_selected_text();
                            if !text.is_empty() {
                                cx.write_to_clipboard(ClipboardItem::new_string(text));
                                if ev.keystroke.key.as_str() == "x" && !this.readonly {
                                    this.delete_selection();
                                    this.mark_modified(cx);
                                }
                            }
                            cx.notify();
                            return;
                        }
                        "v" => {
                            if this.readonly { return; }
                            if let Some(item) = cx.read_from_clipboard() {
                                if let Some(text) = item.text() {
                                    this.delete_selection();
                                    this.buffer.insert_text(this.cursor_row, this.cursor_col, &text);
                                    // Move cursor to end of pasted text
                                    let newlines = text.matches('\n').count();
                                    if newlines > 0 {
                                        this.cursor_row += newlines;
                                        this.cursor_col = text.rsplit('\n').next().map(|s| s.len()).unwrap_or(0);
                                    } else {
                                        this.cursor_col += text.len();
                                    }
                                    this.invalidate_tokens(this.cursor_row.saturating_sub(newlines));
                                    this.mark_modified(cx);
                                    this.scroll_to_cursor();
                                }
                            }
                            cx.notify();
                            return;
                        }
                        "left" => { this.move_home(shift); this.update_bracket_match(); cx.notify(); return; }
                        "right" => { this.move_end(shift); this.update_bracket_match(); cx.notify(); return; }
                        "up" => {
                            // Cmd+Up: go to start of file
                            if shift { this.start_or_extend_selection(); } else { this.selection = None; }
                            this.cursor_row = 0;
                            this.cursor_col = 0;
                            if shift { this.extend_selection(); }
                            this.scroll_to_cursor();
                            cx.notify();
                            return;
                        }
                        "down" => {
                            // Cmd+Down: go to end of file
                            if shift { this.start_or_extend_selection(); } else { this.selection = None; }
                            this.cursor_row = this.buffer.line_count().saturating_sub(1);
                            this.cursor_col = this.buffer.line(this.cursor_row).len();
                            if shift { this.extend_selection(); }
                            this.scroll_to_cursor();
                            cx.notify();
                            return;
                        }
                        _ => { return; }
                    }
                }

                let handled = match ev.keystroke.key.as_str() {
                    "enter" => { if !this.readonly { this.insert_newline(cx); this.scroll_to_cursor(); } true }
                    "backspace" => {
                        if !this.readonly {
                            if alt { this.delete_word_left(cx); }
                            else { this.backspace(cx); }
                            this.scroll_to_cursor();
                        }
                        true
                    }
                    "delete" => { if !this.readonly { this.delete_forward(cx); } true }
                    "tab" => {
                        if !this.readonly {
                            if shift { this.dedent_lines(cx); }
                            else { this.insert_tab(cx); }
                        }
                        true
                    }
                    "space" => { if !this.readonly { this.insert_char(" ", cx); } true }
                    "left" => {
                        if alt { this.move_word_left(shift); }
                        else { this.move_left(shift); }
                        this.scroll_to_cursor();
                        true
                    }
                    "right" => {
                        if alt { this.move_word_right(shift); }
                        else { this.move_right(shift); }
                        this.scroll_to_cursor();
                        true
                    }
                    "up" => { this.move_up(shift); this.scroll_to_cursor(); true }
                    "down" => { this.move_down(shift); this.scroll_to_cursor(); true }
                    "home" => { this.move_home(shift); true }
                    "end" => { this.move_end(shift); true }
                    "escape" => {
                        if this.find_open {
                            this.find_open = false;
                            this.find_matches.clear();
                        }
                        this.selection = None;
                        true
                    }
                    "pageup" => {
                        if shift { this.start_or_extend_selection(); } else { this.selection = None; }
                        this.cursor_row = this.cursor_row.saturating_sub(30);
                        this.ensure_cursor_bounds();
                        if shift { this.extend_selection(); }
                        this.scroll_to_cursor();
                        true
                    }
                    "pagedown" => {
                        if shift { this.start_or_extend_selection(); } else { this.selection = None; }
                        this.cursor_row = (this.cursor_row + 30).min(this.buffer.line_count().saturating_sub(1));
                        this.ensure_cursor_bounds();
                        if shift { this.extend_selection(); }
                        this.scroll_to_cursor();
                        true
                    }
                    _ => false,
                };

                if !handled && !this.readonly {
                    if let Some(key_char) = &ev.keystroke.key_char {
                        if !ev.keystroke.modifiers.platform {
                            this.delete_selection();
                            this.insert_char(key_char, cx);
                            this.scroll_to_cursor();
                        }
                    } else if ev.keystroke.key.len() == 1 {
                        let ch = ev.keystroke.key.clone();
                        this.delete_selection();
                        this.insert_char(&ch, cx);
                        this.scroll_to_cursor();
                    } else {
                        return;
                    }
                }

                this.update_bracket_match();
                cx.notify();
            }))
            // File header (hidden in compact/diff mode)
            .when(!self.compact, |d: Div| {
                d.child(
                    div()
                        .flex().flex_row().items_center()
                        .w_full().h(px(28.)).min_h(px(28.)).flex_shrink_0()
                        .px(px(10.))
                        .border_b_1().border_color(theme::surface1())
                        .child(div().text_color(theme::text()).child(title_display.clone()))
                        .child(div().ml(px(8.)).text_xs().text_color(theme::overlay()).child(self.path.to_string_lossy().to_string())),
                )
            })
            // Find bar
            .when(find_open && !self.compact, |d: Div| {
                d.child(
                    div()
                        .flex().flex_row().items_center()
                        .w_full().h(px(32.)).min_h(px(32.)).flex_shrink_0()
                        .px(px(10.)).gap(px(8.))
                        .bg(theme::surface0())
                        .border_b_1().border_color(theme::surface1())
                        .child(
                            div()
                                .flex().flex_row().items_center()
                                .flex_1()
                                .h(px(24.))
                                .px(px(8.))
                                .bg(theme::base())
                                .rounded(px(4.))
                                .border_1().border_color(theme::blue())
                                .text_color(theme::text())
                                .child(if find_query.is_empty() {
                                    div().text_color(theme::overlay()).child("Find...").into_any_element()
                                } else {
                                    div().child(find_query.replace(' ', "\u{00A0}")).into_any_element()
                                }),
                        )
                        .child(
                            div().text_xs().text_color(theme::subtext())
                                .child(if find_count > 0 {
                                    format!("{} / {}", find_current + 1, find_count)
                                } else if !find_query.is_empty() {
                                    "No matches".to_string()
                                } else {
                                    String::new()
                                }),
                        ),
                )
            })
            // Content
            .child({
                let lines: Vec<AnyElement> = (0..self.buffer.line_count()).map(|row| {
                    self.render_line(row, line_num_width, is_focused, cx).into_any_element()
                }).collect();

                let content_inner = div().flex().flex_col().p(px(4.)).children(lines);

                if compact {
                    // No scroll — parent (DiffView) handles scrolling
                    div().w_full().child(content_inner).into_any_element()
                } else {
                    div()
                        .id("file-content-scroll")
                        .flex_1()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll_handle)
                        .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _window, cx| {
                            let (dx, dy): (f32, f32) = match &ev.delta {
                                ScrollDelta::Lines(d) => (d.x * 20.0, d.y * 20.0),
                                ScrollDelta::Pixels(d) => {
                                    let x: f32 = d.x.into();
                                    let y: f32 = d.y.into();
                                    (x, y)
                                }
                            };
                            // Rail: only apply horizontal if clearly horizontal (ratio 3:1)
                            if dx.abs() > dy.abs() * 3.0 {
                                this.scroll_x = (this.scroll_x - dx).max(0.0);
                                cx.notify();
                            }
                        }))
                        .child(content_inner)
                        .into_any_element()
                }
            })
            // Status bar (hidden in compact/diff mode)
            .when(!self.compact, |d: Div| {
                d.child(
                    div()
                        .flex().flex_row().items_center()
                        .w_full().h(px(22.)).min_h(px(22.)).flex_shrink_0()
                        .px(px(10.))
                        .bg(theme::surface0())
                        .border_t_1().border_color(theme::surface1())
                        .text_xs().text_color(theme::subtext())
                        .child(format!("Ln {}, Col {}", cursor_row + 1, cursor_col + 1))
                        .child(div().flex_1())
                        .child(div().mr(px(12.)).child(lang_label.to_string()))
                        .child(div().mr(px(12.)).child("UTF-8"))
                        .child("4 Spaces"),
                )
            })
    }
}
