use eframe::egui;
use egui_plot::{Heatmap, Plot};

use crate::ddm_frame::DdmFrame;

pub struct DdmViewer {
    frames: Vec<Vec<DdmFrame>>,
    outer_index: usize,
    inner_index: usize,

    playing: bool,
    fps: f32,
    last_frame_time: std::time::Instant,
}

impl DdmViewer {
    pub fn new(frames: Vec<Vec<DdmFrame>>) -> Self {
        Self {
            frames,
            outer_index: 0,
            inner_index: 0,
            playing: false,
            fps: 5.0,
            last_frame_time: std::time::Instant::now(),
        }
    }

    fn current_frame(&self) -> Option<&DdmFrame> {
        self.frames
            .get(self.outer_index)
            .and_then(|frames| frames.get(self.inner_index))
    }
}

impl eframe::App for DdmViewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // ─────────────────────────────────────────────
        // Playback
        // ─────────────────────────────────────────────

        if self.playing {
            let frame_duration = std::time::Duration::from_secs_f32(1.0 / self.fps);

            if self.last_frame_time.elapsed() >= frame_duration {
                if let Some(frames) = self.frames.get(self.outer_index)
                    && !frames.is_empty() {
                        self.inner_index = (self.inner_index + 1) % frames.len();
                    }

                self.last_frame_time = std::time::Instant::now();
            }

            // Keep repainting while playing.
            ui.ctx().request_repaint();
        }

        ui.columns(2, |columns| {
            // ─────────────────────────────────────────────
            // Left column
            // ─────────────────────────────────────────────

            columns[0].vertical(|ui| {
                ui.heading("DDM Frame");
                ui.separator();

                if self.frames.is_empty() {
                    ui.label("No frames");
                    return;
                }

                // Outer index
                ui.add(
                    egui::Slider::new(&mut self.outer_index, 0..=self.frames.len() - 1)
                        .text("Channel"),
                );

                let inner_len = self.frames[self.outer_index].len();

                if inner_len == 0 {
                    self.inner_index = 0;
                    ui.label("No frames");
                    return;
                }

                self.inner_index = self.inner_index.min(inner_len - 1);

                // Inner index
                ui.add(egui::Slider::new(&mut self.inner_index, 0..=inner_len - 1).text("Frame"));

                ui.separator();

                // ─────────────────────────────────────
                // Playback controls
                // ─────────────────────────────────────

                ui.horizontal(|ui| {
                    let button_text = if self.playing {
                        "⏸ Pause"
                    } else {
                        "▶ Play"
                    };

                    if ui.button(button_text).clicked() {
                        self.playing = !self.playing;
                        self.last_frame_time = std::time::Instant::now();
                    }

                    if ui.button("⏮").clicked() {
                        self.inner_index = 0;
                        self.last_frame_time = std::time::Instant::now();
                    }
                });

                ui.add(
                    egui::Slider::new(&mut self.fps, 0.1..=30.0)
                        .text("FPS")
                        .logarithmic(true),
                );

                ui.label(format!("{:.1} frames/sec", self.fps));

                ui.separator();

                // ─────────────────────────────────────
                // Metadata
                // ─────────────────────────────────────

                if let Some(frame) = self.current_frame() {
                    ui.label(format!("Channel: {}", frame.channel_num));

                    ui.label(format!("Timestamp seconds: {}", frame.timestamp_seconds));

                    ui.label(format!("Timestamp clocks: {}", frame.timestamp_clocks));

                    ui.separator();

                    let mut min = u16::MAX;
                    let mut max = u16::MIN;

                    for row in &frame.pixel_data {
                        for &value in row {
                            min = min.min(value);
                            max = max.max(value);
                        }
                    }

                    ui.label(format!("Pixel min: {min}"));
                    ui.label(format!("Pixel max: {max}"));
                }
            });

            // ─────────────────────────────────────────────
            // Right column: heatmap
            // ─────────────────────────────────────────────

            columns[1].vertical(|ui| {
                let Some(frame) = self.current_frame() else {
                    ui.centered_and_justified(|ui| {
                        ui.label("No frame selected");
                    });
                    return;
                };

                ui.heading(format!(
                    "Channel {} — [{}, {}]",
                    frame.channel_num, self.outer_index, self.inner_index,
                ));

                // Reverse the rows so row 0 appears at the
                // bottom of the heatmap rather than the top.
                let values: Vec<f64> = frame
                    .pixel_data
                    .iter()
                    .rev()
                    .flat_map(|row| row.iter())
                    .map(|&v| v as f64)
                    .collect();

                let min = values.iter().copied().fold(f64::INFINITY, f64::min);

                let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

                Plot::new("pixel_heatmap")
                    .data_aspect(1.0)
                    .show_axes([true, true])
                    .show(ui, |plot_ui| {
                        plot_ui.heatmap(
                            Heatmap::new(values, 20)
                                .range(min, max)
                                .show_labels(false)
                                .name("Pixel values")
                                .size(40.0, 52.0),
                        );
                    });
            });
        });
    }
}
