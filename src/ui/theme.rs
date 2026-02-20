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
    pub fn dark() -> Self {
        Theme {
            bg: Color::Rgb(18, 18, 27),
            fg: Color::Rgb(205, 214, 244),
            border: Color::Rgb(88, 91, 112),
            accent: Color::Rgb(137, 180, 250),
            user_msg: Color::Rgb(137, 220, 235),
            assistant_msg: Color::Rgb(205, 214, 244),
            system_msg: Color::Rgb(166, 227, 161),
            input_bg: Color::Rgb(30, 30, 46),
            input_fg: Color::Rgb(205, 214, 244),
            status_ok: Color::Rgb(166, 227, 161),
            status_err: Color::Rgb(243, 139, 168),
            highlight: Color::Rgb(203, 166, 247),
            title: Color::Rgb(137, 180, 250),
        }
    }

    pub fn light() -> Self {
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

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn focused_border_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.title)
            .add_modifier(Modifier::BOLD)
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

pub fn get_theme(name: &str) -> Theme {
    match name {
        "light" => Theme::light(),
        _ => Theme::dark(),
    }
}
