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
        let mut total_frames = 0_usize;
        let mut start_time = u32::MAX;
        let mut end_time = u32::MIN;
        let mut channels_set = HashSet::new();
        let input: Vec<DDMFrame<_, _>> = iter
            .into_iter()
            .inspect(|frame| {
                start_time = start_time.min(frame.metadata().gps_seconds());
                end_time = end_time.max(frame.metadata().gps_seconds());
                let channel = frame.metadata().channel();
                if usize::from(channel) < 16 {
                    channels_set.insert(channel);
                } else {
                    warn!(channel, "Dropping frame with invalid channel number");
                }
            })
            .collect();
        if input.is_empty() {
            return Self::default();
        }
        let Some(num_timestamps) = end_time
            .checked_sub(start_time)
            .and_then(|difference| difference.checked_add(1))
        else {
            warn!("Frame timestamp range cannot be represented");
            return Self::default();
        };
        let channels = physical_channels(&channels_set);
        let Ok(num_timestamps_usize) = usize::try_from(num_timestamps) else {
            warn!("Frame timestamp count cannot be represented on this platform");
            return Self::default();
        };
        let frames = Array2::from_elem([channels_set.len(), num_timestamps_usize], None);
        let mut output = Self {
            channels,
            start_time,
            num_timestamps,
            frames,
        };
        for frame in input {
            total_frames = total_frames.saturating_add(1);
            let channel = frame.metadata().channel();
            let gps_seconds = frame.metadata().gps_seconds();
            if output
                .get((channel, gps_seconds))
                .is_some_and(Option::is_some)
            {
                warn!(
                    thread_id = ?thread_id::get(),
                    "Frame stream contains duplicated frame at channel: {channel} gps_seconds {gps_seconds}"
                );
            }
            if let Some(slot) = output.get_mut((channel, gps_seconds)) {
                *slot = Some(frame);
            }
        }
        debug!(
            thread_id = ?thread_id::get(),
            "Finished gathering {total_frames} frames from frame stream into frame map"
        );
        output
    }
}

fn physical_channels(channels_set: &HashSet<u8>) -> [Option<u8>; 16] {
    let mut channels = [None; 16];
    let mut physical_channel = 0_u8;
    for (logical_index, slot) in channels.iter_mut().enumerate() {
        let Ok(logical_channel) = u8::try_from(logical_index) else {
            continue;
        };
        if channels_set.contains(&logical_channel) {
            *slot = Some(physical_channel);
            physical_channel = physical_channel.saturating_add(1);
        }
    }
    channels
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

impl<const WIDTH: usize, const HEIGHT: usize> Default for DDMFrameMap<WIDTH, HEIGHT> {
    fn default() -> Self {
        Self {
            channels: [None; 16],
            start_time: 0,
            num_timestamps: 0,
            frames: Array2::from_elem([0, 0], None),
        }
    }
}
