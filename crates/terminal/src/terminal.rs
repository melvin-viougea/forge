use anyhow::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{EventListener, VoidListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::cell::Cell as AlaCell;
use alacritty_terminal::term::cell::Flags as CellFlags;
use alacritty_terminal::term::Config;
use alacritty_terminal::Term;
use alacritty_terminal::vte::ansi;

pub struct Terminal {
    term: Arc<Mutex<Term<VoidListener>>>,
    writer: Option<Box<dyn Write + Send>>,
    _reader_handle: Option<std::thread::JoinHandle<()>>,
    pub has_new_data: Arc<AtomicBool>,
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
                if is_fg { rgb(0xcdd6f4) } else { rgb(0x00000000) }
            }
            TermColor::Indexed(idx) => {
                let hex = match idx {
                    0 => 0x45475a, 1 => 0xf38ba8, 2 => 0xa6e3a1, 3 => 0xf9e2af,
                    4 => 0x89b4fa, 5 => 0xf5c2e7, 6 => 0x94e2d5, 7 => 0xbac2de,
                    8 => 0x585b70, 9 => 0xf38ba8, 10 => 0xa6e3a1, 11 => 0xf9e2af,
                    12 => 0x89b4fa, 13 => 0xf5c2e7, 14 => 0x94e2d5, 15 => 0xa6adc8,
                    16..=231 => {
                        let n = idx - 16;
                        let b = (n % 6) * 51; let g = ((n / 6) % 6) * 51; let r = (n / 36) * 51;
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

        pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let mut reader = pair.master.try_clone_reader()?;

        // Create alacritty terminal
        let config = Config::default();
        let dimensions = TermDimensions {
            columns: cols as usize,
            screen_lines: rows as usize,
        };
        let term = Term::new(config, &dimensions, VoidListener);
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

        Ok(Self {
            term,
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

    /// Read visible lines from alacritty's grid for rendering
    pub fn get_visible_lines(&self, _max_lines: usize) -> Vec<TerminalLine> {
        use alacritty_terminal::index::{Line, Column};

        let term = self.term.lock().unwrap();
        let grid = term.grid();
        let mut lines = Vec::new();

        let total_lines = grid.screen_lines();

        for line_idx in 0..total_lines {
            let row = &grid[Line(line_idx as i32)];
            let mut cells = Vec::new();

            for col_idx in 0..grid.columns() {
                let cell = &row[Column(col_idx)];
                cells.push(convert_cell(cell));
            }

            // Trim trailing spaces (but keep lines with background colors)
            while cells.last().map_or(false, |c| c.ch == ' ' && c.style.bg == TermColor::Default) {
                cells.pop();
            }

            lines.push(TerminalLine { cells });
        }

        lines
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

    pub fn check_and_clear_new_data(&self) -> bool {
        self.has_new_data.swap(false, Ordering::Relaxed)
    }
}
