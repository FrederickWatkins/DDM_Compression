//! Functions for encoding and decoding DDM frames
use ddm_core::{
    error::Error,
    l0b::{
        DDMDecoder as _, DDMEncoder, DDMFrame, L0bPacket, MP00Decoder, MP00Encoder,
        MP00EncoderConfig, MQ00Decoder, MQ00Encoder, MQ00EncoderConfig,
    },
};
use either::Either;
use itertools::Itertools;
use prodash::unit::{self, Human, display::Mode};
use tracing::warn;

use crate::{L0bFormat, L0bFormatConfig};

/// Decode DDM frame data from packets
pub fn decode<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: Iterator<Item = L0bPacket<WIDTH, HEIGHT>>,
>(
    packets: I,
    progress: &mut dyn prodash::Progress,
) -> impl Iterator<Item = Result<DDMFrame<WIDTH, HEIGHT>, Error>> {
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    progress.init(
        packets.try_len().ok(),
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "packets"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    let mut mp00_decoder = MP00Decoder::new();
    let mut mq00_decoder = MQ00Decoder::new();
    let mut prev_format = None;
    let mut warning_emitted = false;
    packets.filter_map(move |packet| {
        progress.inc();
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
    progress: &mut dyn prodash::Progress,
) -> Result<impl Iterator<Item = Result<L0bPacket<WIDTH, HEIGHT>, Error>>, Error> {
    let mut formatter = human_format::Formatter::new();
    formatter.with_decimals(2);
    progress.init(
        frames.try_len().ok(),
        Some(unit::dynamic_and_mode(
            Human::new(formatter, "frames"),
            Mode::with_throughput().and_percentage(),
        )),
    );
    match format {
        L0bFormatConfig::MP00 { quantisation } => {
            let config = MP00EncoderConfig::new(quantisation);
            let mut encoder = MP00Encoder::new(config);
            Ok(Either::Left(frames.map(move |frame| {
                progress.inc();
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
                progress.inc();
                Ok(L0bPacket::MQ00(encoder.encode(frame)?))
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use ddm_core::l0b::DDMFrame;
    use prodash::progress::Discard;
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #![proptest_config(ProptestConfig { cases: 16, .. ProptestConfig::default() })]

        #[test]
        fn mp00_round_trip(frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..8)) {
            let mut progress = Discard;
            let packets = encode(
                frames.iter().cloned(),
                L0bFormatConfig::MP00 { quantisation: 0 },
                &mut progress,
            )?
            .collect::<Result<Vec<_>, _>>()?;
            let decoded_frames = decode(packets.into_iter(), &mut progress)
                .collect::<Result<Vec<_>, _>>()?;

            prop_assert_eq!(frames, decoded_frames);
        }

        #[test]
        fn mq00_round_trip(
            frames in proptest::collection::vec(any::<DDMFrame<20, 52>>(), 1..8),
            wavelet_stages in 0..16u8,
            gop_length in 1..=32u8
        ) {
            let mut progress = Discard;
            let packets = encode(
                frames.iter().cloned(),
                L0bFormatConfig::MQ00 {
                    quantisation: 0,
                    dc_adjust: false,
                    wavelet_stages,
                    gop_length,
                },
                &mut progress,
            )?
            .collect::<Result<Vec<_>, _>>()?;
            let decoded_frames = decode(packets.into_iter(), &mut progress)
                .collect::<Result<Vec<_>, _>>()?;

            prop_assert_eq!(frames, decoded_frames);
        }
    }
}
