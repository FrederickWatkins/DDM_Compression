//! Workspaces

use iced::{Element, Task, widget::space};
use tracing::error;

use crate::subsystem::file::DataType;

mod ddm_player;

/// Workspace message
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Live interactive DDM player
    DDMPlayer(ddm_player::Message),
    /// Viewer for single DDM
    DDMViewer,
}

/// Workspace type
#[derive(Debug, Clone, PartialEq)]
pub enum Workspace {
    /// Live interactive DDM player
    DDMPlayer(ddm_player::DDMPlayer),
    /// Viewer for single DDM
    DDMViewer,
}

impl Workspace {
    /// Create new workspace with data
    pub fn new(name: String, data: DataType) -> Self {
        match data {
            DataType::L0b(l0b) => Self::DDMPlayer(ddm_player::DDMPlayer::new(name, l0b)),
        }
    }

    /// Workspace name
    pub fn name(&self) -> String {
        match self {
            Self::DDMPlayer(ddm_player) => ddm_player.name(),
            Self::DDMViewer => todo!(),
        }
    }

    /// Update workspace with message
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match (self, message) {
            (Self::DDMPlayer(ddm_player), Message::DDMPlayer(message)) => {
                ddm_player.update(message).map(Message::DDMPlayer)
            }
            (s, m) => {
                error!(workspace=?s, message=?m, "Wrong message type for workspace");
                Task::none()
            }
        }
    }

    /// View workspace
    pub fn view(&self) -> Element<'_, Message> {
        match self {
            Self::DDMPlayer(ddm_player) => ddm_player.view().map(Message::DDMPlayer),
            Self::DDMViewer => todo!(),
        }
    }

    /// Sidebar content
    pub fn sidebar_content(&self) -> Element<'_, Message> {
        match self {
            Self::DDMPlayer(ddm_player) => ddm_player.sidebar_content().map(Message::DDMPlayer),
            Self::DDMViewer => space().into(),
        }
    }

    /// Required runtime subscriptions
    pub fn subscription(&self) -> iced::Subscription<Message> {
        match self {
            Self::DDMPlayer(ddm_player) => ddm_player.subscription().map(Message::DDMPlayer),
            Self::DDMViewer => todo!(),
        }
    }
}
