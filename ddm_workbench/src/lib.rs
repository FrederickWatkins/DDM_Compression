//! DDM workbench

use ddm_core::l0b;

pub mod ui;
pub mod app;
pub mod subsystem;

const XS: f32 = 3.0;
const S: f32 = 6.0;
const M: f32 = 12.0;
const L: f32 = 24.0;
const XL: f32 = 48.0;
const WIDTH: usize = 20;
const HEIGHT: usize = 52;

type DDMFrame = l0b::DDMFrame<WIDTH, HEIGHT>;
type MQ00Packet = l0b::MQ00Packet<WIDTH, HEIGHT>;
type MP00Packet = l0b::MP00Packet<WIDTH, HEIGHT>;
type DDMFrameMap = ddm_app::DDMFrameMap<WIDTH, HEIGHT>;
type L0bFileData = ddm_app::L0bFileData<WIDTH, HEIGHT>;
