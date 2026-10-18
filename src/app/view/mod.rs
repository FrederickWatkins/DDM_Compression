pub(super) mod export_dialog;
mod menu_bar;
mod sidebar;
mod status_bar;
mod tabs;
mod workspace_view;

use super::{
    context::AppContext,
    message::{MenuTarget, Message},
    workspace::Workspace,
};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Theme, mouse,
    widget::{Column, Space, button, column, container, mouse_area, row, rule, svg, text},
    window,
};

/// Builds the overall window layout: custom title bar, tabs, sidebar + workspace, status bar.
pub(super) fn app_shell<'a>(
    context: &'a AppContext,
    workspace: &'a Workspace,
    menu_target: Option<MenuTarget>,
    export: Option<&'a export_dialog::State>,
) -> Element<'a, Message> {
    let layout = app_content(context, workspace, menu_target);
    let frame = resize_frame(layout);
    let menus = menu_layer(menu_target);

    let base_stack = iced::widget::stack![frame, menus].width(Fill).height(Fill);
    if let Some(export) = export {
        iced::widget::stack![
            base_stack,
            export_dialog::view(workspace, export).map(Message::ExportDialog)
        ]
        .width(Fill)
        .height(Fill)
        .into()
    } else {
        base_stack.into()
    }
}

fn app_content<'a>(
    context: &'a AppContext,
    workspace: &'a Workspace,
    menu_target: Option<MenuTarget>,
) -> Column<'a, Message> {
    column![
        title_bar(menu_target),
        rule::horizontal(1),
        tabs::view(workspace.tab_titles()).map(Message::Workspace),
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
}

fn resize_frame(content: Column<'_, Message>) -> Column<'_, Message> {
    column![
        resize_band(true),
        row![
            resize_handle(Length::Fixed(6.0), Fill, window::Direction::West),
            content,
            resize_handle(Length::Fixed(6.0), Fill, window::Direction::East),
        ]
        .height(Fill),
        resize_band(false),
    ]
    .width(Fill)
    .height(Fill)
}

fn menu_layer(menu_target: Option<MenuTarget>) -> Column<'static, Message> {
    column![
        Space::new().height(Length::Fixed(34.0)),
        row![
            Space::new().width(Length::Fixed(11.0)),
            menu_bar::dropdown(menu_target),
            Space::new().width(Fill),
        ],
        Space::new().height(Fill),
    ]
    .width(Fill)
    .height(Fill)
}

const MINIMIZE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 4h10v2H0z' fill='#FFFFFF'/></svg>";
const MAXIMIZE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 0h10v10H0V0zm1 1v8h8V1H1z' fill='#FFFFFF'/></svg>";
const CLOSE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M1.4 0L0 1.4 3.6 5 0 8.6 1.4 10 5 6.4 8.6 10 10 8.6 6.4 5 10 1.4 8.6 0 5 3.6 1.4 0z' fill='#FFFFFF'/></svg>";

fn title_bar(menu_target: Option<MenuTarget>) -> Element<'static, Message> {
    let controls = window_controls();
    let base = row![menu_bar::view(menu_target), title_drag_space(), controls]
        .height(Fill)
        .align_y(Alignment::Center);
    let title = container(text("DDM Workbench").size(13))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill);

    container(iced::widget::stack![base, title].width(Fill).height(Fill))
        .height(34)
        .width(Fill)
        .padding([2, 5])
        .into()
}

fn window_controls() -> Element<'static, Message> {
    mouse_area(
        row![
            window_control_button(MINIMIZE_SVG, Message::WindowMinimize),
            window_control_button(MAXIMIZE_SVG, Message::WindowToggleMaximize),
            window_control_button(CLOSE_SVG, Message::WindowClose),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .on_enter(Message::MenuHover(None))
    .into()
}

fn window_control_button(svg_data: &'static [u8], message: Message) -> Element<'static, Message> {
    let icon = svg(svg::Handle::from_memory(svg_data)).width(10).height(10);
    let button_content = container(icon)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill);

    button(button_content)
        .width(32)
        .height(27)
        .padding(0)
        .style(chrome_button)
        .on_press(message)
        .into()
}

fn title_drag_space() -> Element<'static, Message> {
    mouse_area(Space::new().width(Fill).height(Fill))
        .on_enter(Message::MenuHover(None))
        .on_press(Message::WindowDrag)
        .on_double_click(Message::WindowToggleMaximize)
        .into()
}

fn chrome_button(theme: &Theme, status: button::Status) -> button::Style {
    let background = theme.palette().background;
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);

    button::Style {
        background: Some(Background::Color(if hovered {
            Color::from_rgb8(67, 70, 76)
        } else {
            background
        })),
        text_color: theme.palette().text,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

fn resize_handle(
    width: Length,
    height: Length,
    direction: window::Direction,
) -> Element<'static, Message> {
    mouse_area(Space::new().width(width).height(height))
        .interaction(resize_interaction(direction))
        .on_press(Message::WindowResize(direction))
        .into()
}

fn resize_interaction(direction: window::Direction) -> mouse::Interaction {
    match direction {
        window::Direction::North | window::Direction::South => {
            mouse::Interaction::ResizingVertically
        }
        window::Direction::East | window::Direction::West => {
            mouse::Interaction::ResizingHorizontally
        }
        window::Direction::NorthEast | window::Direction::SouthWest => {
            mouse::Interaction::ResizingDiagonallyUp
        }
        window::Direction::NorthWest | window::Direction::SouthEast => {
            mouse::Interaction::ResizingDiagonallyDown
        }
    }
}

fn resize_band(top: bool) -> Element<'static, Message> {
    let (left, middle, right) = if top {
        (
            window::Direction::NorthWest,
            window::Direction::North,
            window::Direction::NorthEast,
        )
    } else {
        (
            window::Direction::SouthWest,
            window::Direction::South,
            window::Direction::SouthEast,
        )
    };

    row![
        resize_handle(Length::Fixed(6.0), Length::Fixed(6.0), left),
        resize_handle(Fill, Length::Fixed(6.0), middle),
        resize_handle(Length::Fixed(6.0), Length::Fixed(6.0), right),
    ]
    .width(Fill)
    .into()
}
