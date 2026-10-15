use super::super::{context::AppContext, message::Message};
use iced::widget::{column, container, text};
use iced::{Alignment, Element, Fill};

/// Main work area. It takes all horizontal space left after the sidebar.
pub(super) fn view(context: &AppContext) -> Element<'_, Message> {
    container(
        column![
            text("Workspace").size(26),
            text(context.document.display_name()).size(16),
            text("Your editor, visualisation, or document tabs can live here."),
        ]
        .spacing(12)
        .align_x(Alignment::Start),
    )
    .width(Fill)
    .height(Fill)
    .padding(24)
    .into()
}
