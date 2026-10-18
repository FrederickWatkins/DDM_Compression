use super::super::message::Message;
use iced::widget::{container, text};
use iced::{Element, Fill};

pub(super) fn view(message: &str) -> Element<'_, Message> {
    container(text(message).size(13))
        .width(Fill)
        .padding([5, 10])
        .into()
}
