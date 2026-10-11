pub struct DdmSeries<T> {
    channel_num: u8,
    timestamp_seconds: Vec<u32>,
    timestamp_clocks: Vec<i32>,
    pixel_data: T,
}

impl<T> DdmSeries<T> {
    pub fn recode<U: std::convert::From<T>>(self) -> DdmSeries<U> {
        DdmSeries {
            channel_num: self.channel_num,
            timestamp_seconds: self.timestamp_seconds,
            timestamp_clocks: self.timestamp_clocks,
            pixel_data: self.pixel_data.into(),
        }
    }
}
