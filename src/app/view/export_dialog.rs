use super::super::workspace::Workspace;
use crate::l0b_packet::{L0bPacket, mp00::MP00, mq00::MQ00};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Task, Theme,
    widget::{
        Column, button, checkbox, column, container, pick_list, row, slider, text, text_input,
    },
};
use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::{BufWriter, Write},
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum Format {
    L0b,
    Archive,
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::L0b => formatter.write_str("L0B data file"),
            Self::Archive => formatter.write_str("DDM frame archive"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum PacketFormat {
    MQ00,
    MP00,
}

impl fmt::Display for PacketFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MQ00 => "MQ00",
            Self::MP00 => "MP00",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app) enum ArchiveFormat {
    Cbor,
}

impl fmt::Display for ArchiveFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CBOR")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct TabChoice {
    index: usize,
    title: String,
    is_viewer: bool,
}

impl fmt::Display for TabChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = if self.is_viewer {
            "DDM viewer"
        } else {
            "DDM analyzer"
        };
        write!(formatter, "{} · {kind}", self.title)
    }
}

#[derive(Debug, Clone)]
pub(in crate::app) enum Message {
    Close,
    TabSelected(TabChoice),
    FormatSelected(Format),
    AllChannelsChanged(bool),
    ChannelsChanged(String),
    StartChanged(String),
    EndChanged(String),
    FromStartChanged(bool),
    FromEndChanged(bool),
    PacketFormatSelected(PacketFormat),
    LlQuantizationChanged(f32),
    DetailQuantizationChanged(f32),
    ArchiveFormatSelected(ArchiveFormat),
    CompressionChanged(bool),
    SaveRequested,
    PathSelected(Option<PathBuf>),
    ExportFinished(Result<usize, String>),
}

#[derive(Debug)]
pub(in crate::app) struct State {
    format: Format,
    selected_tab: Option<TabChoice>,
    viewer: ViewerOptions,
    l0b: L0bOptions,
    archive: ArchiveOptions,
}

#[derive(Debug)]
struct ViewerOptions {
    all_channels: bool,
    channels: String,
    start: String,
    end: String,
    from_start: bool,
    from_end: bool,
}

#[derive(Debug)]
struct L0bOptions {
    packet_format: PacketFormat,
    ll_quantization: u8,
    detail_quantization: u8,
}

#[derive(Debug)]
struct ArchiveOptions {
    archive_format: ArchiveFormat,
    compress_archive: bool,
}

impl State {
    pub(in crate::app) fn new(format: Format, workspace: &Workspace) -> Self {
        let selected_tab = workspace.active_index().and_then(|index| {
            workspace
                .export_tabs()
                .into_iter()
                .find(|(tab_index, _, _)| *tab_index == index)
                .map(|(_, title, is_viewer)| TabChoice {
                    index,
                    title,
                    is_viewer,
                })
        });
        let end = selected_tab
            .as_ref()
            .and_then(|tab| workspace.export_frames(tab.index))
            .and_then(|data| data.frames.iter().map(|(offset, _, _)| *offset).max())
            .unwrap_or(0)
            .to_string();
        Self {
            format,
            selected_tab,
            viewer: ViewerOptions {
                all_channels: true,
                channels: String::from("0-15"),
                start: String::from("0"),
                end,
                from_start: true,
                from_end: true,
            },
            l0b: L0bOptions {
                packet_format: PacketFormat::MQ00,
                ll_quantization: 0,
                detail_quantization: 0,
            },
            archive: ArchiveOptions {
                archive_format: ArchiveFormat::Cbor,
                compress_archive: true,
            },
        }
    }

    pub(in crate::app) fn update(
        &mut self,
        message: Message,
        workspace: &Workspace,
    ) -> Option<ExportAction> {
        match message {
            Message::Close => Some(Self::close()),
            Message::TabSelected(tab) => self.select_tab(tab, workspace),
            Message::FormatSelected(format) => self.select_format(format),
            Message::AllChannelsChanged(value) => self.set_all_channels(value),
            Message::ChannelsChanged(value) => self.set_channels(value),
            Message::StartChanged(value) => self.set_start(value),
            Message::EndChanged(value) => self.set_end(value),
            Message::FromStartChanged(value) => self.set_from_start(value),
            Message::FromEndChanged(value) => self.set_from_end(value),
            Message::PacketFormatSelected(format) => self.select_packet_format(format),
            Message::LlQuantizationChanged(level) => self.set_ll_quantization(level),
            Message::DetailQuantizationChanged(level) => self.set_detail_quantization(level),
            Message::ArchiveFormatSelected(format) => self.select_archive_format(format),
            Message::CompressionChanged(compress) => self.set_compress_archive(compress),
            Message::SaveRequested => Some(Self::save()),
            Message::PathSelected(path) => Some(Self::path_selected(path)),
            Message::ExportFinished(result) => Some(Self::export_finished(result)),
        }
    }

    fn close() -> ExportAction {
        ExportAction::Close
    }

    fn select_tab(&mut self, tab: TabChoice, workspace: &Workspace) -> Option<ExportAction> {
        self.selected_tab = Some(tab);
        self.reset_time_range(workspace);
        None
    }

    fn select_format(&mut self, format: Format) -> Option<ExportAction> {
        self.format = format;
        None
    }

    fn set_all_channels(&mut self, value: bool) -> Option<ExportAction> {
        self.viewer.all_channels = value;
        None
    }

    fn set_channels(&mut self, value: String) -> Option<ExportAction> {
        self.viewer.channels = value;
        None
    }

    fn set_start(&mut self, value: String) -> Option<ExportAction> {
        self.viewer.start = value;
        None
    }

    fn set_end(&mut self, value: String) -> Option<ExportAction> {
        self.viewer.end = value;
        None
    }

    fn set_from_start(&mut self, value: bool) -> Option<ExportAction> {
        self.viewer.from_start = value;
        None
    }

    fn set_from_end(&mut self, value: bool) -> Option<ExportAction> {
        self.viewer.from_end = value;
        None
    }

    fn select_packet_format(&mut self, format: PacketFormat) -> Option<ExportAction> {
        self.l0b.packet_format = format;
        None
    }

    fn set_ll_quantization(&mut self, level: f32) -> Option<ExportAction> {
        self.l0b.ll_quantization = quantization_level(level);
        None
    }

    fn set_detail_quantization(&mut self, level: f32) -> Option<ExportAction> {
        self.l0b.detail_quantization = quantization_level(level);
        None
    }

    fn select_archive_format(&mut self, format: ArchiveFormat) -> Option<ExportAction> {
        self.archive.archive_format = format;
        None
    }

    fn set_compress_archive(&mut self, compress: bool) -> Option<ExportAction> {
        self.archive.compress_archive = compress;
        None
    }

    fn reset_time_range(&mut self, workspace: &Workspace) {
        self.viewer.start = String::from("0");
        self.viewer.end = self
            .selected_tab
            .as_ref()
            .and_then(|tab| workspace.export_frames(tab.index))
            .and_then(|data| data.frames.iter().map(|(offset, _, _)| *offset).max())
            .unwrap_or(0)
            .to_string();
    }

    fn save() -> ExportAction {
        ExportAction::Save
    }

    fn path_selected(path: Option<PathBuf>) -> ExportAction {
        ExportAction::Finished(path)
    }

    fn export_finished(result: Result<usize, String>) -> ExportAction {
        ExportAction::ExportFinished(result)
    }
}

pub(in crate::app) enum ExportAction {
    Close,
    Save,
    Finished(Option<PathBuf>),
    ExportFinished(Result<usize, String>),
}

pub(in crate::app) fn write_export_task(
    state: &State,
    workspace: &Workspace,
    path: PathBuf,
) -> Task<Message> {
    let plan = build_plan(state, workspace);
    match plan {
        Ok(plan) => Task::perform(
            async move {
                tokio::task::spawn_blocking(move || plan.write(&path))
                    .await
                    .map_err(|error| error.to_string())?
            },
            Message::ExportFinished,
        ),
        Err(error) => Task::done(Message::ExportFinished(Err(error))),
    }
}

struct ExportPlan {
    format: Format,
    packet_format: PacketFormat,
    ll_quantization: u8,
    detail_quantization: u8,
    compress_archive: bool,
    frames: Vec<(usize, u8, MQ00)>,
    viewer: bool,
}

impl ExportPlan {
    fn default_filename(&self) -> String {
        let mut channels = self
            .frames
            .iter()
            .map(|(_, channel, _)| *channel)
            .collect::<Vec<_>>();
        channels.sort_unstable();
        channels.dedup();
        let channel_part = channels
            .iter()
            .map(u8::to_string)
            .collect::<Vec<_>>()
            .join("_");
        let extension = match self.format {
            Format::L0b => "l0b.ddm",
            Format::Archive if self.compress_archive => "cbor.zst",
            Format::Archive => "cbor",
        };
        if self.frames.len() == 1 {
            let (offset, channel, packet) = &self.frames[0];
            let timestamp = if self.viewer {
                *offset
            } else {
                usize::try_from(packet.timestamp_seconds()).unwrap_or(0)
            };
            format!("channel{channel}_time{timestamp}.{extension}")
        } else {
            let start = self
                .frames
                .iter()
                .map(|(offset, _, _)| *offset)
                .min()
                .unwrap_or(0);
            let end = self
                .frames
                .iter()
                .map(|(offset, _, _)| *offset)
                .max()
                .unwrap_or(start);
            format!("channel{channel_part}_time{start}_{end}.{extension}")
        }
    }

    fn write(self, path: &std::path::Path) -> Result<usize, String> {
        match self.format {
            Format::L0b => {
                let file = File::create(path).map_err(|error| error.to_string())?;
                let mut writer = BufWriter::new(file);
                let packets = self
                    .frames
                    .into_iter()
                    .map(|(_, _, mut packet)| match self.packet_format {
                        PacketFormat::MQ00 => {
                            packet
                                .set_ll_quantisation(self.ll_quantization)
                                .and_then(|()| {
                                    packet.set_detail_quantisation(self.detail_quantization)
                                })
                                .map_err(|error| error.to_string())?;
                            Ok(L0bPacket::MQ00(Box::new(packet)))
                        }
                        PacketFormat::MP00 => MP00::try_from(&packet)
                            .map(|packet| L0bPacket::MP00(Box::new(packet)))
                            .map_err(|error| error.to_string()),
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let packet_count = packets.len();
                crate::l0b_packet::encode_all(&packets, &mut writer)
                    .map_err(|error| error.to_string())?;
                writer.flush().map_err(|error| error.to_string())?;
                Ok(packet_count)
            }
            Format::Archive => {
                let archive = if self.viewer {
                    viewer_archive(&self.frames)
                } else {
                    let (_, _, packet) = self
                        .frames
                        .first()
                        .ok_or_else(|| String::from("there is no DDM data to export"))?;
                    pixels_value(packet)
                };
                let file = File::create(path).map_err(|error| error.to_string())?;
                if self.compress_archive {
                    let mut encoder =
                        zstd::stream::Encoder::new(file, 0).map_err(|error| error.to_string())?;
                    ciborium::ser::into_writer(&archive, &mut encoder)
                        .map_err(|error| error.to_string())?;
                    encoder.finish().map_err(|error| error.to_string())?;
                } else {
                    let mut writer = BufWriter::new(file);
                    ciborium::ser::into_writer(&archive, &mut writer)
                        .map_err(|error| error.to_string())?;
                    writer.flush().map_err(|error| error.to_string())?;
                }
                Ok(self.frames.len())
            }
        }
    }
}

fn build_plan(state: &State, workspace: &Workspace) -> Result<ExportPlan, String> {
    let selected = state
        .selected_tab
        .as_ref()
        .ok_or_else(|| String::from("select a tab to export"))?;
    let export_frames = workspace
        .export_frames(selected.index)
        .ok_or_else(|| String::from("the selected tab is no longer open"))?;
    let viewer = export_frames.viewer;
    let mut frames = export_frames.frames;
    if viewer != selected.is_viewer {
        return Err(String::from(
            "the selected tab changed type; reopen the export dialog",
        ));
    }
    if viewer {
        let last_offset = frames
            .iter()
            .map(|(offset, _, _)| *offset)
            .max()
            .unwrap_or(0);
        let start = if state.viewer.from_start {
            0
        } else {
            state
                .viewer
                .start
                .parse::<usize>()
                .map_err(|_| String::from("start offset must be a non-negative whole number"))?
        };
        let end = if state.viewer.from_end {
            last_offset
        } else {
            state
                .viewer
                .end
                .parse::<usize>()
                .map_err(|_| String::from("end offset must be a non-negative whole number"))?
        };
        if end < start {
            return Err(String::from(
                "end offset must be greater than or equal to start offset",
            ));
        }
        let channels = if state.viewer.all_channels {
            None
        } else {
            Some(parse_channels(&state.viewer.channels)?)
        };
        frames.retain(|(offset, channel, _)| {
            (start..=end).contains(offset)
                && channels
                    .as_ref()
                    .is_none_or(|channels| channels.contains(channel))
        });
    }
    if frames.is_empty() {
        return Err(String::from(
            "no DDM frames match the selected export options",
        ));
    }
    Ok(ExportPlan {
        format: state.format,
        packet_format: state.l0b.packet_format,
        ll_quantization: state.l0b.ll_quantization,
        detail_quantization: state.l0b.detail_quantization,
        compress_archive: state.archive.compress_archive,
        frames,
        viewer,
    })
}

fn parse_channels(input: &str) -> Result<HashSet<u8>, String> {
    let mut channels = HashSet::new();
    for item in input
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        if let Some((start, end)) = item.split_once('-') {
            let start = parse_channel(start)?;
            let end = parse_channel(end)?;
            if end < start {
                return Err(format!("channel range {item} ends before it starts"));
            }
            channels.extend(start..=end);
        } else {
            channels.insert(parse_channel(item)?);
        }
    }
    if channels.is_empty() {
        return Err(String::from(
            "enter one or more channels, or select all channels",
        ));
    }
    Ok(channels)
}

fn parse_channel(input: &str) -> Result<u8, String> {
    let channel = input
        .parse::<u8>()
        .map_err(|_| format!("invalid channel number: {input}"))?;
    if channel >= crate::DDM_CHANNEL_COUNT {
        return Err(format!(
            "channel number must be less than {}",
            crate::DDM_CHANNEL_COUNT
        ));
    }
    Ok(channel)
}

fn quantization_level(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the slider value is rounded and clamped to the inclusive range 0 through 15"
    )]
    {
        value.round().clamp(0.0, 15.0) as u8
    }
}

fn viewer_archive(frames: &[(usize, u8, MQ00)]) -> ciborium::Value {
    let last_offset = frames
        .iter()
        .map(|(offset, _, _)| *offset)
        .max()
        .unwrap_or(0);
    let frame_lookup = frames
        .iter()
        .map(|(offset, channel, packet)| ((*offset, usize::from(*channel)), packet))
        .collect::<HashMap<_, _>>();
    let channels = (0..usize::from(crate::DDM_CHANNEL_COUNT))
        .map(|channel| {
            ciborium::Value::Array(
                (0..=last_offset)
                    .map(|offset| {
                        frame_lookup
                            .get(&(offset, channel))
                            .map_or_else(|| ciborium::Value::Null, |packet| pixels_value(packet))
                    })
                    .collect(),
            )
        })
        .collect();
    ciborium::Value::Array(channels)
}

fn pixels_value(packet: &MQ00) -> ciborium::Value {
    ciborium::Value::Array(
        packet
            .raw_pixel_data()
            .rows()
            .into_iter()
            .map(|row| {
                ciborium::Value::Array(
                    row.iter()
                        .map(|pixel| ciborium::Value::Integer((*pixel).into()))
                        .collect(),
                )
            })
            .collect(),
    )
}

pub(super) fn view<'a>(workspace: &'a Workspace, state: &'a State) -> Element<'a, Message> {
    let tabs: Vec<_> = workspace
        .export_tabs()
        .into_iter()
        .map(|(index, title, is_viewer)| TabChoice {
            index,
            title,
            is_viewer,
        })
        .collect();
    let selected = state
        .selected_tab
        .as_ref()
        .and_then(|selected| tabs.iter().find(|tab| tab.index == selected.index).cloned());
    let viewer_selected = selected.as_ref().is_some_and(|tab| tab.is_viewer);
    let tab_options = tab_options(state, tabs, selected.clone(), viewer_selected);
    let format_options = format_options(state);
    let actions = row![
        button("Cancel").on_press(Message::Close),
        button("Export").on_press_maybe(selected.map(|_| Message::SaveRequested)),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let panel = container(
        column![
            row![
                text("Export").size(20),
                iced::widget::Space::new().width(Fill),
                button("×").on_press(Message::Close)
            ],
            row![tab_options, iced::widget::rule::vertical(1), format_options]
                .spacing(18)
                .align_y(Alignment::Start),
            row![iced::widget::Space::new().width(Fill), actions],
        ]
        .spacing(18),
    )
    .width(Length::Fixed(760.0))
    .padding(22)
    .style(panel_style);

    container(panel)
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .style(backdrop_style)
        .into()
}

fn tab_options(
    state: &State,
    tabs: Vec<TabChoice>,
    selected: Option<TabChoice>,
    viewer_selected: bool,
) -> Column<'_, Message> {
    column![
        text("Tab options").size(16),
        text("Export type"),
        pick_list(
            vec![Format::L0b, Format::Archive],
            Some(state.format),
            Message::FormatSelected,
        )
        .width(Fill),
        text("Tab to export"),
        pick_list(tabs, selected, Message::TabSelected).width(Fill),
        if viewer_selected {
            column![
                checkbox(state.viewer.all_channels)
                    .label("All channels")
                    .on_toggle(Message::AllChannelsChanged),
                text("Channels (examples: 1, 3-5, 8)").size(13),
                text_input("0-15", &state.viewer.channels)
                    .on_input_maybe(
                        (!state.viewer.all_channels).then_some(Message::ChannelsChanged)
                    )
                    .padding(7),
                text("Timestamp offset range (seconds from data start)").size(13),
                row![
                    text_input("Start", &state.viewer.start)
                        .on_input_maybe((!state.viewer.from_start).then_some(Message::StartChanged))
                        .padding(7),
                    text("to"),
                    text_input("End", &state.viewer.end)
                        .on_input_maybe((!state.viewer.from_end).then_some(Message::EndChanged))
                        .padding(7),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    checkbox(state.viewer.from_start)
                        .label("From start")
                        .on_toggle(Message::FromStartChanged),
                    checkbox(state.viewer.from_end)
                        .label("From end")
                        .on_toggle(Message::FromEndChanged),
                ]
                .spacing(16),
            ]
            .spacing(8)
        } else {
            column![text("No tab-specific options")]
        }
    ]
    .spacing(10)
    .width(Fill)
}

fn format_options(state: &State) -> Column<'_, Message> {
    match state.format {
        Format::L0b => column![
            text("Format options").size(16),
            text("Packet format"),
            pick_list(
                vec![PacketFormat::MQ00, PacketFormat::MP00],
                Some(state.l0b.packet_format),
                Message::PacketFormatSelected,
            )
            .width(Fill),
            if state.l0b.packet_format == PacketFormat::MQ00 {
                mq00_quantization_options(state)
            } else {
                column![]
            }
        ]
        .spacing(10)
        .width(Fill),
        Format::Archive => column![
            text("Format options").size(16),
            text("Archive format"),
            pick_list(
                vec![ArchiveFormat::Cbor],
                Some(state.archive.archive_format),
                Message::ArchiveFormatSelected,
            )
            .width(Fill),
            checkbox(state.archive.compress_archive)
                .label("Compress with zstd")
                .on_toggle(Message::CompressionChanged),
        ]
        .spacing(10)
        .width(Fill),
    }
}

fn mq00_quantization_options(state: &State) -> Column<'_, Message> {
    column![
        text(format!(
            "Detail quantization: {}",
            state.l0b.detail_quantization
        )),
        slider(
            0.0..=15.0,
            f32::from(state.l0b.detail_quantization),
            Message::DetailQuantizationChanged,
        )
        .step(1.0_f32),
        text(format!("LL quantization: {}", state.l0b.ll_quantization)),
        slider(
            0.0..=15.0,
            f32::from(state.l0b.ll_quantization),
            Message::LlQuantizationChanged,
        )
        .step(1.0_f32),
    ]
    .spacing(8)
}

pub(in crate::app) fn save_picker(state: &State, workspace: &Workspace) -> Task<Message> {
    let plan = match build_plan(state, workspace) {
        Ok(plan) => plan,
        Err(error) => return Task::done(Message::ExportFinished(Err(error))),
    };
    let picker = rfd::AsyncFileDialog::new()
        .set_title("Export data")
        .set_file_name(plan.default_filename());
    let picker = match state.format {
        Format::L0b => picker.add_filter("L0B data", &["ddm"]),
        Format::Archive if state.archive.compress_archive => {
            picker.add_filter("Compressed CBOR archive", &["cbor", "zst"])
        }
        Format::Archive => picker.add_filter("CBOR archive", &["cbor"]),
    };
    Task::perform(picker.save_file(), |file| {
        Message::PathSelected(file.map(|file| file.path().to_path_buf()))
    })
}

fn panel_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(38, 41, 47))),
        text_color: Some(theme.palette().text),
        border: Border::default()
            .rounded(8)
            .width(1)
            .color(Color::from_rgb8(80, 84, 92)),
        ..container::Style::default()
    }
}

fn backdrop_style(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.75))),
        text_color: Some(theme.palette().text),
        ..container::Style::default()
    }
}
