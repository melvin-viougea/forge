pub mod dock;
pub mod file_view;
pub mod image_view;
pub mod markdown_view;
pub mod pane;
pub mod workspace;

pub use dock::*;
pub use file_view::*;
pub use image_view::*;
pub use markdown_view::*;
pub use pane::*;
pub use workspace::*;

/// Configurable theme system with presets
pub mod theme {
    use gpui::rgb;
    use gpui::Rgba;
    use std::sync::Mutex;

    #[derive(Clone, Copy)]
    pub struct ThemeColors {
        pub base: u32,
        pub mantle: u32,
        pub surface0: u32,
        pub surface1: u32,
        pub text: u32,
        pub subtext: u32,
        pub blue: u32,
        pub green: u32,
        pub red: u32,
        pub yellow: u32,
        pub lavender: u32,
        pub overlay: u32,
        pub selection: u32,
        pub teal: u32,
        pub peach: u32,
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum ThemeName {
        ForgeDark,
        VSCode,
        CatppuccinMocha,
        Nord,
        Dracula,
        OneDark,
    }

    impl ThemeName {
        pub fn label(&self) -> &'static str {
            match self {
                Self::ForgeDark => "Forge Dark",
                Self::VSCode => "Visual Studio Code",
                Self::CatppuccinMocha => "Catppuccin Mocha",
                Self::Nord => "Nord",
                Self::Dracula => "Dracula",
                Self::OneDark => "One Dark",
            }
        }

        pub fn as_str(&self) -> &'static str {
            match self {
                Self::ForgeDark => "forge-dark",
                Self::VSCode => "vscode",
                Self::CatppuccinMocha => "catppuccin-mocha",
                Self::Nord => "nord",
                Self::Dracula => "dracula",
                Self::OneDark => "one-dark",
            }
        }

        pub fn from_str(s: &str) -> Option<Self> {
            match s {
                "forge-dark" => Some(Self::ForgeDark),
                "vscode" => Some(Self::VSCode),
                "catppuccin-mocha" => Some(Self::CatppuccinMocha),
                "nord" => Some(Self::Nord),
                "dracula" => Some(Self::Dracula),
                "one-dark" => Some(Self::OneDark),
                _ => None,
            }
        }

        pub fn all() -> &'static [ThemeName] {
            &[
                Self::ForgeDark,
                Self::VSCode,
                Self::CatppuccinMocha,
                Self::Nord,
                Self::Dracula,
                Self::OneDark,
            ]
        }

        pub fn colors(&self) -> ThemeColors {
            match self {
                Self::ForgeDark => FORGE_DARK,
                Self::VSCode => VSCODE_DARK,
                Self::CatppuccinMocha => CATPPUCCIN_MOCHA,
                Self::Nord => NORD,
                Self::Dracula => DRACULA,
                Self::OneDark => ONE_DARK,
            }
        }
    }

    const FORGE_DARK: ThemeColors = ThemeColors {
        base: 0x0a0e14,
        mantle: 0x0d1117,
        surface0: 0x161b22,
        surface1: 0x21262d,
        text: 0xc9d1d9,
        subtext: 0x8b949e,
        blue: 0x569cd6,
        green: 0x6a9955,
        red: 0xf85149,
        yellow: 0xdcdcaa,
        lavender: 0xc586c0,
        overlay: 0x484f58,
        selection: 0x264f78,
        teal: 0x4ec9b0,
        peach: 0xce9178,
    };

    // VS Code Dark+ — exact colors from the default VS Code dark theme
    const VSCODE_DARK: ThemeColors = ThemeColors {
        base: 0x1e1e1e,       // editor.background
        mantle: 0x181818,     // sideBar.background
        surface0: 0x252526,   // editorWidget.background
        surface1: 0x3c3c3c,   // editorGroup.border
        text: 0xd4d4d4,       // editor.foreground
        subtext: 0x808080,    // editorLineNumber.foreground
        blue: 0x569cd6,       // keyword.type / storage
        green: 0x6a9955,      // comment
        red: 0xf44747,        // invalid / error
        yellow: 0xdcdcaa,     // entity.name.function
        lavender: 0xc586c0,   // keyword.control
        overlay: 0x5a5a5a,    // editorIndentGuide
        selection: 0x264f78,  // editor.selectionBackground
        teal: 0x4ec9b0,       // entity.name.type
        peach: 0xce9178,      // string
    };

    const CATPPUCCIN_MOCHA: ThemeColors = ThemeColors {
        base: 0x1e1e2e,
        mantle: 0x181825,
        surface0: 0x313244,
        surface1: 0x45475a,
        text: 0xcdd6f4,
        subtext: 0xa6adc8,
        blue: 0x89b4fa,
        green: 0xa6e3a1,
        red: 0xf38ba8,
        yellow: 0xf9e2af,
        lavender: 0xb4befe,
        overlay: 0x6c7086,
        selection: 0x364060,
        teal: 0x94e2d5,
        peach: 0xfab387,
    };

    const NORD: ThemeColors = ThemeColors {
        base: 0x2e3440,
        mantle: 0x272c36,
        surface0: 0x3b4252,
        surface1: 0x434c5e,
        text: 0xeceff4,
        subtext: 0xd8dee9,
        blue: 0x88c0d0,
        green: 0xa3be8c,
        red: 0xbf616a,
        yellow: 0xebcb8b,
        lavender: 0xb48ead,
        overlay: 0x4c566a,
        selection: 0x3b4f6a,
        teal: 0x8fbcbb,
        peach: 0xd08770,
    };

    const DRACULA: ThemeColors = ThemeColors {
        base: 0x282a36,
        mantle: 0x21222c,
        surface0: 0x343746,
        surface1: 0x44475a,
        text: 0xf8f8f2,
        subtext: 0xbfbfbf,
        blue: 0x8be9fd,
        green: 0x50fa7b,
        red: 0xff5555,
        yellow: 0xf1fa8c,
        lavender: 0xbd93f9,
        overlay: 0x6272a4,
        selection: 0x44475a,
        teal: 0x8be9fd,
        peach: 0xffb86c,
    };

    const ONE_DARK: ThemeColors = ThemeColors {
        base: 0x21252b,
        mantle: 0x1b1d23,
        surface0: 0x282c34,
        surface1: 0x353b45,
        text: 0xabb2bf,
        subtext: 0x7f848e,
        blue: 0x61afef,
        green: 0x98c379,
        red: 0xe06c75,
        yellow: 0xe5c07b,
        lavender: 0xc678dd,
        overlay: 0x4b5263,
        selection: 0x2c3545,
        teal: 0x56b6c2,
        peach: 0xd19a66,
    };

    struct ThemeState {
        name: ThemeName,
        colors: ThemeColors,
    }

    static CURRENT: Mutex<ThemeState> = Mutex::new(ThemeState {
        name: ThemeName::ForgeDark,
        colors: FORGE_DARK,
    });

    pub fn set_theme(name: ThemeName) {
        let mut state = CURRENT.lock().unwrap();
        state.name = name;
        state.colors = name.colors();
    }

    pub fn current_name() -> ThemeName {
        CURRENT.lock().unwrap().name
    }

    fn c() -> ThemeColors {
        CURRENT.lock().unwrap().colors
    }

    // ── Wallpaper state ──────────────────────────────
    static WALLPAPER: Mutex<Option<String>> = Mutex::new(None);
    static WALLPAPER_OPACITY: Mutex<f32> = Mutex::new(0.65);
    static TERMINAL_OPACITY: Mutex<f32> = Mutex::new(1.0);

    pub fn set_wallpaper(path: Option<String>) {
        *WALLPAPER.lock().unwrap() = path;
    }

    pub fn wallpaper() -> Option<String> {
        WALLPAPER.lock().unwrap().clone()
    }

    pub fn has_wallpaper() -> bool {
        WALLPAPER.lock().unwrap().is_some()
    }

    pub fn set_wallpaper_opacity(opacity: f32) {
        *WALLPAPER_OPACITY.lock().unwrap() = opacity.clamp(0.0, 1.0);
    }

    pub fn wallpaper_opacity() -> f32 {
        *WALLPAPER_OPACITY.lock().unwrap()
    }

    pub fn set_terminal_opacity(opacity: f32) {
        *TERMINAL_OPACITY.lock().unwrap() = opacity.clamp(0.0, 1.0);
    }

    pub fn terminal_opacity() -> f32 {
        *TERMINAL_OPACITY.lock().unwrap()
    }

    fn translucent(mut color: Rgba, alpha: f32) -> Rgba {
        if has_wallpaper() {
            // wallpaper_opacity: 0 = full wallpaper visible, 1 = fully transparent
            // At 0: UI panels use their alpha → wallpaper shows through
            // At 1: UI panels fully opaque → wallpaper hidden
            let t = wallpaper_opacity();
            color.a = alpha + (1.0 - alpha) * t;
        }
        color
    }

    // ── Color accessors ─────────────────────────────
    pub fn base() -> Rgba { translucent(rgb(c().base), 0.40) }
    pub fn mantle() -> Rgba { translucent(rgb(c().mantle), 0.55) }
    pub fn surface0() -> Rgba { translucent(rgb(c().surface0), 0.40) }
    pub fn surface1() -> Rgba { translucent(rgb(c().surface1), 0.40) }
    pub fn text() -> Rgba { rgb(c().text) }
    pub fn subtext() -> Rgba { rgb(c().subtext) }
    pub fn blue() -> Rgba { rgb(c().blue) }
    pub fn green() -> Rgba { rgb(c().green) }
    pub fn red() -> Rgba { rgb(c().red) }
    pub fn yellow() -> Rgba { rgb(c().yellow) }
    pub fn lavender() -> Rgba { rgb(c().lavender) }
    pub fn overlay() -> Rgba { rgb(c().overlay) }
    pub fn teal() -> Rgba { rgb(c().teal) }
    pub fn peach() -> Rgba { rgb(c().peach) }
    pub fn cursor() -> Rgba { rgb(c().blue) }
    pub fn selection() -> Rgba { rgb(c().selection) }

    // Translucent variants for wallpaper mode
    pub fn base_bg() -> Rgba { translucent(base(), 0.30) }
    pub fn mantle_bg() -> Rgba { translucent(rgb(c().mantle), 0.45) }
    pub fn surface0_bg() -> Rgba { translucent(surface0(), 0.40) }

    // Opaque variants (ignore wallpaper — for views that need solid backgrounds)
    pub fn base_solid() -> Rgba { rgb(c().base) }
    pub fn mantle_solid() -> Rgba { rgb(c().mantle) }
    pub fn surface0_solid() -> Rgba { rgb(c().surface0) }
    pub fn surface1_solid() -> Rgba { rgb(c().surface1) }
}
