use std::{collections::HashSet, ops::Index};

use ddm_core::l0b::DDMFrame;
use ndarray::{Array2, Ix2};
use prodash::Progress;

use crate::pipeline::PipelineInput;

pub struct DDMFrameMap<const WIDTH: usize, const HEIGHT: usize> {
    channels: [Option<u8>; 16],
    start_time: u32,
    frames: Array2<DDMFrame<WIDTH, HEIGHT>>,
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMFrameMap<WIDTH, HEIGHT> {
    pub fn gather<I: PipelineInput<Item = DDMFrame<WIDTH, HEIGHT>>>(
        input: &mut I,
        timestamp_range: (u32, u32),
        channels: HashSet<u8>,
        progress: &mut dyn Progress,
    ) {
        todo!()
    }
}

impl<'a, const WIDTH: usize, const HEIGHT: usize> IntoIterator for &'a DDMFrameMap<WIDTH, HEIGHT> {
    type IntoIter = ndarray::iter::Iter<'a, DDMFrame<WIDTH, HEIGHT>, Ix2>;
    type Item = &'a DDMFrame<WIDTH, HEIGHT>;

    fn into_iter(self) -> Self::IntoIter {
        self.frames.iter()
    }
}

impl<'a, const WIDTH: usize, const HEIGHT: usize> Index<Ix2> for &'a DDMFrameMap<WIDTH, HEIGHT> {
    type Output = &'a DDMFrame<WIDTH, HEIGHT>;

    fn index(&self, index: Ix2) -> &Self::Output {
        todo!()
    }
}

impl<'a, const WIDTH: usize, const HEIGHT: usize> Index<Ix2> for &'a mut DDMFrameMap<WIDTH, HEIGHT> {
    type Output = &'a mut DDMFrame<WIDTH, HEIGHT>;

    fn index(&self, index: Ix2) -> &Self::Output {
        todo!()
    }
}
