use ndarray::{Array2, Array4, Axis};
use anyhow::Result;

use crate::l0b_packet::{L0bPacket, mq00::MQ00};

pub fn into_mq00_array(packets: Vec<L0bPacket>) -> Array2<Option<MQ00>> {
    let start_time = packets
        .iter()
        .filter_map(|packet| match packet {
            L0bPacket::MQ00(packet) => Some(packet.timestamp_seconds()),
            L0bPacket::MP00(packet) => Some(packet.timestamp_seconds()),
            _ => None,
        })
        .min()
        .unwrap_or(0) as usize;

    let end_time = packets
        .iter()
        .filter_map(|packet| match packet {
            L0bPacket::MQ00(packet) => Some(packet.timestamp_seconds()),
            L0bPacket::MP00(packet) => Some(packet.timestamp_seconds()),
            _ => None,
        })
        .max()
        .unwrap_or(0) as usize;

    let mut output = Array2::from_elem((16, end_time - start_time + 1), None);
    for packet in packets {
        match packet {
            L0bPacket::MQ00(packet) => {
                let time_index = packet.timestamp_seconds() as usize - start_time;
                let channel_index = packet.channel_num() as usize;
                output[[channel_index, time_index]] = Some(*packet);
            }
            L0bPacket::MP00(packet) => {
                let time_index = packet.timestamp_seconds() as usize - start_time;
                let channel_index = packet.channel_num() as usize;
                output[[channel_index, time_index]] = Some(MQ00::from(*packet));
            }
            _ => {}
        }
    }
    output
}

pub fn into_raw_array(packets: Array2<Option<MQ00>>) -> Result<Array4<Option<u16>>> {
    let mut output = Array4::from_elem((16, packets.dim().1, 52, 20), None);
    for (old_channel, mut new_channel) in packets.axis_iter(Axis(0)).zip(output.axis_iter_mut(Axis(0))) {
        for (old_packet, mut new_packet) in old_channel.iter().zip(new_channel.axis_iter_mut(Axis(0))) {
            if let Some(packet) = old_packet {
                let pixel_data = packet.pixel_data()?;
                for (old_pixel, new_pixel) in pixel_data.iter().flat_map(|row| row.iter()).zip(new_packet.iter_mut()) {
                    *new_pixel = Some(*old_pixel);
                }
            }
        }
    }
    Ok(output)
}
