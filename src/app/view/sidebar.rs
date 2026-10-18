use super::super::{message::Message, workspace::Workspace};
use iced::Element;

/// Displays metadata belonging to the currently active workspace.
pub(super) fn view(workspace: &Workspace) -> Element<'_, Message> {
    workspace.sidebar_view().map(Message::Workspace)
}
