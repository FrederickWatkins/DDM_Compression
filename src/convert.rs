//! Conversions between crate data types.
use anyhow::{Result, anyhow};
use ndarray::{Array2, Array4, ArrayView2, Axis};

use crate::l0b_packet::{L0bPacket, mq00::MQ00};
use crate::{DDM_CHANNEL_COUNT, DDM_HEIGHT, DDM_WIDTH};

/// Turn L0B packets into a channel-by-second MQ00 array.
///
/// MP00 packets are converted to MQ00 packets, and MT00 packets are ignored. Missing channel and
/// time combinations are `None`. If multiple packets have the same channel and timestamp, the last
/// packet in `packets` is kept.
/// The time axis starts at the earliest DDM timestamp and advances in one-second offsets.
///
/// # Errors
///
/// Returns an error if a packet names a channel outside the 16 DDM channels or if the timeline
/// dimensions cannot be represented or allocated.
pub fn into_mq00_array(packets: Vec<L0bPacket>) -> Result<Array2<Option<MQ00>>> {
    let mut time_range: Option<(u32, u32)> = None;
    for packet in &packets {
        if let Some((channel, timestamp)) = packet_metadata(packet) {
            validate_channel(channel)?;
            time_range = Some(match time_range {
                Some((start, end)) => (start.min(timestamp), end.max(timestamp)),
                None => (timestamp, timestamp),
            });
        }
    }

    let start_time = time_range.map_or(0, |(start, _)| start);
    let time_count = if let Some((start, end)) = time_range {
        usize::try_from(end - start)?
            .checked_add(1)
            .ok_or_else(|| anyhow!("L0B timeline length overflows usize"))?
    } else {
        0
    };
    let channel_count = usize::from(DDM_CHANNEL_COUNT);
    let cell_count = channel_count
        .checked_mul(time_count)
        .ok_or_else(|| anyhow!("L0B timeline dimensions overflow usize"))?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|error| anyhow!("could not allocate L0B timeline: {error}"))?;
    cells.resize_with(cell_count, || None);
    let mut output = Array2::from_shape_vec((channel_count, time_count), cells)?;

    for packet in packets {
        match packet {
            L0bPacket::MQ00(packet) => {
                set_timeline_packet(&mut output, *packet, start_time)?;
            }
            L0bPacket::MP00(packet) => {
                set_timeline_packet(&mut output, MQ00::from(*packet), start_time)?;
            }
            L0bPacket::MT00(..) => {}
        }
    }

    Ok(output)
}

fn packet_metadata(packet: &L0bPacket) -> Option<(u8, u32)> {
    match packet {
        L0bPacket::MQ00(packet) => Some((packet.channel_num(), packet.timestamp_seconds())),
        L0bPacket::MP00(packet) => Some((packet.channel_num(), packet.timestamp_seconds())),
        L0bPacket::MT00(_) => None,
    }
}

fn validate_channel(channel: u8) -> Result<()> {
    if channel >= DDM_CHANNEL_COUNT {
        return Err(anyhow!(
            "packet channel {channel} is outside the {DDM_CHANNEL_COUNT} DDM channels"
        ));
    }
    Ok(())
}

fn set_timeline_packet(
    output: &mut Array2<Option<MQ00>>,
    packet: MQ00,
    start_time: u32,
) -> Result<()> {
    let channel = packet.channel_num();
    let timestamp = packet.timestamp_seconds();
    let channel_index = usize::from(channel);
    let time_index = usize::try_from(timestamp - start_time)?;
    let Some(slot) = output.get_mut((channel_index, time_index)) else {
        return Err(anyhow!("packet timestamp is outside the timeline"));
    };
    *slot = Some(packet);
    Ok(())
}

/// Turn a channel-by-time MQ00 array into raw pixel data.
///
/// The returned dimensions are `(channels, times, 52, 20)`.
pub fn into_raw_array(packets: &ArrayView2<Option<MQ00>>) -> Result<Array4<Option<u16>>> {
    let mut output = Array4::from_elem(
        (packets.dim().0, packets.dim().1, DDM_HEIGHT, DDM_WIDTH),
        None,
    );
    for (old_channel, mut new_channel) in packets
        .axis_iter(Axis(0))
        .zip(output.axis_iter_mut(Axis(0)))
    {
        for (old_packet, mut new_packet) in
            old_channel.iter().zip(new_channel.axis_iter_mut(Axis(0)))
        {
            if let Some(packet) = old_packet {
                let pixel_data = packet.pixel_data()?;
                for (old_pixel, new_pixel) in pixel_data
                    .iter()
                    .flat_map(|row| row.iter())
                    .zip(new_packet.iter_mut())
                {
                    *new_pixel = Some(*old_pixel);
                }
            }
        }
    }
    Ok(output)
}
