use ndarray::Array2;
use ratatui::{Frame, buffer::Buffer, layout::Rect, style::Color, widgets::Widget};

use crate::App;

/// Render ui to terminal
pub fn ui(frame: &mut Frame, app: &App) {
    let Some(ddm_frame) = app.get_frame() else {
        return;
    };
    let pixel_data = ddm_frame.pixel_data().mapv(|x| (x / 8) as u8);
    let area = frame.area();
    let widget = PixelWidget {
        pixel_data: &pixel_data,
    };
    frame.render_widget(widget, area);
}

struct PixelWidget<'a> {
    pixel_data: &'a Array2<u8>,
}

impl<'a> Widget for PixelWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (height, width) = self.pixel_data.dim();

        // Each terminal character cell represents 2 vertical pixels (top and bottom)
        for y_cell in 0..height.div_ceil(2) {
            for x_cell in 0..width {
                // Ensure we don't draw outside the allocated Ratatui widget area
                if x_cell >= area.width as usize || y_cell >= area.height as usize {
                    continue;
                }

                let y_top = y_cell * 2;
                let y_bot = y_top + 1;

                let top_val = self.pixel_data.get((y_top, x_cell)).copied().unwrap_or(0);
                let bot_val = if y_bot < height {
                    self.pixel_data.get((y_bot, x_cell)).copied().unwrap_or(0)
                } else {
                    0
                };

                let top_color = Color::Rgb(top_val, top_val, top_val);
                let bot_color = Color::Rgb(bot_val, bot_val, bot_val);

                let cell = buf.get_mut(area.x + x_cell as u16, area.y + y_cell as u16);
                cell.set_char('▀');
                cell.set_fg(top_color);
                cell.set_bg(bot_color);
            }
        }
    }
}
