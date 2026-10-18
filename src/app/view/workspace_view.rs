use super::super::{message::Message, workspace::Workspace};
use iced::Element;

/// Displays the active workspace component, or the initial welcome view.
pub(super) fn view(workspace: &Workspace) -> Element<'_, Message> {
    workspace.view().map(Message::Workspace)
}
