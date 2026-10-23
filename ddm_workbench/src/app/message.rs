use crate::subsystem::file::{DataType, FileType};

use iced::window;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Noop,
    WindowId(Option<window::Id>),
    Workspace(crate::ui::component::workspace::Message),
    OpenFile(FileType),
    DataLoaded(DataType),
    ProgressTick,
    Error(String),
    DragWindow,
    Minimize,
    Maximize,
    CloseWindow,
}