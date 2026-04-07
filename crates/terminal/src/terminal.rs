use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Cell as AlaCell;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::{Config, TermMode};
use alacritty_terminal::Term;
use alacritty_terminal::vte::ansi;

/// Captures OSC title change events from the terminal.
#[derive(Clone)]
struct TitleListener {
    title: Arc<Mutex<Option<String>>>,
}

impl EventListener for TitleListener {
    fn send_event(&self, event: Event) {
        if let Event::Title(title) = event {
            *self.title.lock().unwrap() = Some(title);
        }
    }
}

pub struct Terminal {
    term: Arc<Mutex<Term<TitleListener>>>,
    writer: Option<Box<dyn Write + Send>>,
    master_pty: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>>,
    _reader_handle: Option<std::thread::JoinHandle<()>>,
    pub has_new_data: Arc<AtomicBool>,
    osc_title: Arc<Mutex<Option<String>>>,
    pub title: String,
    pub cols: u16,
    pub rows: u16,
}

// ── Color model (for rendering) ──────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TermColor {
    Default,
    Indexed(u8),
    Rgb(u8, u8, u8),
}

impl TermColor {
    pub fn to_rgba(&self, is_fg: bool) -> gpui::Rgba {
        use gpui::rgb;
        match self {
            TermColor::Default => {
                if is_fg { rgb(0xc9d1d9) } else { rgb(0x00000000) }
            }
            TermColor::Indexed(idx) => {
                let hex = match idx {
                    0 => 0x484f58, 1 => 0xf85149, 2 => 0x3fb950, 3 => 0xd29922,
                    4 => 0x58a6ff, 5 => 0xbc8cff, 6 => 0x56d4dd, 7 => 0xb1bac4,
                    8 => 0x6e7681, 9 => 0xf85149, 10 => 0x3fb950, 11 => 0xd29922,
                    12 => 0x58a6ff, 13 => 0xbc8cff, 14 => 0x56d4dd, 15 => 0x8b949e,
                    16..=231 => {
                        let n = idx - 16;
                        let b = (n % 6) * 51; let g = ((n / 6) % 6) * 51; let r = (n / 36) * 51;
                        ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
                    }
                    232..=255 => {
                        let v = 8 + (idx - 232) * 10;
                        ((v as u32) << 16) | ((v as u32) << 8) | (v as u32)
                    }
                    _ => 0xc9d1d9,
                };
                rgb(hex)
            }
            TermColor::Rgb(r, g, b) => {
                gpui::rgb(((*r as u32) << 16) | ((*g as u32) << 8) | (*b as u32))
            }
        }
    }
}

fn convert_color(color: &ansi::Color) -> TermColor {
    use alacritty_terminal::vte::ansi::NamedColor;
    match color {
        ansi::Color::Named(named) => match named {
            NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => TermColor::Default,
            NamedColor::Background => TermColor::Default,
            other => {
                let idx = *other as u8;
                if idx <= 15 { TermColor::Indexed(idx) } else { TermColor::Default }
            }
        },
        ansi::Color::Spec(rgb) => TermColor::Rgb(rgb.r, rgb.g, rgb.b),
        ansi::Color::Indexed(idx) => TermColor::Indexed(*idx),
    }
}

// ── Cell & Line (rendering types) ────────────────────────────

#[derive(Clone, Debug)]
pub struct CellStyle {
    pub fg: TermColor,
    pub bg: TermColor,
    pub bold: bool,
}

impl Default for CellStyle {
    fn default() -> Self {
        Self { fg: TermColor::Default, bg: TermColor::Default, bold: false }
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

pub struct StyledSpan {
    pub text: String,
    pub style: CellStyle,
}

impl TerminalLine {
    pub fn to_spans(&self) -> Vec<StyledSpan> {
        if self.cells.is_empty() {
            return vec![StyledSpan { text: " ".to_string(), style: CellStyle::default() }];
        }
        let mut spans = Vec::new();
        let mut text = String::new();
        let mut style = self.cells[0].style.clone();

        for cell in &self.cells {
            if cell.style.fg == style.fg && cell.style.bg == style.bg && cell.style.bold == style.bold {
                text.push(cell.ch);
            } else {
                if !text.is_empty() {
                    spans.push(StyledSpan { text: text.clone(), style: style.clone() });
                }
                text.clear();
                text.push(cell.ch);
                style = cell.style.clone();
            }
        }
        if !text.is_empty() {
            spans.push(StyledSpan { text, style });
        }
        spans
    }
}

/// Convert an alacritty Cell to our rendering Cell
fn convert_cell(cell: &AlaCell) -> Cell {
    Cell {
        ch: cell.c,
        style: CellStyle {
            fg: convert_color(&cell.fg),
            bg: convert_color(&cell.bg),
            bold: cell.flags.contains(CellFlags::BOLD),
        },
    }
}

// ── Terminal dimensions helper ───────────────────────────────

struct TermDimensions {
    columns: usize,
    screen_lines: usize,
}

impl Dimensions for TermDimensions {
    fn total_lines(&self) -> usize { self.screen_lines }
    fn screen_lines(&self) -> usize { self.screen_lines }
    fn columns(&self) -> usize { self.columns }
}

// ── Terminal impl ────────────────────────────────────────────

impl Terminal {
    pub fn new(title: String, cols: u16, rows: u16, working_dir: Option<PathBuf>) -> Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows, cols, pixel_width: 0, pixel_height: 0,
        })?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let mut cmd = CommandBuilder::new(&shell);
        cmd.arg("-l");
        cmd.cwd(working_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| "/".into())));
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("PROMPT", "%~ %# ");

        pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        // Create alacritty terminal with title listener
        let config = Config::default();
        let dimensions = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let osc_title: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let listener = TitleListener { title: osc_title.clone() };
        let term = Term::new(config, &dimensions, listener);
        let term = Arc::new(Mutex::new(term));

        let has_new_data = Arc::new(AtomicBool::new(false));
        let data_flag = has_new_data.clone();
        let term_clone = term.clone();

        let reader_handle = std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut processor: ansi::Processor = ansi::Processor::new();

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let mut term = term_clone.lock().unwrap();
                        processor.advance(&mut *term, &buf[..n]);
                        data_flag.store(true, Ordering::Relaxed);
                    }
                    Err(_) => break,
                }
            }
        });

        let master_pty: Arc<Mutex<Box<dyn portable_pty::MasterPty + Send>>> =
            Arc::new(Mutex::new(pair.master));

        Ok(Self {
            term,
            writer: Some(writer),
            master_pty,
            _reader_handle: Some(reader_handle),
            has_new_data,
            osc_title,
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

    /// Read visible lines from alacritty's grid for rendering
    pub fn get_visible_lines(&self, _max_lines: usize) -> Vec<TerminalLine> {
        use alacritty_terminal::index::{Line, Column};

        let term = self.term.lock().unwrap();
        let grid = term.grid();
        let display_offset = grid.display_offset();
        let total_lines = grid.screen_lines();
        let mut lines = Vec::new();

        for line_idx in 0..total_lines {
            // Negative line indices = scrollback, positive = visible screen
            let line = Line(line_idx as i32) - display_offset;
            let row = &grid[line];
            let mut cells = Vec::new();

            for col_idx in 0..grid.columns() {
                let cell = &row[Column(col_idx)];
                cells.push(convert_cell(cell));
            }

            lines.push(TerminalLine { cells });
        }

        lines
    }

    /// Scroll the terminal view (positive = up into scrollback, negative = down)
    pub fn scroll(&self, delta: i32) {
        use alacritty_terminal::grid::Scroll;
        let mut term = self.term.lock().unwrap();
        term.grid_mut().scroll_display(Scroll::Delta(delta));
    }

    /// Scroll to bottom (follow mode)
    pub fn scroll_to_bottom(&self) {
        use alacritty_terminal::grid::Scroll;
        let mut term = self.term.lock().unwrap();
        term.grid_mut().scroll_display(Scroll::Bottom);
    }

    /// Check if we're scrolled up (not at bottom)
    pub fn is_scrolled(&self) -> bool {
        self.term.lock().unwrap().grid().display_offset() > 0
    }

    /// Get scroll info: (display_offset, total_history_lines, screen_lines)
    pub fn scroll_info(&self) -> (usize, usize, usize) {
        let term = self.term.lock().unwrap();
        let grid = term.grid();
        let offset = grid.display_offset();
        let history = grid.history_size();
        let screen = grid.screen_lines();
        (offset, history, screen)
    }

    /// Get cursor position (row in visible area, col)
    pub fn cursor_position(&self) -> (usize, usize) {
        let term = self.term.lock().unwrap();
        let cursor = term.grid().cursor.point;
        (cursor.line.0 as usize, cursor.column.0)
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_position().1
    }

    /// Resize the terminal to new dimensions (in characters)
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == self.cols && rows == self.rows {
            return;
        }
        if cols == 0 || rows == 0 {
            return;
        }
        self.cols = cols;
        self.rows = rows;

        // Resize the PTY
        let _ = self.master_pty.lock().unwrap().resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });

        // Resize alacritty's term
        let dims = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        self.term.lock().unwrap().resize(dims);
    }

    /// Whether the terminal application has enabled bracketed paste mode.
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.term
            .lock()
            .unwrap()
            .mode()
            .contains(TermMode::BRACKETED_PASTE)
    }

    /// Write data to the PTY, wrapping in bracket paste sequences if the mode is active.
    pub fn paste(&mut self, data: &[u8]) {
        if self.bracketed_paste_enabled() {
            self.write_input(b"\x1b[200~");
            self.write_input(data);
            self.write_input(b"\x1b[201~");
        } else {
            self.write_input(data);
        }
    }

    pub fn check_and_clear_new_data(&self) -> bool {
        self.has_new_data.swap(false, Ordering::Relaxed)
    }

    /// Take the latest OSC title if one was set since last call.
    pub fn take_osc_title(&self) -> Option<String> {
        self.osc_title.lock().unwrap().take()
    }
}
