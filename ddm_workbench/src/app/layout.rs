//! Application main window layout
use std::fmt::Display;

use iced::{
    Alignment, Element, Font, Length, widget::{column, container, row, space, svg, text},
};

use crate::{L, S, app::Message, ui::widget::title_bar};
use crate::app::style::bar_style;

const LOGO: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo.svg"));

/// Layout main window
pub fn layout<'a>(
    window_title: impl Display,
    menu_bar: impl Into<Element<'a, Message>>,
    main_content: impl Into<Element<'a, Message>>,
    status_bar: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title_bar = container(title_bar(
        row![
            container(
                svg(svg::Handle::from_memory(LOGO))
                    .width(Length::Fixed(L))
                    .height(Length::Fixed(L))
            )
            .padding(S),
            container(menu_bar)
                .width(Length::Shrink)
                .height(Length::Shrink)
                .padding(S)
        ],
        container(text(window_title.to_string()).font(Font::MONOSPACE)).padding(S),
        space(),
        Message::DragWindow,
        Message::MinimizeWindow,
        Message::MaximizeWindow,
        Message::CloseWindow,
    )).style(bar_style);
    let status_bar = container(status_bar).height(Length::Fixed(L)).width(Length::Fill).style(bar_style).align_y(Alignment::Center);
    container(column![title_bar, main_content.into(), status_bar]).into()
}
