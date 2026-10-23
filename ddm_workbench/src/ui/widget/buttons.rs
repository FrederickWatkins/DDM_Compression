//! Buttons
use iced::{Element, Theme, widget::{Button, button}};

/// Circular button
pub fn circular_button<'a, Message: Clone + 'a>(content: impl Into<Element<'a, Message>>, size: f32) -> Button<'a, Message> {
    button(content).width(size).height(size).style(move |theme: &Theme, status| {
            let mut style = button::primary(theme, status);
            style.border.radius = (size / 2.0).into();
            style
        })
}