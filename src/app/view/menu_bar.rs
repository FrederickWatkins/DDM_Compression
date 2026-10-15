use super::super::message::Message;
use iced::widget::{button, container};
use iced::{Element, Fill};
use iced_aw::menu::{Item, Menu, MenuBar};

/// Traditional application menu bar. Add further top-level menus here as needed.
pub(super) fn view() -> Element<'static, Message> {
    let l0b_file_item = Item::new(
        button("L0B data file…")
            .width(Fill)
            .padding([6, 12])
            .on_press(Message::OpenL0bRequested),
    );

    let archive_item = Item::new(
        button("DDM frame archive…")
            .width(Fill)
            .padding([6, 12])
            .on_press(Message::OpenArchiveRequested),
    );

    // The Open item owns a submenu, so the menu path is File -> Open -> file type.
    let open_menu = Item::with_menu(
        button("Open").width(Fill).padding([6, 12]),
        Menu::new(vec![l0b_file_item, archive_item])
            .max_width(220.0)
            .offset(2.0),
    );

    let file_menu = Item::with_menu(
        button("File").padding([6, 10]),
        Menu::new(vec![open_menu]).max_width(180.0).offset(2.0),
    );

    container(MenuBar::new(vec![file_menu]))
        .width(Fill)
        .padding([3, 6])
        .into()
}