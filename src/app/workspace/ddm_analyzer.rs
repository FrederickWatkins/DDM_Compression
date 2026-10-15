//! Static inspection view for a selected DDM.

use crate::l0b_packet::{Decode, Encode, mp00::MP00, mq00::MQ00};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, scrollable, slider, text},
};
use std::{io::Cursor, mem::size_of};

use super::heatmap;

#[derive(Debug, Clone)]
pub(in crate::app) enum Message {
    AddConfiguration,
    RemoveConfiguration(usize),
    LlChanged(usize, f32),
    DetailChanged(usize, f32),
}

#[derive(Debug)]
struct QuantisationConfiguration {
    ll: u8,
    detail: u8,
    decoded: Option<MQ00>,
    encoded_size: Option<usize>,
}

impl QuantisationConfiguration {
    fn new(packet: &MQ00, ll: u8, detail: u8) -> Self {
        let (decoded, encoded_size) = quantised_round_trip(packet, ll, detail);
        Self { ll, detail, decoded, encoded_size }
    }
}

#[derive(Debug)]
pub(super) struct DdmAnalyzer {
    packet: MQ00,
    wavelet_coded_packet: MQ00,
    configurations: Vec<QuantisationConfiguration>,
    mp00_encoded_size: Option<usize>,
}

impl DdmAnalyzer {
    pub(super) fn new(packet: MQ00) -> Self {
        let mut wavelet_coded_packet = packet.clone();
        wavelet_coded_packet.dwt();
        let mp00_encoded_size = MP00::try_from(packet.clone())
            .ok()
            .and_then(|mp00| encode_size(&mp00));
        Self {
            configurations: vec![
                QuantisationConfiguration::new(&packet, 0, 3),
                QuantisationConfiguration::new(&packet, 2, 5),
            ],
            packet,
            wavelet_coded_packet,
            mp00_encoded_size,
        }
    }

    pub(super) fn update(&mut self, message: Message) {
        match message {
            Message::AddConfiguration => {
                self.configurations
                    .push(QuantisationConfiguration::new(&self.packet, 0, 0));
            }
            Message::RemoveConfiguration(index) => {
                if index < self.configurations.len() {
                    self.configurations.remove(index);
                }
            }
            Message::LlChanged(index, level) => {
                if let Some(configuration) = self.configurations.get_mut(index) {
                    configuration.ll = quantisation_level(level);
                    *configuration = QuantisationConfiguration::new(
                        &self.packet,
                        configuration.ll,
                        configuration.detail,
                    );
                }
            }
            Message::DetailChanged(index, level) => {
                if let Some(configuration) = self.configurations.get_mut(index) {
                    configuration.detail = quantisation_level(level);
                    *configuration = QuantisationConfiguration::new(
                        &self.packet,
                        configuration.ll,
                        configuration.detail,
                    );
                }
            }
        }
    }

    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let mut controls = column![
            text("Compression performance"),
            text(format!("Raw MP00: {} bytes", size_of::<MP00>())),
            text(size_text("Encoded MP00", self.mp00_encoded_size)),
            button("Add quantisation configuration").on_press(Message::AddConfiguration),
        ]
        .spacing(10);

        for (index, configuration) in self.configurations.iter().enumerate() {
            let compression = configuration.encoded_size.map_or_else(
                || "Encoded MQ00: unavailable".to_owned(),
                |size| {
                    let mp00_ratio = self.mp00_encoded_size.map_or_else(
                        || "unavailable".to_owned(),
                        |mp00_size| format!("{:.2}×", mp00_size as f32 / size as f32),
                    );
                    format!(
                        "Encoded MQ00: {size} bytes\n{:.2}× vs raw MP00\n{mp00_ratio} vs encoded MP00",
                        size_of::<MP00>() as f32 / size as f32,
                    )
                },
            );
            controls = controls.push(
                container(
                    column![
                        text(format!("Configuration {}", index + 1)),
                        text(format!("LL quantisation: {}", configuration.ll)),
                        slider(0.0..=15.0, f32::from(configuration.ll), move |level| {
                            Message::LlChanged(index, level)
                        })
                        .step(1.0_f32),
                        text(format!("Detail quantisation: {}", configuration.detail)),
                        slider(0.0..=15.0, f32::from(configuration.detail), move |level| {
                            Message::DetailChanged(index, level)
                        })
                        .step(1.0_f32),
                        text(compression),
                        button("Remove").on_press(Message::RemoveConfiguration(index)),
                    ]
                    .spacing(6),
                )
                .padding(8),
            );
        }

        container(scrollable(controls).height(Length::Fill))
            .width(240.0)
            .height(Length::Fill)
            .padding(12)
            .into()
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        let mut panels = vec![self.packet_panel(
            "Original",
            &self.packet,
            &self.wavelet_coded_packet,
        )];

        for configuration in &self.configurations {
            panels.push(self.configuration_panel(configuration));
        }

        container(
            scrollable(row(panels).spacing(16))
                .direction(scrollable::Direction::Horizontal(scrollable::Scrollbar::default()))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .into()
    }

    fn packet_panel<'a>(
        &self,
        title: &'a str,
        packet: &'a MQ00,
        wavelet_packet: &'a MQ00,
    ) -> Element<'a, Message> {
        column![
            text(title),
            heatmap::view(packet.raw_pixel_data(), 10.0, 5.0),
            text("Wavelet coded"),
            heatmap::view(wavelet_packet.raw_pixel_data(), 10.0, 5.0),
        ]
        .spacing(6)
        .into()
    }

    fn configuration_panel<'a>(
        &'a self,
        configuration: &'a QuantisationConfiguration,
    ) -> Element<'a, Message> {
        let title = format!("Quantised ({}, {}) round trip", configuration.ll, configuration.detail);
        let wavelet_packet = configuration.decoded.as_ref().map(|packet| {
            let mut wavelet = packet.clone();
            wavelet.dwt();
            wavelet
        });
        let wavelet_view: Element<'a, Message> = match &wavelet_packet {
            Some(packet) => heatmap::view(packet.raw_pixel_data(), 10.0, 5.0),
            None => text("Unavailable").into(),
        };
        let decoded_view: Element<'a, Message> = match &configuration.decoded {
            Some(packet) => heatmap::view(packet.raw_pixel_data(), 10.0, 5.0),
            None => text("Unavailable").into(),
        };
        column![
            text(title),
            decoded_view,
            text("Wavelet coded"),
            wavelet_view,
        ]
        .spacing(6)
        .into()
    }
}

fn quantisation_level(level: f32) -> u8 {
    level.round().clamp(0.0, 15.0) as u8
}

fn size_text(label: &str, size: Option<usize>) -> String {
    match size {
        Some(size) => format!("{label}: {size} bytes"),
        None => format!("{label}: unavailable"),
    }
}

fn encode_size<T: Encode>(packet: &T) -> Option<usize> {
    let mut cursor = Cursor::new(Vec::new());
    packet.encode(&mut cursor).ok()?;
    Some(cursor.into_inner().len())
}

fn quantised_round_trip(packet: &MQ00, ll: u8, detail: u8) -> (Option<MQ00>, Option<usize>) {
    let mut quantised = packet.clone();
    if quantised.set_ll_quantisation(ll).is_err()
        || quantised.set_detail_quantisation(detail).is_err()
    {
        return (None, None);
    }
    let mut cursor = Cursor::new(Vec::new());
    if quantised.encode(&mut cursor).is_err() {
        return (None, None);
    }
    let encoded = cursor.into_inner();
    let size = encoded.len();
    let decoded = MQ00::decode(&mut Cursor::new(encoded)).ok();
    (decoded, Some(size))
}
