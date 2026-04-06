pub mod dock;
pub mod pane;
pub mod workspace;

pub use dock::*;
pub use pane::*;
pub use workspace::*;

// Forge Dark theme — inspired by CMUX / Polyscope
pub mod theme {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn base() -> Rgba {
        rgb(0x0a0e14)
    }
    pub fn mantle() -> Rgba {
        rgb(0x0d1117)
    }
    pub fn surface0() -> Rgba {
        rgb(0x161b22)
    }
    pub fn surface1() -> Rgba {
        rgb(0x21262d)
    }
    pub fn text() -> Rgba {
        rgb(0xc9d1d9)
    }
    pub fn subtext() -> Rgba {
        rgb(0x8b949e)
    }
    pub fn blue() -> Rgba {
        rgb(0x58a6ff)
    }
    pub fn green() -> Rgba {
        rgb(0x3fb950)
    }
    pub fn red() -> Rgba {
        rgb(0xf85149)
    }
    pub fn yellow() -> Rgba {
        rgb(0xd29922)
    }
    pub fn lavender() -> Rgba {
        rgb(0x79c0ff)
    }
    pub fn overlay() -> Rgba {
        rgb(0x484f58)
    }
}
