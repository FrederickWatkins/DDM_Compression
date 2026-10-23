//! Core application messages

use iced::{widget::pane_grid, window};
use slotmap::DefaultKey;

use crate::{
    subsystem::file::{DataType, FileType},
    ui::component::workspace,
};

/// Core app message types
#[derive(Debug, Clone)]
pub enum Message {
    /// Do nothing, workaround for button formatting
    Noop,
    /// Set/change window id
    WindowId(Option<window::Id>),
    /// Change active tab
    SelectTab(DefaultKey),
    /// Close tab
    CloseTab(DefaultKey),
    /// Message from workspace tab
    Workspace {
        /// Which tab the message came from/should go to
        tab: DefaultKey,
        /// The message
        message: workspace::Message,
    },
    /// Pane grid
    ResizeSidebar(pane_grid::ResizeEvent),
    /// Open a file picker for `FileType`
    OpenFile(FileType),
    /// Finished loading data
    DataLoaded {
        /// Name of file
        name: String,
        /// File data
        data: DataType,
    },
    /// Refresh progress bars
    ProgressTick,
    /// An error has occurred
    Error(String),
    /// Drag the window
    DragWindow,
    /// Minimize the window
    MinimizeWindow,
    /// Maximize the window
    MaximizeWindow,
    /// Close the window
    CloseWindow,
}
