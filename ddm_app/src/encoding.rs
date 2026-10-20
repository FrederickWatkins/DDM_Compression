//! Functions for encoding and decoding DDM frames
use ddm_core::{
    error::Error,
    l0b::{
        DDMDecoder as _, DDMEncoder, DDMFrame, L0bPacket, MP00Decoder, MP00Encoder,
        MP00EncoderConfig, MQ00Decoder, MQ00Encoder, MQ00EncoderConfig,
    },
};
use either::Either;
use tracing::warn;

use crate::{L0bFormat, L0bFormatConfig};

/// Decode DDM frame data from packets
pub fn decode<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: Iterator<Item = L0bPacket<WIDTH, HEIGHT>>,
>(
    packets: I,
) -> impl Iterator<Item = Result<DDMFrame<WIDTH, HEIGHT>, Error>> {
    let mut mp00_decoder = MP00Decoder::new();
    let mut mq00_decoder = MQ00Decoder::new();
    let mut prev_format = None;
    let mut warning_emitted = false;
    packets.filter_map(move |packet| {
        let (current_format, output) = match packet {
            L0bPacket::MP00(mp00_packet) => (
                L0bFormat::MP00,
                Some(mp00_decoder.decode(mp00_packet).map_err(Into::into)),
            ),
            L0bPacket::MQ00(mq00_packet) => (
                L0bFormat::MQ00,
                Some(mq00_decoder.decode(mq00_packet).map_err(Into::into)),
            ),
            L0bPacket::MT00(..) => return None,
        };
        if let Some(prev_format) = prev_format && current_format != prev_format && !warning_emitted {
            warn!("Mixed packet types detected in packet stream: previous packet {prev_format}, current packet {current_format}");
            warning_emitted = true;
        }
        prev_format = Some(current_format);
        output
    })
}

/// Encode DDM frame data to packets
///
/// # Errors
/// Passes up errors from the underlying encoder
pub fn encode<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: Iterator<Item = DDMFrame<WIDTH, HEIGHT>>,
>(
    frames: I,
    format: L0bFormatConfig,
) -> Result<impl Iterator<Item = Result<L0bPacket<WIDTH, HEIGHT>, Error>>, Error> {
    match format {
        L0bFormatConfig::MP00 { quantisation } => {
            let config = MP00EncoderConfig::new(quantisation);
            let mut encoder = MP00Encoder::new(config);
            Ok(Either::Left(frames.map(move |frame| {
                Ok(L0bPacket::MP00(encoder.encode(frame)?))
            })))
        }
        L0bFormatConfig::MQ00 {
            quantisation,
            dc_adjust,
            wavelet_stages,
            gop_length,
        } => {
            let config =
                MQ00EncoderConfig::new(quantisation, dc_adjust, wavelet_stages, gop_length)?;
            let mut encoder = MQ00Encoder::new(config);
            Ok(Either::Right(frames.map(move |frame| {
                Ok(L0bPacket::MQ00(encoder.encode(frame)?))
            })))
        }
    }
}
