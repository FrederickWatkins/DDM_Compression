mod menu_bar;
mod sidebar;
mod status_bar;
mod workspace_view;

use super::{context::AppContext, message::Message, workspace::Workspace};
use iced::widget::{column, row, rule};
use iced::{Element, Fill};

/// Builds the overall window layout: menu bar, tabs, sidebar + workspace, status bar.
pub(super) fn app_shell<'a>(
    context: &'a AppContext,
    workspace: &'a Workspace,
) -> Element<'a, Message> {
    column![
        menu_bar::view(),
        rule::horizontal(1),
        workspace.tabs_view().map(Message::Workspace),
        rule::horizontal(1),
        row![
            sidebar::view(workspace),
            rule::vertical(1),
            workspace_view::view(workspace),
        ]
        .height(Fill)
        .spacing(0),
        rule::horizontal(1),
        status_bar::view(&context.status.message),
    ]
    .width(Fill)
    .height(Fill)
    .into()
}
