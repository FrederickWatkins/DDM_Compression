#![allow(dead_code)]

pub mod l0b_packet;

const DDM_HEIGHT: usize = 52;
const DDM_WIDTH: usize = 20;

type DDMPixelData = [[u16; DDM_WIDTH]; DDM_HEIGHT];