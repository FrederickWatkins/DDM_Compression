//! Title bar for window
use iced::{
    Alignment, Element, Length,
    widget::{container, mouse_area, row, space, stack, svg},
};

use crate::{L, S, ui::widget::buttons::circular_button};

const MINIMIZE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <path d="M3 8h10" fill="none" stroke="#ffffff"
        stroke-width="1.5" stroke-linecap="round"/>
</svg>
"##;

const MAXIMIZE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <rect x="3.25" y="3.25" width="9.5" height="9.5"
        fill="none" stroke="#ffffff" stroke-width="1.4"
        rx="1"/>
</svg>
"##;

const CLOSE_ICON: &[u8] = br##"
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16">
  <path d="M4 4l8 8M12 4l-8 8"
        fill="none" stroke="#ffffff"
        stroke-width="1.5" stroke-linecap="round"/>
</svg>
"##;

/// Title bar for window
pub fn title_bar<'a, Message: Clone + 'a>(
    left_content: impl Into<Element<'a, Message>>,
    center_content: impl Into<Element<'a, Message>>,
    right_content: impl Into<Element<'a, Message>>,
    on_drag: Message,
    on_minimize: Message,
    on_maximize: Message,
    on_close: Message,
) -> Element<'a, Message> {
    let sides = row![
        left_content.into(),
        // Expand to occupy the space between the two sides.
        mouse_area(space().width(Length::Fill).height(Length::Fill)).on_press(on_drag),
        right_content.into(),
        window_buttons(on_minimize, on_maximize, on_close),
    ]
    .width(Length::Fill)
    .align_y(Alignment::Center);

    let centered = container(center_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill);

    container(stack![sides, centered,].width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Shrink)
        .into()
}

fn window_buttons<'a, Message: Clone + 'a>(
    on_minimize: Message,
    on_maximize: Message,
    on_close: Message,
) -> Element<'a, Message> {
    row![
        container(
            circular_button(
                svg(svg::Handle::from_memory(MINIMIZE_ICON))
                    .width(Length::Fixed(0.9 * L))
                    .height(Length::Fixed(0.9 * L)),
                L
            )
            .on_press(on_minimize)
        )
        .padding(S),
        container(
            circular_button(
                svg(svg::Handle::from_memory(MAXIMIZE_ICON))
                    .width(Length::Fixed(0.9 * L))
                    .height(Length::Fixed(0.9 * L)),
                L
            )
            .on_press(on_maximize)
        )
        .padding(S),
        container(
            circular_button(
                svg(svg::Handle::from_memory(CLOSE_ICON))
                    .width(Length::Fixed(0.9 * L))
                    .height(Length::Fixed(0.9 * L)),
                L
            )
            .on_press(on_close)
        )
        .padding(S),
    ]
    .into()
}
