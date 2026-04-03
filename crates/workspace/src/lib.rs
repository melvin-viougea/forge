pub mod dock;
pub mod pane;
pub mod workspace;

pub use dock::*;
pub use pane::*;
pub use workspace::*;

// Catppuccin Mocha color palette
pub mod theme {
    use gpui::rgb;
    use gpui::Rgba;

    pub fn base() -> Rgba {
        rgb(0x1e1e2e)
    }
    pub fn mantle() -> Rgba {
        rgb(0x181825)
    }
    pub fn surface0() -> Rgba {
        rgb(0x313244)
    }
    pub fn surface1() -> Rgba {
        rgb(0x45475a)
    }
    pub fn text() -> Rgba {
        rgb(0xcdd6f4)
    }
    pub fn subtext() -> Rgba {
        rgb(0xa6adc8)
    }
    pub fn blue() -> Rgba {
        rgb(0x89b4fa)
    }
    pub fn green() -> Rgba {
        rgb(0xa6e3a1)
    }
    pub fn red() -> Rgba {
        rgb(0xf38ba8)
    }
    pub fn yellow() -> Rgba {
        rgb(0xf9e2af)
    }
    pub fn lavender() -> Rgba {
        rgb(0xb4befe)
    }
    pub fn overlay() -> Rgba {
        rgb(0x6c7086)
    }
}
