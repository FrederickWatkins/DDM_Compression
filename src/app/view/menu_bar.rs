use super::super::message::{MenuTarget, Message};
use iced::{
    Background, Border, Color, Element, Fill, Theme,
    widget::{Space, button, column, container, mouse_area, row, text},
};

/// Builds the File trigger in the custom title bar.
pub(super) fn view(open_menu: Option<MenuTarget>) -> Element<'static, Message> {
    let trigger = button(text("File  ›").size(13))
        .padding([4, 9])
        .style(move |theme, status| trigger_style(theme, status, open_menu.is_some()))
        .on_press(Message::MenuHover(Some(MenuTarget::File)));

    mouse_area(trigger)
        .on_enter(Message::MenuHover(Some(MenuTarget::File)))
        .into()
}

/// Builds the hover-open dropdown layer and its active child menu.
pub(super) fn dropdown(open_menu: Option<MenuTarget>) -> Element<'static, Message> {
    let Some(open_menu) = open_menu else {
        return Space::new().into();
    };

    let root_menu = root_menu(open_menu);
    let menus = match submenu(open_menu) {
        Some(child_menu) => row![root_menu, child_menu].spacing(2),
        None => row![root_menu],
    };

    mouse_area(menus).on_exit(Message::MenuHover(None)).into()
}

fn root_menu(open_menu: MenuTarget) -> Element<'static, Message> {
    container(
        column![
            submenu_row("Open", MenuTarget::Open, open_menu == MenuTarget::Open),
            submenu_row(
                "Export",
                MenuTarget::Export,
                open_menu == MenuTarget::Export
            ),
        ]
        .spacing(1),
    )
    .width(180)
    .padding(3)
    .style(menu_panel)
    .into()
}

fn submenu(target: MenuTarget) -> Option<Element<'static, Message>> {
    match target {
        MenuTarget::File => None,
        MenuTarget::Open => Some(action_menu([
            ("L0B data file", Message::OpenL0bRequested),
            ("DDM frame archive", Message::OpenArchiveRequested),
        ])),
        MenuTarget::Export => Some(action_menu([
            ("L0B data file", Message::ExportL0bRequested),
            ("DDM frame archive", Message::ExportArchiveRequested),
        ])),
    }
}

fn action_menu(items: [(&'static str, Message); 2]) -> Element<'static, Message> {
    let actions = items
        .into_iter()
        .map(|(label, message)| action_row(label, message))
        .collect::<Vec<_>>();

    container(column(actions).spacing(1))
        .width(220)
        .padding(3)
        .style(menu_panel)
        .into()
}

fn submenu_row(
    label: &'static str,
    target: MenuTarget,
    selected: bool,
) -> Element<'static, Message> {
    let trigger = button(row![
        text(label).size(13),
        Space::new().width(Fill),
        text("›").size(14),
    ])
    .width(Fill)
    .padding([5, 9])
    .style(move |theme, status| trigger_style(theme, status, selected));

    mouse_area(trigger)
        .on_enter(Message::MenuHover(Some(target)))
        .into()
}

fn action_row(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(13))
        .width(Fill)
        .padding([5, 9])
        .style(action_style)
        .on_press(message)
        .into()
}

fn trigger_style(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let background = match (selected, status) {
        (_, button::Status::Hovered | button::Status::Pressed) => {
            Some(Color::from_rgb8(65, 69, 76).into())
        }
        (true, _) => Some(Color::from_rgb8(51, 54, 60).into()),
        (false, _) => None,
    };

    button::Style {
        background,
        text_color: theme.palette().text,
        border: Border::default().rounded(5),
        ..button::Style::default()
    }
}

fn action_style(theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(Color::from_rgb8(62, 66, 73).into()),
        button::Status::Pressed => Some(Color::from_rgb8(73, 77, 85).into()),
        button::Status::Active | button::Status::Disabled => None,
    };

    button::Style {
        background,
        text_color: theme.palette().text,
        border: Border::default().rounded(5),
        ..button::Style::default()
    }
}

fn menu_panel(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(38, 40, 44))),
        border: Border::default()
            .color(Color::from_rgb8(77, 81, 88))
            .rounded(7),
        ..container::Style::default()
    }
}
