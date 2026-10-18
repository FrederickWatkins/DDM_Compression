use iced::{
    Color, Element, Length,
    widget::{Column, Container, Row, Space, container},
};
use ndarray::ArrayView2;

const DYNAMIC_RANGE_MAX: u16 = 5_000;

pub(crate) fn view<Message: 'static>(
    pixel_data: &ArrayView2<'_, i32>,
    pixel_width: f32,
    pixel_height: f32,
) -> Element<'static, Message> {
    let (height, width) = pixel_data.dim();
    let rows = (0..height)
        .map(|y| pixel_row(pixel_data, y, width, pixel_width, pixel_height).into())
        .collect::<Vec<_>>();

    Column::with_children(rows).spacing(0).into()
}

fn pixel_row<Message: 'static>(
    pixel_data: &ArrayView2<'_, i32>,
    y: usize,
    width: usize,
    pixel_width: f32,
    pixel_height: f32,
) -> Row<'static, Message> {
    let pixels = (0..width)
        .map(|x| pixel_view(pixel_data[[y, x]], pixel_width, pixel_height))
        .collect::<Vec<_>>();

    Row::with_children(pixels).spacing(0)
}

fn pixel_view<Message: 'static>(
    value: i32,
    pixel_width: f32,
    pixel_height: f32,
) -> Element<'static, Message> {
    let color = heat_color(value);
    Container::new(Space::new().width(Length::Fill).height(Length::Fill))
        .width(pixel_width)
        .height(pixel_height)
        .style(move |_theme| container::Style {
            background: Some(color.into()),
            ..container::Style::default()
        })
        .into()
}

fn heat_color(value: i32) -> Color {
    let capped = u16::try_from(value.unsigned_abs())
        .unwrap_or(DYNAMIC_RANGE_MAX)
        .min(DYNAMIC_RANGE_MAX);
    let amount = f32::from(capped) / f32::from(DYNAMIC_RANGE_MAX);

    // Dark blue → cyan → orange → yellow.
    let stops = [
        (0.0, Color::from_rgb(0.025, 0.035, 0.16)),
        (0.35, Color::from_rgb(0.0, 0.48, 0.82)),
        (0.70, Color::from_rgb(0.95, 0.23, 0.08)),
        (1.0, Color::from_rgb(1.0, 0.92, 0.22)),
    ];

    for pair in stops.windows(2) {
        let (start, first) = pair[0];
        let (end, second) = pair[1];
        if amount <= end {
            let local = ((amount - start) / (end - start)).clamp(0.0, 1.0);
            return Color::from_rgb(
                first.r + (second.r - first.r) * local,
                first.g + (second.g - first.g) * local,
                first.b + (second.b - first.b) * local,
            );
        }
    }

    Color::from_rgb(1.0, 0.92, 0.22)
}
