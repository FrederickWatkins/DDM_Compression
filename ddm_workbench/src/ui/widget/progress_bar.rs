use iced::{Element, Length, alignment::Vertical, Font, widget::{self, row, space, text}};

use crate::{M, S, XL, XS};

pub fn progress_bar<'a, Message: 'a>(name: String, fraction: f32) -> Element<'a, Message> {
    row![
        space().width(XS),
        text(name).size(M).font(Font::MONOSPACE),
        widget::progress_bar(0.0..=1.0, fraction).girth(Length::Fixed(S)).length(Length::Fixed(XL)),
    ].align_y(Vertical::Center).spacing(S).into()
}