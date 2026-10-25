use eyre::eyre;

use crate::{DDMFrame, L0bFileData};

/// Global app state
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct App {
    current_timestamp: u32,
    current_channel: u8,
    l0b_data: L0bFileData,
}

impl App {
    /// Create app
    ///
    /// # Errors
    /// Errors if `l0b_data` contains no frames
    pub fn new(l0b_data: L0bFileData) -> eyre::Result<Self> {
        if let Some(gps_seconds) = l0b_data.info.gps_seconds {
            Ok(Self {
                current_timestamp: gps_seconds.start,
                current_channel: 0,
                l0b_data,
            })
        } else {
            Err(eyre!("L0b file contains no frames"))
        }
    }

    /// Change timestamp returning new value
    ///
    /// # Panics
    /// If `l0b_data` is empty
    #[expect(clippy::unwrap_used, reason = "Already clamped value")]
    pub fn change_timestamp(&mut self, delta: i64) {
        let gps_seconds = self.l0b_data.info.gps_seconds.unwrap();
        self.current_timestamp = (i64::from(self.current_timestamp) + delta)
            .clamp(i64::from(gps_seconds.start), i64::from(gps_seconds.end))
            .try_into()
            .unwrap();
    }

    /// Change channel
    #[expect(
        clippy::unwrap_used,
        clippy::missing_panics_doc,
        reason = "Value already clamped"
    )]
    pub fn change_channel(&mut self, channel: i8) {
        self.current_channel = (i8::try_from(self.current_channel).unwrap() + channel)
            .clamp(0, 15)
            .try_into()
            .unwrap();
    }

    /// Get current frame
    pub fn get_frame(&self) -> Option<&DDMFrame> {
        self.l0b_data
            .frames
            .get((self.current_channel, self.current_timestamp))
            .and_then(|opt| opt.as_ref())
    }
}
