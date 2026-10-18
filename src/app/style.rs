use iced::{
    Border, Color, Theme,
    widget::slider::{self, HandleShape},
};

/// A slider palette that remains visible with the app's dark, low-contrast theme.
pub(super) fn slider_style(_theme: &Theme, status: slider::Status) -> slider::Style {
    let (filled_rail, handle) = match status {
        slider::Status::Active => (
            Color::from_rgb8(111, 116, 123),
            Color::from_rgb8(194, 197, 201),
        ),
        slider::Status::Hovered => (
            Color::from_rgb8(133, 138, 145),
            Color::from_rgb8(222, 224, 227),
        ),
        slider::Status::Dragged => (
            Color::from_rgb8(151, 156, 163),
            Color::from_rgb8(238, 239, 241),
        ),
    };

    slider::Style {
        rail: slider::Rail {
            backgrounds: (filled_rail.into(), Color::from_rgb8(57, 60, 66).into()),
            width: 4.0,
            border: Border::default().rounded(2),
        },
        handle: slider::Handle {
            shape: HandleShape::Circle { radius: 7.0 },
            background: handle.into(),
            border_width: 1.0,
            border_color: Color::from_rgb8(48, 51, 57),
        },
    }
}
