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
use iced::widget::{Space, Stack, button, column, container, mouse_area, row, rule, text, svg};
use iced::{Background, Border, Color, Element, Fill, Length, Theme};

/// Builds the overall window layout: custom title bar, tabs, sidebar + workspace, status bar.
pub(super) fn app_shell<'a>(
    context: &'a AppContext,
    workspace: &'a Workspace,
    menu_target: Option<MenuTarget>,
) -> Element<'a, Message> {
    let content = column![
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
    .height(Fill);

    let base = column![
        horizontal_resize_band(true),
        row![
            resize_handle(Length::Fixed(6.0), Fill, iced::window::Direction::West),
            content,
            resize_handle(Length::Fixed(6.0), Fill, iced::window::Direction::East),
        ]
        .height(Fill),
        horizontal_resize_band(false),
    ]
    .width(Fill)
    .height(Fill);

    let dropdown = column![
        Space::new().height(Length::Fixed(34.0)),
        row![
            Space::new().width(Length::Fixed(11.0)),
            menu_bar::dropdown(menu_target),
            Space::new().width(Fill),
        ],
        Space::new().height(Fill),
    ]
    .width(Fill)
    .height(Fill);

    Stack::new()
        .push(base)
        .push(dropdown)
        .width(Fill)
        .height(Fill)
        .into()
}

const MINIMIZE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 4h10v2H0z' fill='#FFFFFF'/></svg>";
const MAXIMIZE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M0 0h10v10H0V0zm1 1v8h8V1H1z' fill='#FFFFFF'/></svg>";
const CLOSE_SVG: &[u8] = b"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 10 10'><path d='M1.4 0L0 1.4 3.6 5 0 8.6 1.4 10 5 6.4 8.6 10 10 8.6 6.4 5 10 1.4 8.6 0 5 3.6 1.4 0z' fill='#FFFFFF'/></svg>";

fn title_bar(menu_target: Option<MenuTarget>) -> Element<'static, Message> {
    let controls = mouse_area(
        row![
            window_control_button(
                svg::Handle::from_memory(MINIMIZE_SVG.to_vec()),
                Message::WindowMinimize,
            ),
            window_control_button(
                svg::Handle::from_memory(MAXIMIZE_SVG.to_vec()),
                Message::WindowToggleMaximize,
            ),
            window_control_button(
                svg::Handle::from_memory(CLOSE_SVG.to_vec()),
                Message::WindowClose,
            ),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .on_enter(Message::MenuHover(None));

    let title_bar = Stack::new()
        .push(
            row![menu_bar::view(menu_target), title_drag_space(), controls]
                .height(Fill)
                .align_y(iced::Alignment::Center),
        )
        .push(
            container(text("DDM Workbench").size(13))
                .width(Fill)
                .height(Fill)
                .center_x(Fill)
                .center_y(Fill),
        )
        .width(Fill)
        .height(Fill);

    container(title_bar)
        .height(34)
        .width(Fill)
        .padding([2, 5])
        .into()
}

/// Helper that wraps an SVG icon in a fully centered container inside the chrome button.
fn window_control_button(handle: svg::Handle, message: Message) -> Element<'static, Message> {
    let icon = svg(handle)
        .width(10)
        .height(10);

    button(
        container(icon)
            .width(Fill)
            .height(Fill)
            .center_x(Fill)
            .center_y(Fill),
    )
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
    direction: iced::window::Direction,
) -> Element<'static, Message> {
    let interaction = match direction {
        iced::window::Direction::North | iced::window::Direction::South => {
            iced::mouse::Interaction::ResizingVertically
        }
        iced::window::Direction::East | iced::window::Direction::West => {
            iced::mouse::Interaction::ResizingHorizontally
        }
        iced::window::Direction::NorthEast | iced::window::Direction::SouthWest => {
            iced::mouse::Interaction::ResizingDiagonallyUp
        }
        iced::window::Direction::NorthWest | iced::window::Direction::SouthEast => {
            iced::mouse::Interaction::ResizingDiagonallyDown
        }
    };

    mouse_area(Space::new().width(width).height(height))
        .interaction(interaction)
        .on_press(Message::WindowResize(direction))
        .into()
}

fn horizontal_resize_band(top: bool) -> Element<'static, Message> {
    let (left, middle, right) = if top {
        (
            iced::window::Direction::NorthWest,
            iced::window::Direction::North,
            iced::window::Direction::NorthEast,
        )
    } else {
        (
            iced::window::Direction::SouthWest,
            iced::window::Direction::South,
            iced::window::Direction::SouthEast,
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
