use std::{
    collections::HashSet,
    ops::{Index, IndexMut},
};

use ddm_core::l0b::DDMFrame;
use ndarray::{Array2, Ix2};
use prodash::Progress;
use tracing::{debug, warn};

use crate::pipeline::{PipelineInput, PipelineInputIter};

/// Frames mapped into contiguous structure in memory, indexable by channel and timestamp
pub struct DDMFrameMap<const WIDTH: usize, const HEIGHT: usize> {
    channels: [Option<u8>; 16],
    start_time: u32,
    num_timestamps: u32,
    frames: Array2<Option<DDMFrame<WIDTH, HEIGHT>>>,
}

impl<const WIDTH: usize, const HEIGHT: usize> DDMFrameMap<WIDTH, HEIGHT> {
    fn physical_index(&self, index: (u8, u32)) -> Option<[usize; 2]> {
        let Some(Some(channel)) = self.channels.get(usize::from(index.0)) else {
            return None;
        };
        let timestamp = usize::try_from(index.1.checked_sub(self.start_time)?).ok()?;
        if timestamp >= self.frames.dim().1 {
            return None;
        }
        Some([usize::from(*channel), timestamp])
    }

    /// Get frame at channel and gps timestamp
    ///
    /// Returns None for invalid index, or Some(None) for valid index with no frame present
    pub fn get(&self, index: (u8, u32)) -> Option<&Option<DDMFrame<WIDTH, HEIGHT>>> {
        self.frames.get(self.physical_index(index)?)
    }

    /// Get frame at channel and gps timestamp
    ///
    /// Returns None for invalid index, or Some(None) for valid index with no frame present
    pub fn get_mut(&mut self, index: (u8, u32)) -> Option<&mut Option<DDMFrame<WIDTH, HEIGHT>>> {
        let physical_index = self.physical_index(index)?;
        self.frames.get_mut(physical_index)
    }

    /// Create frame map from pipeline stream of frames
    #[tracing::instrument(skip_all, level = "trace")]
    pub fn gather<I: PipelineInput<Item = DDMFrame<WIDTH, HEIGHT>>>(
        input: &mut I,
        progress: &mut dyn Progress,
    ) -> Self {
        let input = PipelineInputIter::new(input).inspect(|_| progress.inc());
        Self::from_iter(input)
    }

    /// Iterate over frames in timestamp-major order.
    pub fn iter(&self) -> ndarray::iter::Iter<'_, Option<DDMFrame<WIDTH, HEIGHT>>, Ix2> {
        self.frames.t().into_iter()
    }

    /// Returns the start time of the data
    pub fn start_time(&self) -> u32 {
        self.start_time
    }

    /// Returns the length of the data on the time axis
    pub fn num_timestamps(&self) -> u32 {
        self.num_timestamps
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> FromIterator<DDMFrame<WIDTH, HEIGHT>>
    for DDMFrameMap<WIDTH, HEIGHT>
{
    fn from_iter<T: IntoIterator<Item = DDMFrame<WIDTH, HEIGHT>>>(iter: T) -> Self {
        debug!(
            thread_id = ?thread_id::get(),
            "Started gathering frame stream into frame map"
        );
        let mut total_frames = 0;
        let mut start_time = u32::MAX;
        let mut end_time = u32::MIN;
        let mut channels_set = HashSet::new();
        let input: Vec<DDMFrame<_, _>> = iter
            .into_iter()
            .inspect(|frame| {
                start_time = start_time.min(frame.metadata().gps_seconds());
                end_time = end_time.max(frame.metadata().gps_seconds());
                channels_set.insert(frame.metadata().channel());
            })
            .collect();
        if input.is_empty() {
            return Self {
                channels: [None; 16],
                start_time: 0,
                num_timestamps: 0,
                frames: Array2::from_elem([0, 0], None),
            };
        }
        let num_timestamps = end_time - start_time + 1;
        let mut channels = [None; 16];
        let mut physical_channel_num = 0_u8;
        let mut logical_channel_num = 0_u8;
        while usize::from(logical_channel_num) < channels.len() {
            let logical_index = usize::from(logical_channel_num);
            if channels_set.contains(&logical_channel_num) {
                channels[logical_index] = Some(physical_channel_num);
                physical_channel_num += 1;
            }
            logical_channel_num += 1;
        }
        let frames = Array2::from_elem([channels_set.len(), num_timestamps as usize], None);
        let mut output = Self {
            channels,
            start_time,
            num_timestamps,
            frames,
        };
        for frame in input {
            total_frames += 1;
            let channel = frame.metadata().channel();
            let gps_seconds = frame.metadata().gps_seconds();
            if output[(channel, gps_seconds)].is_some() {
                warn!(
                    thread_id = ?thread_id::get(),
                    "Frame stream contains duplicated frame at channel: {channel} gps_seconds {gps_seconds}"
                );
            }
            output[(channel, gps_seconds)] = Some(frame);
        }
        debug!(
            thread_id = ?thread_id::get(),
            "Finished gathering {total_frames} frames from frame stream into frame map"
        );
        output
    }
}

impl<'a, const WIDTH: usize, const HEIGHT: usize> IntoIterator for &'a DDMFrameMap<WIDTH, HEIGHT> {
    type IntoIter = ndarray::iter::Iter<'a, Option<DDMFrame<WIDTH, HEIGHT>>, Ix2>;
    type Item = &'a Option<DDMFrame<WIDTH, HEIGHT>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> Index<(u8, u32)> for DDMFrameMap<WIDTH, HEIGHT> {
    type Output = Option<DDMFrame<WIDTH, HEIGHT>>;

    fn index(&self, index: (u8, u32)) -> &Self::Output {
        #[expect(
            clippy::expect_used,
            reason = "Indexing follows the standard collection panic contract"
        )]
        self.get(index).expect("index should be legal")
    }
}

impl<const WIDTH: usize, const HEIGHT: usize> IndexMut<(u8, u32)> for DDMFrameMap<WIDTH, HEIGHT> {
    fn index_mut(&mut self, index: (u8, u32)) -> &mut Self::Output {
        #[expect(
            clippy::expect_used,
            reason = "Mutable indexing follows the standard collection panic contract"
        )]
        self.get_mut(index).expect("index should be legal")
    }
}
