use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};

mod mp00;

fn main() {
    let path = "data/HG1_003014_20260829_102905.l0b.ddm";

    let file = File::open(path).expect("Failed to open VRQ test file");
    let mut reader = BufReader::new(file);

    let mut packet_number = 0;

    loop {
        match mp00::Mp00::deserialize(&mut reader) {
            Ok(packet) => {
                packet_number += 1;

                println!("Packet {}", packet_number);
                println!("  Packet offset: {:x}", reader.stream_position().unwrap());
                println!("  Preamble: 0x{:08X}", packet.packet_preamble);
                println!("  Packet Type: 0x{:08X}", packet.packet_type);
                println!("  Header Version: {}", packet.header_version);
                println!("  Channel: {}", packet.channel_num);
                println!("  GPS Seconds: {}", packet.timestamp_seconds);
                println!("  Sample Clocks: {}", packet.timestamp_clocks);
                println!("  Pixels: {}", packet.pixel_data.len());

                for pixels in packet.pixel_data.chunks(20) {
                    for pixel in pixels {
                        print!("{pixel} ");
                    }
                    println!();
                }
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
        if packet_number == 135 {
            reader.seek(SeekFrom::Current(4)).unwrap();
        }
    }

    println!("Total packets: {}", packet_number);
}
