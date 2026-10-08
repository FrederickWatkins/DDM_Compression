use std::fs::File;
use std::io::{BufReader, Seek};

use crate::l0b_packet::{Decode, L0bPacket, L0bPacketType};

mod l0b_packet;

fn main() {
    let path = "data/HG1_003014_20260829_102905.l0b.ddm";

    let file = File::open(path).expect("Failed to open VRQ test file");
    let mut reader = BufReader::new(file);

    let mut packet_number = 0;

    loop {
        let stream_position = reader.stream_position().unwrap();
        match L0bPacket::decode(&mut reader) {
            Ok(L0bPacket {
                packet_preamble,
                packet_type: L0bPacketType::Mp00(mp00),
            }) => {
                packet_number += 1;

                println!("Packet {}", packet_number);
                println!("  Packet offset: {:x}", stream_position);
                println!("  Preamble: 0x{:08X}", packet_preamble);
                println!("  Packet Type: MP00");
                println!("  Header Version: {}", mp00.header_version);
                println!("  Channel: {}", mp00.channel_num);
                println!("  GPS Seconds: {}", mp00.timestamp_seconds);
                println!("  Sample Clocks: {}", mp00.timestamp_clocks);

                for pixels in mp00.pixel_data.chunks(20) {
                    for pixel in pixels {
                        print!("{pixel} ");
                    }
                    println!();
                }
            }

            Ok(L0bPacket {
                packet_preamble,
                packet_type: L0bPacketType::Mt00(_),
            }) => {
                packet_number += 1;

                println!("Packet {}", packet_number);
                println!("  Packet offset: {:x}", stream_position);
                println!("  Preamble: 0x{:08X}", packet_preamble);
                println!("  Packet Type: MT00");
            }

            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                println!("Reached end of file.");
                break;
            }

            Err(e) => {
                eprintln!("Error reading packet {}: {}", packet_number + 1, e);
                break;
            }
        }
    }

    println!("Total packets: {}", packet_number);
}
