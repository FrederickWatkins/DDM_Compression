use ddm_compression::visualisation::DdmViewer;
use ddm_compression::ddm_frame::DdmFrame;

fn load_frames() -> Vec<Vec<DdmFrame>> {
    let encoded = std::fs::read("data/frames.bin.zst").expect("Failed to read frames");

    let decompressed = zstd::decode_all(encoded.as_slice()).unwrap();

    let (ddm_frames, _) = bincode::serde::decode_from_slice::<Vec<Vec<DdmFrame>>, _>(
        &decompressed,
        bincode::config::standard(),
    )
    .expect("Failed to deserialize frame");
    ddm_frames
}

fn main() -> eframe::Result {
    // Replace this with however you construct/load your frames.
    let frames: Vec<Vec<DdmFrame>> = load_frames();

    let native_options = eframe::NativeOptions::default();

    eframe::run_native(
        "DDM Frame Viewer",
        native_options,
        Box::new(move |_cc| Ok(Box::new(DdmViewer::new(frames)))),
    )
}
