use std::io::{Read, Seek, Write};

use binrw::{
    BinRead, BinWrite,
    meta::{ReadEndian, WriteEndian},
};
use ddm_core::l0b::{DDMDecoder as _, DDMEncoder, DDMFrame, L0bPacket, MP00Decoder, MQ00Decoder};
use prodash::Progress;
use tracing::{debug, error, warn};

use crate::{
    l0b::L0bFormat,
    pipeline::{PipelineInput, PipelineInputIter, PipelineOutput},
};

/// Read packets until EOF
///
/// # Errors
/// Passes on any errors from reader and outputs, or if format doesn't match
#[tracing::instrument(skip_all, level = "trace")]
pub fn read<T: BinRead + ReadEndian, R: Read + Seek, O: PipelineOutput<Item = T>>(
    reader: &mut R,
    output: &mut O,
    progress: &mut dyn Progress,
) -> eyre::Result<()>
where
    for<'a> <T as BinRead>::Args<'a>: Default,
{
    debug!(
        thread_id = ?thread_id::get(),
        "Started reading packets"
    );
    let mut total_packets = 0_usize;
    loop {
        let start_pos = reader.stream_position()?;
        match T::read(reader) {
            Ok(packet) => output.send(packet)?,
            Err(e) if e.is_eof() && reader.stream_position()? == start_pos => {
                debug!(
                    thread_id = ?thread_id::get(),
                    "Finished reading {total_packets} packets"
                );
                return Ok(());
            }
            Err(e) => {
                error!(
                    thread_id = ?thread_id::get(),
                    "Error during read: {e}"
                );
                return Err(e.into());
            }
        }
        if let Ok(position) = reader.stream_position() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "Loading bar truncation doesn't matter"
            )]
            progress.set(position.min(usize::MAX as u64) as usize);
        }
        total_packets = total_packets.saturating_add(1);
    }
}

/// Write packets until EOF
///
/// # Errors
/// Passes on any errors from writer
#[tracing::instrument(skip_all, level = "trace")]
pub fn write<T: BinWrite + WriteEndian, W: Write + Seek, I: PipelineInput<Item = T>>(
    writer: &mut W,
    input: &mut I,
    progress: &mut dyn Progress,
) -> eyre::Result<()>
where
    for<'a> <T as BinWrite>::Args<'a>: Default,
{
    debug!(
        thread_id = ?thread_id::get(),
        "Started writing packets"
    );
    let mut total_packets = 0_usize;
    let input = PipelineInputIter::new(input);
    for packet in input {
        progress.inc();
        total_packets = total_packets.saturating_add(1);
        packet.write(writer)?;
    }
    debug!(
        thread_id = ?thread_id::get(),
        "Finished writing {total_packets} packets"
    );
    Ok(())
}

/// Decode packets into frames
///
/// # Errors
/// Passes on any errors from decoders or pipeline output
#[tracing::instrument(skip_all, level = "trace")]
pub fn decode<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: PipelineInput<Item = L0bPacket<WIDTH, HEIGHT>>,
    O: PipelineOutput<Item = DDMFrame<WIDTH, HEIGHT>>,
>(
    input: &mut I,
    output: &mut O,
    progress: &mut dyn Progress,
) -> eyre::Result<()> {
    debug!(
        thread_id = ?thread_id::get(),
        "Started decoding frames"
    );
    let mut total_packets = 0_usize;
    let mut total_frames = 0_usize;
    let input = PipelineInputIter::new(input);
    let mut mp00_decoder = MP00Decoder::new();
    let mut mq00_decoder = MQ00Decoder::new();
    let mut prev_format = None;
    let mut warning_emitted = false;
    for packet in input {
        total_packets = total_packets.saturating_add(1);
        progress.inc();
        let (current_format, frame) = match packet {
            L0bPacket::MP00(mp00_packet) => (L0bFormat::MP00, mp00_decoder.decode(mp00_packet)?),
            L0bPacket::MQ00(mq00_packet) => (L0bFormat::MQ00, mq00_decoder.decode(mq00_packet)?),
            L0bPacket::MT00(..) => continue,
        };
        total_frames = total_frames.saturating_add(1);
        if let Some(prev_format) = prev_format
            && current_format != prev_format
            && !warning_emitted
        {
            warn!(
                "Mixed packet types detected in packet stream: previous packet {prev_format}, current packet {current_format}"
            );
            warning_emitted = true;
        }
        prev_format = Some(current_format);
        output.send(frame)?;
    }
    debug!(
        thread_id = ?thread_id::get(),
        "Finished decoding {total_frames} frames from {total_packets} packets"
    );
    Ok(())
}

/// Encode frames into packets.
///
/// # Errors
/// Passes on any errors from encoder or pipeline output
#[tracing::instrument(skip_all, level = "trace")]
pub fn encode<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: PipelineInput<Item = DDMFrame<WIDTH, HEIGHT>>,
    O: PipelineOutput<Item = L0bPacket<WIDTH, HEIGHT>>,
    E: DDMEncoder<WIDTH, HEIGHT>,
>(
    input: &mut I,
    output: &mut O,
    encoder: &mut E,
    progress: &mut dyn Progress,
) -> eyre::Result<()>
where
    E::Packet: Into<L0bPacket<WIDTH, HEIGHT>>,
{
    debug!(
        thread_id = ?thread_id::get(),
        "Started encoding frames"
    );
    let mut total_frames = 0_usize;
    let input = PipelineInputIter::new(input);
    for frame in input {
        total_frames = total_frames.saturating_add(1);
        progress.inc();
        let packet = encoder.encode(frame)?;
        output.send(packet.into())?;
    }
    debug!(
        thread_id = ?thread_id::get(),
        "Finished encoding {total_frames} frames"
    );
    Ok(())
}
