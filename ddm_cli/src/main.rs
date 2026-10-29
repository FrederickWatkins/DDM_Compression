//! DDM CLI tools
#![allow(dead_code, reason = "Might use in future")]
use std::{
    env,
    io::{BufReader, ErrorKind::UnexpectedEof},
};

use binrw::{BinRead as _, Endian};
use color_eyre::eyre;
use ddm_core::l0b::{DDMDecoder as _, L0bPacket, MP00Decoder};

mod backtrace;
use backtrace::install_error_reporting as install_eyre_hook;
use duckdb::Connection;

mod dwt_image;

mod benchmark;

fn main() -> eyre::Result<()> {
    const MAX_FRAMES: usize = 16;

    install_eyre_hook()?;

    let mut args = env::args().skip(1);
    let input = args
        .next()
        .ok_or(eyre::eyre!("usage: ddm_cli <input.l0b.ddm> <output.db>"))?;
    let output = args
        .next()
        .ok_or(eyre::eyre!("usage: ddm_cli <input.l0b.ddm> <output.db>"))?;

    let file = std::fs::File::open(input)?;
    let mut reader = BufReader::new(file);

    let mut frames = vec![];

    let mut decoder = MP00Decoder::new();

    while frames.len() < MAX_FRAMES {
        match L0bPacket::read_options(&mut reader, Endian::Big, (52, 20)) {
            Ok(L0bPacket::MP00(packet)) => {
                frames.push(decoder.decode(packet)?.pixel_data().mapv(i32::from));
            }
            Ok(L0bPacket::MT00(..)) => (),
            Err(binrw::Error::Io(e)) if e.kind() == UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
    }

    eyre::ensure!(!frames.is_empty(), "No MP00 frames found in input");
    eprintln!("Decoded {} frames, benchmarking...", frames.len());

    let mut conn = Connection::open(output)?;
    benchmark::benchmark_all(&frames, &mut conn)?;

    Ok(())
}
