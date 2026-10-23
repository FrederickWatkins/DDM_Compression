//! Application styles

use iced::widget::container;

pub fn bar_style(theme: &iced::Theme) -> container::Style {
    let colour = theme.extended_palette().primary.weak;

    container::Style {
        background: Some(colour.color.into()),
        text_color: Some(colour.text),
        ..Default::default()
    }
}

pub fn sidebar_style(theme: &iced::Theme) -> container::Style {
    let colour = theme.extended_palette().secondary.weak;

    container::Style {
        background: Some(colour.color.into()),
        text_color: Some(colour.text),
        ..Default::default()
    }
}