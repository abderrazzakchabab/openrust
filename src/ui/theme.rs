use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub accent: Color,
    pub user_msg: Color,
    pub assistant_msg: Color,
    pub system_msg: Color,
    pub input_bg: Color,
    pub input_fg: Color,
    pub status_ok: Color,
    pub status_err: Color,
    pub highlight: Color,
    pub title: Color,
}

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Theme {
            bg: Color::Rgb(30, 30, 46),
            fg: Color::Rgb(205, 214, 244),
            border: Color::Rgb(88, 91, 112),
            accent: Color::Rgb(137, 180, 250),
            user_msg: Color::Rgb(137, 220, 235),
            assistant_msg: Color::Rgb(205, 214, 244),
            system_msg: Color::Rgb(166, 227, 161),
            input_bg: Color::Rgb(17, 17, 27),
            input_fg: Color::Rgb(205, 214, 244),
            status_ok: Color::Rgb(166, 227, 161),
            status_err: Color::Rgb(243, 139, 168),
            highlight: Color::Rgb(203, 166, 247),
            title: Color::Rgb(137, 180, 250),
        }
    }

    pub fn catppuccin_latte() -> Self {
        Theme {
            bg: Color::Rgb(239, 241, 245),
            fg: Color::Rgb(76, 79, 105),
            border: Color::Rgb(172, 176, 190),
            accent: Color::Rgb(30, 102, 245),
            user_msg: Color::Rgb(23, 146, 153),
            assistant_msg: Color::Rgb(76, 79, 105),
            system_msg: Color::Rgb(64, 160, 43),
            input_bg: Color::Rgb(220, 224, 232),
            input_fg: Color::Rgb(76, 79, 105),
            status_ok: Color::Rgb(64, 160, 43),
            status_err: Color::Rgb(210, 15, 57),
            highlight: Color::Rgb(136, 57, 239),
            title: Color::Rgb(30, 102, 245),
        }
    }

    pub fn dracula() -> Self {
        Theme {
            bg: Color::Rgb(40, 42, 54),
            fg: Color::Rgb(248, 248, 242),
            border: Color::Rgb(98, 114, 164),
            accent: Color::Rgb(189, 147, 249),
            user_msg: Color::Rgb(139, 233, 253),
            assistant_msg: Color::Rgb(248, 248, 242),
            system_msg: Color::Rgb(80, 250, 123),
            input_bg: Color::Rgb(68, 71, 90),
            input_fg: Color::Rgb(248, 248, 242),
            status_ok: Color::Rgb(80, 250, 123),
            status_err: Color::Rgb(255, 85, 85),
            highlight: Color::Rgb(255, 121, 198),
            title: Color::Rgb(189, 147, 249),
        }
    }

    pub fn tokyo_night() -> Self {
        Theme {
            bg: Color::Rgb(26, 27, 38),
            fg: Color::Rgb(192, 202, 245),
            border: Color::Rgb(86, 95, 137),
            accent: Color::Rgb(122, 162, 247),
            user_msg: Color::Rgb(125, 207, 255),
            assistant_msg: Color::Rgb(192, 202, 245),
            system_msg: Color::Rgb(158, 206, 106),
            input_bg: Color::Rgb(36, 40, 59),
            input_fg: Color::Rgb(192, 202, 245),
            status_ok: Color::Rgb(158, 206, 106),
            status_err: Color::Rgb(247, 118, 142),
            highlight: Color::Rgb(187, 154, 247),
            title: Color::Rgb(122, 162, 247),
        }
    }

    pub fn solarized_dark() -> Self {
        Theme {
            bg: Color::Rgb(0, 43, 54),
            fg: Color::Rgb(131, 148, 150),
            border: Color::Rgb(88, 110, 117),
            accent: Color::Rgb(38, 139, 210),
            user_msg: Color::Rgb(42, 161, 152),
            assistant_msg: Color::Rgb(131, 148, 150),
            system_msg: Color::Rgb(133, 153, 0),
            input_bg: Color::Rgb(7, 54, 66),
            input_fg: Color::Rgb(131, 148, 150),
            status_ok: Color::Rgb(133, 153, 0),
            status_err: Color::Rgb(220, 50, 47),
            highlight: Color::Rgb(211, 54, 130),
            title: Color::Rgb(38, 139, 210),
        }
    }

    pub fn solarized_light() -> Self {
        Theme {
            bg: Color::Rgb(253, 246, 227),
            fg: Color::Rgb(101, 123, 131),
            border: Color::Rgb(147, 161, 161),
            accent: Color::Rgb(38, 139, 210),
            user_msg: Color::Rgb(42, 161, 152),
            assistant_msg: Color::Rgb(101, 123, 131),
            system_msg: Color::Rgb(133, 153, 0),
            input_bg: Color::Rgb(238, 232, 213),
            input_fg: Color::Rgb(101, 123, 131),
            status_ok: Color::Rgb(133, 153, 0),
            status_err: Color::Rgb(220, 50, 47),
            highlight: Color::Rgb(211, 54, 130),
            title: Color::Rgb(38, 139, 210),
        }
    }

    pub fn nord() -> Self {
        Theme {
            bg: Color::Rgb(46, 52, 64),
            fg: Color::Rgb(216, 222, 233),
            border: Color::Rgb(76, 86, 106),
            accent: Color::Rgb(136, 192, 208),
            user_msg: Color::Rgb(129, 161, 193),
            assistant_msg: Color::Rgb(216, 222, 233),
            system_msg: Color::Rgb(163, 190, 140),
            input_bg: Color::Rgb(59, 66, 82),
            input_fg: Color::Rgb(216, 222, 233),
            status_ok: Color::Rgb(163, 190, 140),
            status_err: Color::Rgb(191, 97, 106),
            highlight: Color::Rgb(180, 142, 173),
            title: Color::Rgb(136, 192, 208),
        }
    }

    pub fn gruvbox() -> Self {
        Theme {
            bg: Color::Rgb(40, 40, 40),
            fg: Color::Rgb(235, 219, 178),
            border: Color::Rgb(146, 131, 116),
            accent: Color::Rgb(131, 165, 152),
            user_msg: Color::Rgb(142, 192, 124),
            assistant_msg: Color::Rgb(235, 219, 178),
            system_msg: Color::Rgb(184, 187, 38),
            input_bg: Color::Rgb(60, 56, 54),
            input_fg: Color::Rgb(235, 219, 178),
            status_ok: Color::Rgb(184, 187, 38),
            status_err: Color::Rgb(251, 73, 52),
            highlight: Color::Rgb(211, 134, 155),
            title: Color::Rgb(131, 165, 152),
        }
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn focused_border_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn title_style(&self) -> Style {
        Style::default().fg(self.title).add_modifier(Modifier::BOLD)
    }

    pub fn user_style(&self) -> Style {
        Style::default()
            .fg(self.user_msg)
            .add_modifier(Modifier::BOLD)
    }

    pub fn assistant_style(&self) -> Style {
        Style::default().fg(self.assistant_msg)
    }

    pub fn input_style(&self) -> Style {
        Style::default().fg(self.input_fg).bg(self.input_bg)
    }

    pub fn status_style(&self, is_error: bool) -> Style {
        if is_error {
            Style::default().fg(self.status_err)
        } else {
            Style::default().fg(self.status_ok)
        }
    }
}

/// Returns a list of all available theme names
pub fn list_themes() -> Vec<&'static str> {
    vec![
        "catppuccin-mocha",
        "catppuccin-latte",
        "dracula",
        "tokyo-night",
        "solarized-dark",
        "solarized-light",
        "nord",
        "gruvbox",
    ]
}

/// Get a theme by name (case-insensitive)
/// Supports aliases: "dark" → "catppuccin-mocha", "light" → "catppuccin-latte"
/// Returns default theme (catppuccin-mocha) for unknown names
pub fn get_theme(name: &str) -> Theme {
    match name.to_lowercase().as_str() {
        "catppuccin-mocha" | "dark" => Theme::catppuccin_mocha(),
        "catppuccin-latte" | "light" => Theme::catppuccin_latte(),
        "dracula" => Theme::dracula(),
        "tokyo-night" => Theme::tokyo_night(),
        "solarized-dark" => Theme::solarized_dark(),
        "solarized-light" => Theme::solarized_light(),
        "nord" => Theme::nord(),
        "gruvbox" => Theme::gruvbox(),
        _ => Theme::catppuccin_mocha(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_themes_load() {
        let themes = list_themes();
        for theme_name in themes {
            let theme = get_theme(theme_name);
            assert_ne!(theme.bg, Color::Reset);
        }
    }

    #[test]
    fn test_theme_aliases() {
        let dark_theme = get_theme("dark");
        let mocha_theme = get_theme("catppuccin-mocha");
        assert_eq!(
            format!("{:?}", dark_theme.bg),
            format!("{:?}", mocha_theme.bg)
        );

        let light_theme = get_theme("light");
        let latte_theme = get_theme("catppuccin-latte");
        assert_eq!(
            format!("{:?}", light_theme.bg),
            format!("{:?}", latte_theme.bg)
        );
    }

    #[test]
    fn test_unknown_theme_defaults() {
        let unknown = get_theme("nonexistent-theme");
        let default = get_theme("catppuccin-mocha");
        assert_eq!(format!("{:?}", unknown.bg), format!("{:?}", default.bg));
    }

    #[test]
    fn test_list_themes() {
        let themes = list_themes();
        assert_eq!(themes.len(), 8);
        assert!(themes.contains(&"catppuccin-mocha"));
        assert!(themes.contains(&"catppuccin-latte"));
        assert!(themes.contains(&"dracula"));
        assert!(themes.contains(&"tokyo-night"));
        assert!(themes.contains(&"solarized-dark"));
        assert!(themes.contains(&"solarized-light"));
        assert!(themes.contains(&"nord"));
        assert!(themes.contains(&"gruvbox"));
    }

    #[test]
    fn test_theme_colors_distinct() {
        let mocha = get_theme("catppuccin-mocha");
        let latte = get_theme("catppuccin-latte");
        let dracula = get_theme("dracula");
        let tokyo = get_theme("tokyo-night");

        assert_ne!(format!("{:?}", mocha.bg), format!("{:?}", latte.bg));
        assert_ne!(format!("{:?}", mocha.bg), format!("{:?}", dracula.bg));
        assert_ne!(format!("{:?}", mocha.bg), format!("{:?}", tokyo.bg));
        assert_ne!(format!("{:?}", latte.bg), format!("{:?}", dracula.bg));
    }

    #[test]
    fn test_theme_struct_complete() {
        let theme = get_theme("catppuccin-mocha");
        assert_ne!(theme.bg, Color::Reset);
        assert_ne!(theme.fg, Color::Reset);
        assert_ne!(theme.border, Color::Reset);
        assert_ne!(theme.accent, Color::Reset);
        assert_ne!(theme.user_msg, Color::Reset);
        assert_ne!(theme.assistant_msg, Color::Reset);
        assert_ne!(theme.system_msg, Color::Reset);
        assert_ne!(theme.input_bg, Color::Reset);
        assert_ne!(theme.input_fg, Color::Reset);
        assert_ne!(theme.status_ok, Color::Reset);
        assert_ne!(theme.status_err, Color::Reset);
        assert_ne!(theme.highlight, Color::Reset);
        assert_ne!(theme.title, Color::Reset);
    }

    #[test]
    fn test_case_insensitive_theme_names() {
        let lower = get_theme("dracula");
        let upper = get_theme("DRACULA");
        let mixed = get_theme("DrAcUlA");
        assert_eq!(format!("{:?}", lower.bg), format!("{:?}", upper.bg));
        assert_eq!(format!("{:?}", lower.bg), format!("{:?}", mixed.bg));
    }

    #[test]
    fn test_theme_helper_methods() {
        let theme = get_theme("catppuccin-mocha");
        let border = theme.border_style();
        let focused = theme.focused_border_style();
        let title = theme.title_style();
        let user = theme.user_style();
        let assistant = theme.assistant_style();
        let input = theme.input_style();
        let status_ok = theme.status_style(false);
        let status_err = theme.status_style(true);

        assert_eq!(border.fg, Some(theme.border));
        assert_eq!(focused.fg, Some(theme.accent));
        assert_eq!(title.fg, Some(theme.title));
        assert_eq!(user.fg, Some(theme.user_msg));
        assert_eq!(assistant.fg, Some(theme.assistant_msg));
        assert_eq!(input.fg, Some(theme.input_fg));
        assert_eq!(input.bg, Some(theme.input_bg));
        assert_eq!(status_ok.fg, Some(theme.status_ok));
        assert_eq!(status_err.fg, Some(theme.status_err));
    }
}
