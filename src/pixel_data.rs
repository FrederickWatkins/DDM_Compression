use crate::dwt;
use anyhow::Result;
use bitstream_io::{BitRead, BitWrite};

pub trait PixelData
where
    Self: Sized,
{
    fn binarize_bitplanes<W: BitWrite>(&self, writer: &mut W) -> Result<()>;

    fn unbinarize_bitplanes<R: BitRead>(reader: &mut R, frame_count: usize) -> Result<Self>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Frame<T>([[T; 20]; 52]);

impl Frame<u16> {
    pub fn overflowing_sub(&self, value: &Self) -> Frame<i16> {
        let mut output = Frame([[0i16; _]; _]);
        for ((lhs, rhs), result) in self
            .0
            .iter()
            .flatten()
            .zip(value.0.iter().flatten())
            .zip(output.0.iter_mut().flatten())
        {
            *result = lhs.overflowing_sub(*rhs).0 as i16;
        }
        output
    }

    pub fn overflowing_add(&self, value: &Frame<i16>) -> Frame<u16> {
        let mut output = Frame([[0u16; _]; _]);
        for ((lhs, rhs), result) in self
            .0
            .iter()
            .flatten()
            .zip(value.0.iter().flatten())
            .zip(output.0.iter_mut().flatten())
        {
            *result = lhs.overflowing_add(*rhs as u16).0;
        }
        output
    }

    pub fn dwt53(&self) -> Frame<i32> {
        let mut output = Frame([[0i32; _]; _]);
        for (old, new) in self
            .0
            .iter()
            .into_iter()
            .flatten()
            .zip(output.0.iter_mut().into_iter().flatten())
        {
            *new = *old as i32;
        }

        for row in &mut output.0 {
            let transformed = dwt::dwt53(row);
            row.copy_from_slice(&transformed);
        }

        let mut transposed: [[i32; 52]; 20] =
            std::array::from_fn(|col| std::array::from_fn(|row| output.0[row][col]));

        for row in &mut transposed {
            let transformed = dwt::dwt53(row);
            row.copy_from_slice(&transformed);
        }

        output.0 = std::array::from_fn(|row| std::array::from_fn(|col| transposed[col][row]));

        output
    }

    pub fn idwt53(mut value: Frame<i32>) -> Self {
        let mut transposed: [[i32; 52]; 20] =
            std::array::from_fn(|col| std::array::from_fn(|row| value.0[row][col]));

        for row in &mut transposed {
            let transformed = dwt::idwt53(row);
            row.copy_from_slice(&transformed);
        }

        value.0 = std::array::from_fn(|row| std::array::from_fn(|col| transposed[col][row]));

        for row in &mut value.0 {
            let transformed = dwt::idwt53(row);
            row.copy_from_slice(&transformed);
        }

        let mut output = Frame([[0u16; 20]; 52]);

        for (old, new) in value.0.iter().flatten().zip(output.0.iter_mut().flatten()) {
            *new = *old as u16;
        }

        output
    }
}

impl Frame<i16> {
    pub fn dwt53(&self) -> Frame<i32> {
        let mut output = Frame([[0i32; _]; _]);
        for (old, new) in self
            .0
            .iter()
            .into_iter()
            .flatten()
            .zip(output.0.iter_mut().into_iter().flatten())
        {
            *new = *old as i32;
        }

        for row in &mut output.0 {
            let transformed = dwt::dwt53(row);
            row.copy_from_slice(&transformed);
        }

        let mut transposed: [[i32; 52]; 20] =
            std::array::from_fn(|col| std::array::from_fn(|row| output.0[row][col]));

        for row in &mut transposed {
            let transformed = dwt::dwt53(row);
            row.copy_from_slice(&transformed);
        }

        output.0 = std::array::from_fn(|row| std::array::from_fn(|col| transposed[col][row]));

        output
    }

    pub fn idwt53(mut value: Frame<i32>) -> Self {
        let mut transposed: [[i32; 52]; 20] =
            std::array::from_fn(|col| std::array::from_fn(|row| value.0[row][col]));

        for row in &mut transposed {
            let transformed = dwt::idwt53(row);
            row.copy_from_slice(&transformed);
        }

        value.0 = std::array::from_fn(|row| std::array::from_fn(|col| transposed[col][row]));

        for row in &mut value.0 {
            let transformed = dwt::idwt53(row);
            row.copy_from_slice(&transformed);
        }

        let mut output = Frame([[0i16; 20]; 52]);

        for (old, new) in value.0.iter().flatten().zip(output.0.iter_mut().flatten()) {
            *new = *old as i16;
        }

        output
    }
}

#[derive(Clone, Debug)]
pub struct RawPixelData(Vec<Frame<u16>>);

#[derive(Clone, Debug)]
pub struct TimeDeltaPixelData(Vec<Frame<i16>>);

impl From<RawPixelData> for TimeDeltaPixelData {
    fn from(value: RawPixelData) -> Self {
        let mut prev_frame = Frame([[0u16; _]; _]);
        let mut output = TimeDeltaPixelData(vec![]);
        for frame in value.0 {
            output.0.push(frame.overflowing_sub(&prev_frame));
            prev_frame = frame;
        }
        output
    }
}

impl From<TimeDeltaPixelData> for RawPixelData {
    fn from(value: TimeDeltaPixelData) -> Self {
        let mut prev_frame = Frame([[0u16; _]; _]);
        let mut output = RawPixelData(vec![]);
        for frame in value.0 {
            prev_frame = prev_frame.overflowing_add(&frame);
            output.0.push(prev_frame);
        }
        output
    }
}

#[derive(Clone, Debug)]
pub struct TimeDeltaWaveletPixelData(Vec<Frame<i32>>);

impl From<TimeDeltaPixelData> for TimeDeltaWaveletPixelData {
    fn from(value: TimeDeltaPixelData) -> Self {
        let mut output = vec![];
        for frame in value.0 {
            output.push(frame.dwt53());
        }
        TimeDeltaWaveletPixelData(output)
    }
}

impl From<TimeDeltaWaveletPixelData> for TimeDeltaPixelData {
    fn from(mut value: TimeDeltaWaveletPixelData) -> Self {
        let mut output = vec![];
        for frame in value.0.iter_mut() {
            output.push(Frame::<i16>::idwt53(*frame));
        }
        TimeDeltaPixelData(output)
    }
}

const COEFFICIENT_BITS: u32 = 20;
const COEFFICIENT_MASK: u32 = (1 << COEFFICIENT_BITS) - 1;
const COEFFICIENT_MIN: i32 = -(1 << 19);
const COEFFICIENT_MAX: i32 = (1 << 19) - 1;
const PIXELS_PER_FRAME: usize = 52 * 20;

impl PixelData for TimeDeltaWaveletPixelData {
    fn binarize_bitplanes<W: BitWrite>(&self, writer: &mut W) -> Result<()> {
        for bit in (0..COEFFICIENT_BITS).rev() {
            for frame in &self.0 {
                for &coefficient in frame.0.iter().flatten() {
                    let encoded = (coefficient as u32) & COEFFICIENT_MASK;
                    writer.write_bit((encoded & (1 << bit)) != 0)?;
                }
            }
        }

        Ok(())
    }

    fn unbinarize_bitplanes<R: BitRead>(reader: &mut R, frame_count: usize) -> Result<Self> {
        let coefficient_count = frame_count * PIXELS_PER_FRAME;
        let mut coefficients = vec![0u32; coefficient_count];

        for bit in (0..COEFFICIENT_BITS).rev() {
            for coefficient in &mut coefficients {
                if reader.read_bit()? {
                    *coefficient |= 1 << bit;
                }
            }
        }

        let mut frames = Vec::with_capacity(frame_count);
        let mut coefficients = coefficients.into_iter();

        for _ in 0..frame_count {
            let mut frame = Frame([[0i32; 20]; 52]);

            for coefficient in frame.0.iter_mut().flatten() {
                let encoded = coefficients.next().unwrap();
                *coefficient = if encoded & (1 << 19) != 0 {
                    (encoded | !COEFFICIENT_MASK) as i32
                } else {
                    encoded as i32
                };
            }

            frames.push(frame);
        }

        Ok(Self(frames))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ops::Range;
    use std::{fmt::Debug, io::Cursor};

    use bitstream_io::{BigEndian, BitReader, BitWriter};
    use proptest::prelude::*;

    fn arb_frame<T>(values: impl Strategy<Value = T>) -> impl Strategy<Value = Frame<T>>
    where
        T: Debug + Copy + 'static,
    {
        prop::collection::vec(prop::collection::vec(values, 20), 52).prop_map(|rows| {
            let rows: Vec<[T; 20]> = rows
                .into_iter()
                .map(|row| row.try_into().unwrap())
                .collect();

            Frame(rows.try_into().unwrap())
        })
    }

    fn arb_frame_series<T>(
        length: Range<usize>,
        values: impl Strategy<Value = T>,
    ) -> impl Strategy<Value = Vec<Frame<T>>>
    where
        T: Debug + Copy + 'static,
    {
        prop::collection::vec(arb_frame(values), length)
    }

    proptest! {
        #[test]
        fn test_time_delta_roundtrip(
            frames in arb_frame_series(0..100, any::<u16>())
        ) {
            let original = RawPixelData(frames);
            let encoded = TimeDeltaPixelData::from(original.clone());
            let decoded = RawPixelData::from(encoded);

            prop_assert_eq!(decoded.0, original.0);
        }

        #[test]
        fn test_wavelet_round_trip(
            frame in arb_frame(any::<u16>())
        ) {
            let encoded = frame.dwt53();
            let decoded = Frame::<u16>::idwt53(encoded);

            prop_assert_eq!(decoded.0, frame.0);
        }

        #[test]
        fn test_time_delta_wavelet_roundtrip(
            frames in arb_frame_series(0..100, any::<u16>())
        ) {
            let original = RawPixelData(frames);
            let encoded: TimeDeltaWaveletPixelData = TimeDeltaPixelData::from(original.clone()).into();
            let decoded: RawPixelData = TimeDeltaPixelData::from(encoded).into();

            prop_assert_eq!(decoded.0, original.0);
        }

        #[test]
        fn test_bitplanes_encoding_roundtrip(
            frames in arb_frame_series(0..10, -524288i32..=524287i32)
        ) {
            let original = TimeDeltaWaveletPixelData(frames);

            let mut writer = BitWriter::endian(Vec::new(), BigEndian);
            original.binarize_bitplanes(&mut writer).unwrap();
            let bytes = writer.into_writer();

            let mut reader = BitReader::endian(Cursor::new(bytes), BigEndian);
            let decoded = TimeDeltaWaveletPixelData::unbinarize_bitplanes(
                &mut reader,
                original.0.len(),
            ).unwrap();

            prop_assert_eq!(decoded.0, original.0);
        }

        #[test]
        fn test_time_delta_wavelet_bitplanes_roundtrip(
            frames in arb_frame_series(0..100, any::<u16>())
        ) {
            let original = RawPixelData(frames);

            let encoded: TimeDeltaWaveletPixelData =
                TimeDeltaPixelData::from(original.clone()).into();

            let mut writer = BitWriter::endian(Vec::new(), BigEndian);
            encoded.binarize_bitplanes(&mut writer).unwrap();
            let bytes = writer.into_writer();

            let mut reader = BitReader::endian(Cursor::new(bytes), BigEndian);
            let decoded_wavelet = TimeDeltaWaveletPixelData::unbinarize_bitplanes(
                &mut reader,
                encoded.0.len(),
            ).unwrap();

            let decoded: RawPixelData =
                TimeDeltaPixelData::from(decoded_wavelet).into();

            prop_assert_eq!(decoded.0, original.0);
        }
    }
}
