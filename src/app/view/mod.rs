mod menu_bar;
mod sidebar;
mod status_bar;
mod workspace;

use super::{context::AppContext, message::Message};
use iced::widget::{column, rule, row};
use iced::{Element, Fill};

/// Builds the overall window layout: menu bar, then sidebar + workspace, then status bar.
pub(super) fn app_shell(context: &AppContext) -> Element<'_, Message> {
    column![
        menu_bar::view(),
        rule::horizontal(1),
        row![sidebar::view(), rule::vertical(1), workspace::view(context)]
            .height(Fill)
            .spacing(0),
        rule::horizontal(1),
        status_bar::view(&context.status.message),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}
