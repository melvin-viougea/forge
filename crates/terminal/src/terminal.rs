use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Terminal {
    buffer: Arc<Mutex<TerminalBuffer>>,
    writer: Option<Box<dyn Write + Send>>,
    _reader_handle: Option<std::thread::JoinHandle<()>>,
    pub has_new_data: Arc<AtomicBool>,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
}

// ── Color model ──────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TermColor {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl TermColor {
    /// Convert to GPUI Rgba
    pub fn to_rgba(&self, is_fg: bool) -> gpui::Rgba {
        use gpui::rgb;
        match self {
            TermColor::Default => {
                if is_fg { rgb(0xcdd6f4) } else { rgb(0x00000000) }
            }
            TermColor::Indexed(idx) => {
                // Standard 16 colors (Catppuccin Mocha inspired)
                let hex = match idx {
                    0 => 0x45475a,   // black
                    1 => 0xf38ba8,   // red
                    2 => 0xa6e3a1,   // green
                    3 => 0xf9e2af,   // yellow
                    4 => 0x89b4fa,   // blue
                    5 => 0xf5c2e7,   // magenta
                    6 => 0x94e2d5,   // cyan
                    7 => 0xbac2de,   // white
                    8 => 0x585b70,   // bright black
                    9 => 0xf38ba8,   // bright red
                    10 => 0xa6e3a1,  // bright green
                    11 => 0xf9e2af,  // bright yellow
                    12 => 0x89b4fa,  // bright blue
                    13 => 0xf5c2e7,  // bright magenta
                    14 => 0x94e2d5,  // bright cyan
                    15 => 0xa6adc8,  // bright white
                    // 256-color: 16-231 = 6x6x6 color cube, 232-255 = grayscale
                    16..=231 => {
                        let n = idx - 16;
                        let b = (n % 6) * 51;
                        let g = ((n / 6) % 6) * 51;
                        let r = (n / 36) * 51;
                        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
                    }
                    232..=255 => {
                        let v = 8 + (idx - 232) * 10;
                        ((v as u32) << 16) | ((v as u32) << 8) | (v as u32)
                    }
                    _ => 0xcdd6f4,
                };
                rgb(hex)
            }
            TermColor::Rgb(r, g, b) => {
                let hex = ((*r as u32) << 16) | ((*g as u32) << 8) | (*b as u32);
                rgb(hex)
            }
        }
    }
}

// ── Cell & Line ──────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct CellStyle {
    pub fg: TermColor,
    pub bg: TermColor,
    pub bold: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self {
            fg: TermColor::Default,
            bg: TermColor::Default,
            bold: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub ch: char,
    pub style: CellStyle,
}

#[derive(Clone, Debug)]
pub struct TerminalLine {
    pub cells: Vec<Cell>,
}

impl TerminalLine {
    fn new() -> Self {
        Self { cells: Vec::new() }
    }

    /// Get plain text (for backward compat)
    pub fn text(&self) -> String {
        self.cells.iter().map(|c| c.ch).collect()
    }
}

/// A run of text with the same style
#[derive(Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: CellStyle,
}

impl TerminalLine {
    /// Group cells into styled spans for efficient rendering
    pub fn to_spans(&self) -> Vec<StyledSpan> {
        if self.cells.is_empty() {
            return vec![StyledSpan {
                text: " ".to_string(),
                style: CellStyle::default(),
            }];
        }

        let mut spans = Vec::new();
        let mut current_text = String::new();
        let mut current_style = self.cells[0].style.clone();

        for cell in &self.cells {
            if cell.style.fg == current_style.fg
                && cell.style.bg == current_style.bg
                && cell.style.bold == current_style.bold
            {
                current_text.push(cell.ch);
            } else {
                if !current_text.is_empty() {
                    spans.push(StyledSpan {
                        text: current_text.clone(),
                        style: current_style.clone(),
                    });
                }
                current_text.clear();
                current_text.push(cell.ch);
                current_style = cell.style.clone();
            }
        }

        if !current_text.is_empty() {
            spans.push(StyledSpan {
                text: current_text,
                style: current_style,
            });
        }

        spans
    }
}

// ── Buffer ───────────────────────────────────────────────────

struct TerminalBuffer {
    lines: Vec<TerminalLine>,
    cursor_col: usize,
    current_style: CellStyle,
}

impl TerminalBuffer {
    fn new() -> Self {
        Self {
            lines: vec![TerminalLine::new()],
            cursor_col: 0,
            current_style: CellStyle::default(),
        }
    }

    fn newline(&mut self) {
        self.lines.push(TerminalLine::new());
        self.cursor_col = 0;
        if self.lines.len() > 10000 {
            let excess = self.lines.len() - 10000;
            self.lines.drain(..excess);
        }
    }

    fn carriage_return(&mut self) {
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            if let Some(last) = self.lines.last_mut() {
                if self.cursor_col < last.cells.len() {
                    last.cells.remove(self.cursor_col);
                }
            }
        }
    }

    fn tab(&mut self) {
        let spaces = 8 - (self.cursor_col % 8);
        for _ in 0..spaces {
            self.put_char(' ');
        }
    }

    fn put_char(&mut self, ch: char) {
        let cell = Cell {
            ch,
            style: self.current_style.clone(),
        };

        if let Some(last) = self.lines.last_mut() {
            if self.cursor_col < last.cells.len() {
                last.cells[self.cursor_col] = cell;
            } else {
                // Pad with spaces
                while last.cells.len() < self.cursor_col {
                    last.cells.push(Cell {
                        ch: ' ',
                        style: CellStyle::default(),
                    });
                }
                last.cells.push(cell);
            }
            self.cursor_col += 1;
        }
    }

    /// Move cursor to absolute column (1-indexed, CSI G)
    fn cursor_to_col(&mut self, col: usize) {
        self.cursor_col = if col > 0 { col - 1 } else { 0 };
    }

    /// Move cursor forward by n columns (CSI C)
    fn cursor_forward(&mut self, n: usize) {
        self.cursor_col += n;
    }

    /// Move cursor backward by n columns (CSI D)
    fn cursor_backward(&mut self, n: usize) {
        self.cursor_col = self.cursor_col.saturating_sub(n);
    }

    /// Erase in line (CSI K)
    fn erase_in_line(&mut self, mode: u16) {
        if let Some(last) = self.lines.last_mut() {
            match mode {
                0 => {
                    // Erase from cursor to end of line
                    if self.cursor_col < last.cells.len() {
                        last.cells.truncate(self.cursor_col);
                    }
                }
                1 => {
                    // Erase from beginning to cursor
                    for i in 0..self.cursor_col.min(last.cells.len()) {
                        last.cells[i] = Cell {
                            ch: ' ',
                            style: CellStyle::default(),
                        };
                    }
                }
                2 => {
                    // Erase entire line
                    last.cells.clear();
                    self.cursor_col = 0;
                }
                _ => {}
            }
        }
    }

    /// Erase in display (CSI J)
    fn erase_in_display(&mut self, mode: u16) {
        match mode {
            0 => {
                // Erase from cursor to end - just erase current line from cursor
                self.erase_in_line(0);
            }
            2 | 3 => {
                // Don't actually clear scrollback — just add visual separation
                // This preserves "Last login" and other startup messages
                self.lines.push(TerminalLine::new());
                self.cursor_col = 0;
            }
            _ => {}
        }
    }

    /// Handle a complete CSI sequence
    fn handle_csi(&mut self, params: &[u16], ch: char) {
        match ch {
            'm' => self.apply_sgr(params),
            'G' => {
                // CHA - Cursor Horizontal Absolute
                let col = params.first().copied().unwrap_or(1) as usize;
                self.cursor_to_col(col);
            }
            'C' => {
                // CUF - Cursor Forward
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_forward(n);
            }
            'D' => {
                // CUB - Cursor Backward
                let n = params.first().copied().unwrap_or(1) as usize;
                self.cursor_backward(n);
            }
            'K' => {
                // EL - Erase in Line
                let mode = params.first().copied().unwrap_or(0);
                self.erase_in_line(mode);
            }
            'J' => {
                // ED - Erase in Display
                let mode = params.first().copied().unwrap_or(0);
                self.erase_in_display(mode);
            }
            'A' => {
                // CUU - Cursor Up (ignore for now, we don't track row)
            }
            'B' => {
                // CUD - Cursor Down (ignore for now)
            }
            'H' | 'f' => {
                // CUP - Cursor Position (row;col) - handle column only
                if params.len() >= 2 {
                    self.cursor_to_col(params[1] as usize);
                }
            }
            'X' => {
                // ECH - Erase Characters
                let n = params.first().copied().unwrap_or(1) as usize;
                if let Some(last) = self.lines.last_mut() {
                    for i in 0..n {
                        let pos = self.cursor_col + i;
                        if pos < last.cells.len() {
                            last.cells[pos] = Cell {
                                ch: ' ',
                                style: CellStyle::default(),
                            };
                        }
                    }
                }
            }
            'd' => {
                // VPA - Vertical Position Absolute (move to row, keep col)
                // We can't easily move between rows in our model, ignore
            }
            'r' => {
                // DECSTBM - Set Scrolling Region, ignore
            }
            'h' | 'l' => {
                // SM/RM - Set/Reset Mode (non-private), ignore
            }
            'n' => {
                // DSR - Device Status Report, ignore
            }
            's' => {
                // SCP - Save Cursor Position, ignore
            }
            'u' => {
                // RCP - Restore Cursor Position, ignore
            }
            'P' => {
                // DCH - Delete Characters
                let n = params.first().copied().unwrap_or(1) as usize;
                if let Some(last) = self.lines.last_mut() {
                    for _ in 0..n {
                        if self.cursor_col < last.cells.len() {
                            last.cells.remove(self.cursor_col);
                        }
                    }
                }
            }
            '@' => {
                // ICH - Insert Characters
                let n = params.first().copied().unwrap_or(1) as usize;
                if let Some(last) = self.lines.last_mut() {
                    for _ in 0..n {
                        last.cells.insert(self.cursor_col, Cell {
                            ch: ' ',
                            style: CellStyle::default(),
                        });
                    }
                }
            }
            'L' => {
                // IL - Insert Lines, ignore (no row tracking)
            }
            'M' => {
                // DL - Delete Lines, ignore
            }
            'S' => {
                // SU - Scroll Up, ignore
            }
            'T' => {
                // SD - Scroll Down, ignore
            }
            _ => {} // Ignore unsupported
        }
    }

    /// Handle DEC private mode sequences (ESC[?...h/l)
    fn handle_dec_private(&mut self, _params: &[u16], _ch: char) {
        // Private modes we silently ignore:
        // ?1    - Application cursor keys
        // ?7    - Auto-wrap mode
        // ?12   - Cursor blink
        // ?25   - Cursor visibility (h=show, l=hide)
        // ?47   - Alternate screen buffer (old)
        // ?1000 - Mouse tracking
        // ?1049 - Alternate screen buffer (new)
        // ?2004 - Bracketed paste mode
        // All are display concerns we don't need to track for text content
    }

    /// Apply SGR (Select Graphic Rendition) parameters
    fn apply_sgr(&mut self, params: &[u16]) {
        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => self.current_style = CellStyle::default(),
                1 => self.current_style.bold = true,
                2 => {} // Dim - ignore for now
                3 => {} // Italic - ignore for now
                4 => {} // Underline - ignore for now
                7 => {
                    // Reverse video - swap fg/bg
                    std::mem::swap(&mut self.current_style.fg, &mut self.current_style.bg);
                }
                22 => self.current_style.bold = false,
                23 => {} // Not italic
                24 => {} // Not underlined
                27 => {
                    // Reverse off - swap back
                    std::mem::swap(&mut self.current_style.fg, &mut self.current_style.bg);
                }
                // Standard foreground colors
                30..=37 => self.current_style.fg = TermColor::Indexed((params[i] - 30) as u8),
                39 => self.current_style.fg = TermColor::Default,
                // Standard background colors
                40..=47 => self.current_style.bg = TermColor::Indexed((params[i] - 40) as u8),
                49 => self.current_style.bg = TermColor::Default,
                // Bright foreground
                90..=97 => self.current_style.fg = TermColor::Indexed((params[i] - 90 + 8) as u8),
                // Bright background
                100..=107 => self.current_style.bg = TermColor::Indexed((params[i] - 100 + 8) as u8),
                // Extended colors
                38 => {
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 if i + 2 < params.len() => {
                                self.current_style.fg = TermColor::Indexed(params[i + 2] as u8);
                                i += 2;
                            }
                            2 if i + 4 < params.len() => {
                                self.current_style.fg = TermColor::Rgb(
                                    params[i + 2] as u8,
                                    params[i + 3] as u8,
                                    params[i + 4] as u8,
                                );
                                i += 4;
                            }
                            _ => { i += 1; }
                        }
                    }
                }
                48 => {
                    if i + 1 < params.len() {
                        match params[i + 1] {
                            5 if i + 2 < params.len() => {
                                self.current_style.bg = TermColor::Indexed(params[i + 2] as u8);
                                i += 2;
                            }
                            2 if i + 4 < params.len() => {
                                self.current_style.bg = TermColor::Rgb(
                                    params[i + 2] as u8,
                                    params[i + 3] as u8,
                                    params[i + 4] as u8,
                                );
                                i += 4;
                            }
                            _ => { i += 1; }
                        }
                    }
                }
                _ => {} // Ignore unsupported
            }
            i += 1;
        }
    }
}

// ── ANSI parser ──────────────────────────────────────────────

#[derive(PartialEq)]
enum AnsiState {
    Normal,
    Escape,
    CsiParam,
    OscString,
}

// ── Terminal impl ────────────────────────────────────────────

impl Terminal {
    pub fn new(title: String, cols: u16, rows: u16) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        cmd.cwd(std::env::current_dir().unwrap_or_else(|_| "/".into()));

        // Use xterm-256color for full color support
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        let buffer: Arc<Mutex<TerminalBuffer>> =
            Arc::new(Mutex::new(TerminalBuffer::new()));

        let has_new_data = Arc::new(AtomicBool::new(false));
        let data_flag = has_new_data.clone();
        let buffer_clone = buffer.clone();

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let mut ansi_state = AnsiState::Normal;
            let mut csi_params = String::new();

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = String::from_utf8_lossy(&buf[..n]);
                        let mut tb = buffer_clone.lock().unwrap();

                        for ch in text.chars() {
                            match ansi_state {
                                AnsiState::Normal => match ch {
                                    '\x1b' => {
                                        ansi_state = AnsiState::Escape;
                                    }
                                    '\n' => tb.newline(),
                                    '\r' => tb.carriage_return(),
                                    '\x08' => tb.backspace(),
                                    '\t' => tb.tab(),
                                    '\x07' => {}
                                    _ if ch.is_control() => {}
                                    _ => tb.put_char(ch),
                                },
                                AnsiState::Escape => match ch {
                                    '[' => {
                                        ansi_state = AnsiState::CsiParam;
                                        csi_params.clear();
                                    }
                                    ']' => {
                                        ansi_state = AnsiState::OscString;
                                    }
                                    '(' | ')' | '*' | '+' => {
                                        ansi_state = AnsiState::Normal;
                                    }
                                    _ => {
                                        ansi_state = AnsiState::Normal;
                                    }
                                },
                                AnsiState::CsiParam => {
                                    if ch.is_ascii_alphabetic() || ch == '@' || ch == '`' {
                                        // End of CSI — strip prefix chars (?, >, !, =)
                                        let param_str = csi_params
                                            .trim_start_matches(|c: char| c == '?' || c == '>' || c == '!' || c == '=');
                                        let is_private = csi_params.starts_with('?');
                                        let params: Vec<u16> = if param_str.is_empty() {
                                            vec![0]
                                        } else {
                                            param_str
                                                .split(|c: char| c == ';' || c == ':')
                                                .filter_map(|s| s.parse().ok())
                                                .collect()
                                        };
                                        if is_private {
                                            tb.handle_dec_private(&params, ch);
                                        } else {
                                            tb.handle_csi(&params, ch);
                                        }
                                        ansi_state = AnsiState::Normal;
                                    } else {
                                        csi_params.push(ch);
                                    }
                                }
                                AnsiState::OscString => {
                                    if ch == '\x07' || ch == '\\' {
                                        ansi_state = AnsiState::Normal;
                                    }
                                }
                            }
                        }

                        data_flag.store(true, Ordering::Relaxed);
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            buffer,
            writer: Some(writer),
            _reader_handle: Some(reader_handle),
            has_new_data,
            title,
            cols,
            rows,
        })
    }

    pub fn write_input(&mut self, data: &[u8]) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.write_all(data);
            let _ = writer.flush();
        }
    }

    pub fn get_visible_lines(&self, max_lines: usize) -> Vec<TerminalLine> {
        let tb = self.buffer.lock().unwrap();
        let lines = &tb.lines;
        let start = if lines.len() > max_lines {
            lines.len() - max_lines
        } else {
            0
        };
        lines[start..].to_vec()
    }

    pub fn cursor_col(&self) -> usize {
        self.buffer.lock().unwrap().cursor_col
    }

    pub fn check_and_clear_new_data(&self) -> bool {
        self.has_new_data.swap(false, Ordering::Relaxed)
    }
}
