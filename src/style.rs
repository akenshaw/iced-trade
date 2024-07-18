use iced::widget::container::Style;
use iced::{theme, Border, Color, Theme};

fn styled(pair: theme::palette::Pair) -> Style {
    Style {
        background: Some(pair.color.into()),
        text_color: pair.text.into(),
        ..Default::default()
    }
}

pub fn primary(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    styled(palette.primary.weak)
}

pub fn tooltip(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(palette.background.weak.color.into()),
        border: Border {
            width: 1.0,
            color: palette.primary.weak.color,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

pub fn title_bar_active(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.strong.text),
        background: Some(palette.background.strong.color.into()),
        border: Border {
            width: 1.0,
            color: palette.primary.strong.color,
            radius: 4.0.into(), 
        },
        ..Default::default()
    }
}
pub fn title_bar_focused(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.primary.strong.text),
        background: Some(palette.primary.strong.color.into()),
        ..Default::default()
    }
}
pub fn pane_active(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(Color::BLACK.into()),
        border: Border {
            width: 1.0,
            color: palette.background.strong.color,
            ..Border::default()
        },
        ..Default::default()
    }
}
pub fn pane_focused(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        background: Some(Color::BLACK.into()),
        border: Border {
            width: 1.0,
            color: palette.primary.strong.color,
            ..Border::default()
        },
        ..Default::default()
    }
}
pub fn sell_side_red(color_alpha: f32) -> Style {
    Style {
        text_color: Color::from_rgba(192.0 / 255.0, 80.0 / 255.0, 77.0 / 255.0, 1.0).into(),
        border: Border {
            width: 1.0,
            color: Color::from_rgba(192.0 / 255.0, 80.0 / 255.0, 77.0 / 255.0, color_alpha),
            ..Border::default()
        },
        ..Default::default()
    }
}

pub fn buy_side_green(color_alpha: f32) -> Style {
    Style {
        text_color: Color::from_rgba(81.0 / 255.0, 205.0 / 255.0, 160.0 / 255.0, 1.0).into(),
        border: Border {
            width: 1.0,
            color: Color::from_rgba(81.0 / 255.0, 205.0 / 255.0, 160.0 / 255.0, color_alpha),
            ..Border::default()
        },
        ..Default::default()
    }
}