use super::super::workspace::Message;
use iced::{
    Background, Border, Color, Element, Fill, Theme,
    widget::{Space, button, container, row, text},
};

const ACTIVE_TAB: Color = Color::from_rgb8(44, 47, 53);
const ACTIVE_BORDER: Color = Color::from_rgb8(136, 163, 198);

/// Builds the tab strip shown above the workspace area.
pub(super) fn view(tabs: Vec<(String, bool)>) -> Element<'static, Message> {
    let tabs = tabs
        .into_iter()
        .enumerate()
        .map(|(index, (title, active))| tab(index, title, active));
    let strip = row(tabs).spacing(5).push(Space::new().width(Fill));

    container(strip).width(Fill).padding([7, 9]).into()
}

fn tab(index: usize, title: String, active: bool) -> Element<'static, Message> {
    let select = button(text(title).size(13))
        .on_press(Message::Select(index))
        .padding([5, 8])
        .style(move |theme, status| tab_button_style(theme, status, active));
    let close = button(text("×").size(14))
        .on_press(Message::Close(index))
        .padding([4, 7])
        .style(close_button_style);
    let controls = row![select, close].spacing(2);

    container(controls)
        .padding([3, 4])
        .style(move |_theme| tab_surface_style(active))
        .into()
}

fn tab_button_style(theme: &Theme, status: button::Status, active: bool) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(Color::from_rgb8(58, 62, 69).into()),
        button::Status::Pressed => Some(Color::from_rgb8(69, 73, 81).into()),
        button::Status::Active | button::Status::Disabled => None,
    };

    button::Style {
        background,
        text_color: if active {
            theme.palette().text
        } else {
            Color::from_rgb8(184, 187, 192)
        },
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

fn close_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color) = match status {
        button::Status::Hovered => (
            Some(Color::from_rgb8(113, 55, 59).into()),
            theme.palette().text,
        ),
        button::Status::Pressed => (
            Some(Color::from_rgb8(139, 61, 66).into()),
            theme.palette().text,
        ),
        button::Status::Active | button::Status::Disabled => {
            (None, Color::from_rgb8(170, 173, 178))
        }
    };

    button::Style {
        background,
        text_color,
        border: Border::default().rounded(4),
        ..button::Style::default()
    }
}

fn tab_surface_style(active: bool) -> container::Style {
    let border = if active {
        Border::default().color(ACTIVE_BORDER).width(1).rounded(6)
    } else {
        Border::default().rounded(6)
    };

    container::Style {
        background: active.then_some(Background::Color(ACTIVE_TAB)),
        border,
        ..container::Style::default()
    }
}
