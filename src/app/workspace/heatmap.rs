use iced::{
    Color, Element, Length,
    widget::{Column, Container, Row},
};
use ndarray::ArrayView2;

const DYNAMIC_RANGE_MAX: i32 = 5_000;

pub(crate) fn view<'a, Message: 'a>(
    pixel_data: ArrayView2<i32>,
    pixel_width: f32,
    pixel_height: f32,
) -> Element<'a, Message> {
    let (height, width) = pixel_data.dim();

    let rows = (0..height).map(|y| {
        let pixels = (0..width).map(|x| {
            let color = heat_color(pixel_data[[y, x]]);

            Container::new(
                iced::widget::Space::new()
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .width(pixel_width)
            .height(pixel_height)
            .style(move |_theme| iced::widget::container::Style {
                background: Some(color.into()),
                ..Default::default()
            })
            .into()
        });

        Row::with_children(pixels.collect::<Vec<_>>())
            .spacing(0)
            .into()
    });

    Column::with_children(rows.collect::<Vec<_>>())
        .spacing(0)
        .into()
}

fn heat_color(value: i32) -> Color {
    let amount = value.abs().min(DYNAMIC_RANGE_MAX) as f32 / DYNAMIC_RANGE_MAX as f32;

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
