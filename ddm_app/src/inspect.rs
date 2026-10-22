use std::collections::HashSet;

use ddm_core::l0b::{DDMFrameMetadata, L0bPacket};
use prodash::Progress;
use tracing::debug;

use crate::{
    l0b::{GpsSecondRange, L0bFileInfo, L0bFormat},
    pipeline::{PipelineInput, PipelineInputIter},
};

/// Inspect information about L0b file
#[tracing::instrument(skip_all, level = "trace")]
pub fn inspect<
    const WIDTH: usize,
    const HEIGHT: usize,
    I: PipelineInput<Item = L0bPacket<WIDTH, HEIGHT>>,
>(
    frames: &mut I,
    progress: &mut dyn Progress,
) -> L0bFileInfo {
    debug!(
        thread_id = ?thread_id::get(),
        "Started inspecting packets"
    );
    let frames = PipelineInputIter::new(frames);
    let mut num_packets = 0;
    let mut num_frames = 0;
    let mut formats = HashSet::new();
    let mut channels = HashSet::new();
    let mut gps_seconds: Option<GpsSecondRange> = None;
    for frame in frames {
        progress.inc();
        num_packets += 1;

        let metadata: DDMFrameMetadata;
        match frame {
            L0bPacket::MP00(mp00_packet) => {
                formats.insert(L0bFormat::MP00);
                metadata = mp00_packet.header().into();
                num_frames += 1;
            }
            L0bPacket::MQ00(mq00_packet) => {
                formats.insert(L0bFormat::MQ00);
                metadata = mq00_packet.header().into();
                num_frames += 1;
            }
            L0bPacket::MT00(..) => continue,
        }

        channels.insert(metadata.channel());
        let timestamp = metadata.gps_seconds();
        let range = gps_seconds.get_or_insert(GpsSecondRange {
            start: timestamp,
            end: timestamp,
        });
        range.start = range.start.min(timestamp);
        range.end = range.end.max(timestamp);
    }

    debug!(
        thread_id = ?thread_id::get(),
        "Finished inspecting {num_packets} packets and {num_frames} frames"
    );
    L0bFileInfo {
        num_packets,
        num_frames,
        formats,
        channels,
        gps_seconds,
    }
}
