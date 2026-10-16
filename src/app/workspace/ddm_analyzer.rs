//! Static inspection view for a selected DDM.

use crate::l0b_packet::{Decode, Encode, mp00::MP00, mq00::MQ00};
use iced::{
    Alignment, Element, Length,
    widget::{Column, Container, button, column, container, row, scrollable, slider, text},
};
use std::{io::Cursor, mem::size_of};

use super::heatmap;

#[derive(Debug, Clone, Copy)]
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
    wavelet_coded: Option<MQ00>,
    encoded_size: Option<usize>,
}

impl QuantisationConfiguration {
    fn new(packet: &MQ00, ll: u8, detail: u8) -> Self {
        let mut configuration = Self {
            ll,
            detail,
            decoded: None,
            wavelet_coded: None,
            encoded_size: None,
        };
        configuration.recalculate(packet);
        configuration
    }

    fn set_ll(&mut self, packet: &MQ00, level: f32) {
        self.ll = quantisation_level(level);
        self.recalculate(packet);
    }

    fn set_detail(&mut self, packet: &MQ00, level: f32) {
        self.detail = quantisation_level(level);
        self.recalculate(packet);
    }

    fn recalculate(&mut self, packet: &MQ00) {
        (self.decoded, self.encoded_size) = quantised_round_trip(packet, self.ll, self.detail);
        self.wavelet_coded = self.decoded.as_ref().map(wavelet_coded);
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
        let wavelet_coded_packet = wavelet_coded(&packet);
        let mp00_encoded_size = MP00::try_from(&packet)
            .ok()
            .and_then(|mp00| encode_size(&mp00));
        let configurations = [
            QuantisationConfiguration::new(&packet, 0, 3),
            QuantisationConfiguration::new(&packet, 2, 5),
        ]
        .into();

        Self {
            packet,
            wavelet_coded_packet,
            configurations,
            mp00_encoded_size,
        }
    }

    pub(super) fn update(&mut self, message: Message) {
        match message {
            Message::AddConfiguration => self.add_configuration(),
            Message::RemoveConfiguration(index) => self.remove_configuration(index),
            Message::LlChanged(index, level) => {
                self.update_configuration(index, |config, packet| {
                    config.set_ll(packet, level);
                });
            }
            Message::DetailChanged(index, level) => {
                self.update_configuration(index, |config, packet| {
                    config.set_detail(packet, level);
                });
            }
        }
    }

    fn add_configuration(&mut self) {
        self.configurations
            .push(QuantisationConfiguration::new(&self.packet, 0, 0));
    }

    fn remove_configuration(&mut self, index: usize) {
        if index < self.configurations.len() {
            self.configurations.remove(index);
        }
    }

    fn update_configuration(
        &mut self,
        index: usize,
        update: impl FnOnce(&mut QuantisationConfiguration, &MQ00),
    ) {
        if let Some(configuration) = self.configurations.get_mut(index) {
            update(configuration, &self.packet);
        }
    }

    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let mut controls = self.summary_controls();
        for (index, configuration) in self.configurations.iter().enumerate() {
            controls = controls.push(self.configuration_controls(index, configuration));
        }

        container(scrollable(controls).height(Length::Fill))
            .width(240.0)
            .height(Length::Fill)
            .padding(12)
            .into()
    }

    fn summary_controls(&self) -> Column<'_, Message> {
        column![
            text("Compression performance"),
            text(format!("Raw MP00: {} bytes", size_of::<MP00>())),
            text(size_text("Encoded MP00", self.mp00_encoded_size)),
            button("Add quantisation configuration").on_press(Message::AddConfiguration),
        ]
        .spacing(10)
    }

    fn configuration_controls(
        &self,
        index: usize,
        configuration: &QuantisationConfiguration,
    ) -> Container<'_, Message> {
        let content = column![
            text(format!("Configuration {}", index + 1)),
            text(format!("LL quantisation: {}", configuration.ll)),
            slider(0.0..=15.0, f32::from(configuration.ll), move |level| {
                Message::LlChanged(index, level)
            })
            .step(1.0_f32)
            .style(super::super::style::slider_style),
            text(format!("Detail quantisation: {}", configuration.detail)),
            slider(0.0..=15.0, f32::from(configuration.detail), move |level| {
                Message::DetailChanged(index, level)
            })
            .step(1.0_f32)
            .style(super::super::style::slider_style),
            text(self.compression_summary(configuration.encoded_size)),
            button("Remove").on_press(Message::RemoveConfiguration(index)),
        ]
        .spacing(6);

        container(content).padding(8)
    }

    fn compression_summary(&self, encoded_size: Option<usize>) -> String {
        let Some(size) = encoded_size else {
            return "Encoded MQ00: unavailable".to_owned();
        };

        let mp00_ratio = self.mp00_encoded_size.map_or_else(
            || "unavailable".to_owned(),
            |mp00_size| ratio_text(mp00_size, size),
        );
        format!(
            "Encoded MQ00: {size} bytes\n{} vs raw MP00\n{mp00_ratio} vs encoded MP00",
            ratio_text(size_of::<MP00>(), size),
        )
    }

    pub(super) fn view(&self) -> Element<'_, Message> {
        let panels = std::iter::once(self.original_panel())
            .chain(self.configurations.iter().map(Self::configuration_panel))
            .collect::<Vec<_>>();

        container(
            scrollable(row(panels).spacing(16))
                .direction(scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default(),
                ))
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
        .into()
    }

    fn original_panel(&self) -> Element<'_, Message> {
        packet_panel("Original", &self.packet, &self.wavelet_coded_packet)
    }

    fn configuration_panel(configuration: &QuantisationConfiguration) -> Element<'_, Message> {
        let title = format!(
            "Quantised ({}, {}) round trip",
            configuration.ll, configuration.detail
        );
        let decoded_view: Element<'static, Message> = configuration.decoded.as_ref().map_or_else(
            || text("Unavailable").into(),
            |packet| heatmap::view(&packet.raw_pixel_data(), 10.0, 5.0),
        );
        let wavelet_view: Element<'static, Message> =
            configuration.wavelet_coded.as_ref().map_or_else(
                || text("Unavailable").into(),
                |packet| heatmap::view(&packet.raw_pixel_data(), 10.0, 5.0),
            );

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

fn packet_panel<'a>(
    title: &'a str,
    packet: &'a MQ00,
    wavelet_packet: &'a MQ00,
) -> Element<'a, Message> {
    column![
        text(title),
        heatmap::view(&packet.raw_pixel_data(), 10.0, 5.0),
        text("Wavelet coded"),
        heatmap::view(&wavelet_packet.raw_pixel_data(), 10.0, 5.0),
    ]
    .spacing(6)
    .into()
}

fn wavelet_coded(packet: &MQ00) -> MQ00 {
    let mut wavelet = packet.clone();
    wavelet.dwt();
    wavelet
}

fn quantisation_level(level: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is rounded and clamped to the inclusive u8 range 0 through 15"
    )]
    {
        level.round().clamp(0.0, 15.0) as u8
    }
}

fn ratio_text(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        return "unavailable".to_owned();
    }

    let numerator = u128::try_from(numerator).unwrap_or(u128::MAX);
    let denominator = u128::try_from(denominator).unwrap_or(u128::MAX);
    let rounded_hundredths = (numerator * 100 + denominator / 2) / denominator;
    format!(
        "{}.{:02}×",
        rounded_hundredths / 100,
        rounded_hundredths % 100
    )
}

fn size_text(label: &str, size: Option<usize>) -> String {
    size.map_or_else(
        || format!("{label}: unavailable"),
        |size| format!("{label}: {size} bytes"),
    )
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
