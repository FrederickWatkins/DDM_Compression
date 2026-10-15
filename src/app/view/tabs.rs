use super::super::workspace::Message;
use iced::{
    Element, Fill,
    widget::{button, container, row, text},
};

/// Builds the tab strip shown above the workspace area.
pub(super) fn view(tabs: Vec<(String, bool)>) -> Element<'static, Message> {
    let tabs = tabs
        .into_iter()
        .enumerate()
        .map(|(index, (title, active))| {
            let title = if active {
                format!("▣ {title}")
            } else {
                title
            };
            row![
                button(text(title).size(13))
                    .on_press(Message::Select(index))
                    .padding([4, 8]),
                button(text("×").size(13))
                    .on_press(Message::Close(index))
                    .padding([4, 7]),
            ]
            .spacing(2)
            .into()
        });

    container(row(tabs).spacing(4))
        .width(Fill)
        .padding([4, 8])
        .into()
}
