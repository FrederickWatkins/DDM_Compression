use super::super::message::{MenuTarget, Message};
use iced::widget::{Space, button, column, container, mouse_area, row, text};
use iced::{Background, Border, Color, Element, Fill, Theme};

/// Builds the File trigger in the custom title bar.
pub(super) fn view(open_menu: Option<MenuTarget>) -> Element<'static, Message> {
    mouse_area(
        button(text("File").size(13))
            .padding([3, 8])
            .style(move |theme, status| {
                if open_menu.is_some() {
                    selected_button(theme)
                } else {
                    super::chrome_button(theme, status)
                }
            })
            .on_press(Message::MenuHover(Some(MenuTarget::File))),
    )
    .on_enter(Message::MenuHover(Some(MenuTarget::File)))
    .into()
}

/// Builds the hover-open dropdown layer. Child menus are shown beside their parent row.
pub(super) fn dropdown(open_menu: Option<MenuTarget>) -> Element<'static, Message> {
    let Some(open_menu) = open_menu else {
        return Space::new().into();
    };

    let open_row = submenu_row("Open", MenuTarget::Open, open_menu == MenuTarget::Open);
    let export_row = submenu_row(
        "Export",
        MenuTarget::Export,
        open_menu == MenuTarget::Export,
    );

    let root_menu = container(column![open_row, export_row].spacing(1))
        .width(180)
        .padding(3)
        .style(menu_panel);

    let submenu = match open_menu {
        MenuTarget::Open => Some(
            container(
                column![
                    leaf_row("L0B data file", Message::OpenL0bRequested),
                    leaf_row("DDM frame archive", Message::OpenArchiveRequested),
                ]
                .spacing(1),
            )
            .width(220)
            .padding(3)
            .style(menu_panel),
        ),
        MenuTarget::Export => Some(
            container(
                column![
                    leaf_row("L0B data file", Message::ExportL0bRequested),
                    leaf_row("DDM frame archive", Message::ExportArchiveRequested),
                ]
                .spacing(1),
            )
            .width(220)
            .padding(3)
            .style(menu_panel),
        ),
        MenuTarget::File => None,
    };

    let menus = if let Some(submenu) = submenu {
        row![root_menu, submenu].spacing(2)
    } else {
        row![root_menu]
    };

    mouse_area(menus).on_exit(Message::MenuHover(None)).into()
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
    .padding([4, 8])
    .style(move |theme, status| {
        if selected {
            selected_button(theme)
        } else {
            super::chrome_button(theme, status)
        }
    });

    mouse_area(trigger)
        .on_enter(Message::MenuHover(Some(target)))
        .into()
}

fn leaf_row(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(13))
        .width(Fill)
        .padding([4, 8])
        .style(super::chrome_button)
        .on_press(message)
        .into()
}

fn selected_button(theme: &Theme) -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb8(67, 70, 76))),
        text_color: theme.palette().text,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

fn menu_panel(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(theme.palette().background)),
        border: Border::default()
            .color(Color::from_rgb8(67, 70, 76))
            .rounded(4),
        ..container::Style::default()
    }
}
