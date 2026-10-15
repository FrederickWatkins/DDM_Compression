use super::super::message::Message;
use iced::widget::{Space, container};
use iced::{Element, Fill};

const SIDEBAR_WIDTH: f32 = 240.0;

/// Intentionally empty for now, but owns a fixed-width region on the left.
pub(super) fn view() -> Element<'static, Message> {
    container(Space::new().width(Fill).height(Fill))
        .width(SIDEBAR_WIDTH)
        .height(Fill)
        .into()
}
