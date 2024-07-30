use iced::widget::button::Status;
use iced::widget::container::Style;
use iced::{theme, Border, Color, Font, Theme, overlay};
use iced::widget::pick_list;

pub const ICON_BYTES: &[u8] = include_bytes!("fonts/icons.ttf");
pub const ICON_FONT: Font = Font::with_name("icons");

pub enum Icon {
    Locked,
    Unlocked,
    ResizeFull,
    ResizeSmall,
    Close,
    Layout,
    Cog,
}

impl From<Icon> for char {
    fn from(icon: Icon) -> Self {
        match icon {
            Icon::Unlocked => '\u{E800}',
            Icon::Locked => '\u{E801}',
            Icon::ResizeFull => '\u{E802}',
            Icon::ResizeSmall => '\u{E803}',
            Icon::Close => '\u{E804}',
            Icon::Layout => '\u{E805}',
            Icon::Cog => '\u{E806}',
        }
    }
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
        text_color: Some(palette.background.base.text),
        background: Some(Color::BLACK.into()),
        ..Default::default()
    }
}
pub fn title_bar_focused(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.weak.text),
        background: Some(Color::TRANSPARENT.into()),
        ..Default::default()
    }
}
pub fn pane_active(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(Color::BLACK.into()),
        ..Default::default()
    }
}
pub fn pane_focused(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.weak.text),
        background: Some(Color::BLACK.into()),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

pub fn chart_modal(theme: &Theme) -> Style {
    let palette = theme.extended_palette();

    Style {
        text_color: Some(palette.background.base.text),
        background: Some(palette.background.base.color.into()),
        border: Border {
            width: 1.0,
            color: palette.background.weak.color,
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

pub fn button_primary(theme: &Theme, status: Status) -> iced::widget::button::Style {
    let palette = theme.extended_palette();

    match status {
        Status::Active => iced::widget::button::Style {
            background: Some(Color::BLACK.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        Status::Pressed => iced::widget::button::Style {
            background: Some(Color::BLACK.into()),
            text_color: palette.background.base.text,
            border: Border {
                color: palette.primary.weak.color,
                width: 2.0,
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        Status::Hovered => iced::widget::button::Style {
            background: Some(Color::BLACK.into()),
            text_color: palette.background.weak.text,
            border: Border {
                color: palette.primary.strong.color,
                width: 1.0,
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        },
        Status::Disabled => iced::widget::button::Style {
            background: Some(Color::BLACK.into()),
            text_color: palette.background.base.text,
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub fn picklist_primary(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let palette = theme.extended_palette();
    
    match status {
        pick_list::Status::Active => pick_list::Style {
            text_color: palette.background.base.text,
            placeholder_color: palette.background.base.text,
            handle_color: palette.background.base.text,
            background: palette.background.base.color.into(),
            border: Border {
                radius: 3.0.into(),
                width: 1.0,
                color: palette.background.weak.color,
                ..Default::default()
            },
        },
        pick_list::Status::Opened => pick_list::Style {
            text_color: palette.background.base.text,
            placeholder_color: palette.background.base.text,
            handle_color: palette.background.base.text,
            background: palette.background.base.color.into(),
            border: Border {
                radius: 3.0.into(),
                width: 1.0,
                color: palette.primary.base.color,
                ..Default::default()
            },
        },
        pick_list::Status::Hovered => pick_list::Style {
            text_color: palette.background.weak.text,
            placeholder_color: palette.background.weak.text,
            handle_color: palette.background.weak.text,
            background: palette.background.base.color.into(),
            border: Border {
                radius: 3.0.into(),
                width: 1.0,
                color: palette.primary.strong.color,
                ..Default::default()
            },
        },
    }
}

pub fn picklist_menu_primary(theme: &Theme) -> overlay::menu::Style {
    let palette = theme.extended_palette();

    overlay::menu::Style {
        text_color: palette.background.base.text,
        background: palette.background.base.color.into(),
        border: Border {
            radius: 3.0.into(),
            width: 1.0,
            color: palette.background.base.color,
            ..Default::default()
        },
        selected_text_color: palette.background.weak.text,
        selected_background: palette.secondary.weak.color.into(),
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