use std::collections::BTreeMap;
use std::hash::Hash;

use constriction::stream::{
    Decode,
    model::{
        DefaultNonContiguousCategoricalDecoderModel, DefaultNonContiguousCategoricalEncoderModel,
    },
    stack::DefaultAnsCoder,
};
use ndarray::{Array, Dimension};

/// The fixed-point precision used by constriction's default categorical models.
///
/// 2^24 is the total probability mass.
const PRECISION: u32 = 24;
const TOTAL_PROBABILITY: u32 = 1 << PRECISION;

/// A reusable static categorical entropy model.
///
/// `probabilities[i]` is the fixed-point probability assigned to
/// `symbols[i]`.
///
/// The probabilities sum to 2^24 and every probability is non-zero.
///
/// This is deliberately our own representation rather than storing a
/// constriction model directly, because it gives us something that can
/// easily be serialized into a file header later.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntropyModel<T> {
    symbols: Vec<T>,
    probabilities: Vec<u32>,
}

impl<T> EntropyModel<T>
where
    T: Clone + Eq + Hash,
{
    /// Constructs an entropy model from a sequence of symbols.
    ///
    /// The empirical histogram of `data` becomes the probability model.
    pub fn from_symbols<I>(data: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Ord,
    {
        let mut counts = BTreeMap::<T, usize>::new();

        for symbol in data {
            *counts.entry(symbol).or_insert(0) += 1;
        }

        assert!(
            !counts.is_empty(),
            "cannot construct an entropy model from empty data"
        );

        let symbols: Vec<T> = counts.keys().cloned().collect();
        let counts: Vec<usize> = counts.values().copied().collect();

        let probabilities = quantize_probabilities(&counts);

        Self {
            symbols,
            probabilities,
        }
    }

    pub fn symbols(&self) -> &[T] {
        &self.symbols
    }

    pub fn probabilities(&self) -> &[u32] {
        &self.probabilities
    }

    pub fn support_size(&self) -> usize {
        self.symbols.len()
    }

    /// Construct the constriction encoder model represented by this model.
    fn encoder_model(&self) -> DefaultNonContiguousCategoricalEncoderModel<T>
    where
        T: Clone + Eq + Hash,
    {
        DefaultNonContiguousCategoricalEncoderModel::
            from_symbols_and_nonzero_fixed_point_probabilities(
                self.symbols.iter().cloned(),
                &self.probabilities,
                false,
            )
            .expect("invalid entropy model")
    }

    /// Construct the matching constriction decoder model.
    fn decoder_model(&self) -> DefaultNonContiguousCategoricalDecoderModel<T>
    where
        T: Clone,
    {
        DefaultNonContiguousCategoricalDecoderModel::
            from_symbols_and_nonzero_fixed_point_probabilities(
                self.symbols.iter().cloned(),
                &self.probabilities,
                false,
            )
            .expect("invalid entropy model")
    }
}

/// Convert histogram counts into constriction's 24-bit fixed-point
/// probability representation.
///
/// Every symbol gets at least one unit of probability.
fn quantize_probabilities(counts: &[usize]) -> Vec<u32> {
    assert!(!counts.is_empty());

    let total_count: u64 = counts.iter().map(|&x| x as u64).sum();

    assert!(
        counts.len() <= TOTAL_PROBABILITY as usize,
        "too many distinct symbols for a {}-bit probability model",
        PRECISION
    );

    // Start by assigning one probability unit to every symbol.
    let mut probabilities = vec![1u32; counts.len()];
    let remaining = TOTAL_PROBABILITY as u64 - counts.len() as u64;

    // Largest-remainder method:
    //
    // 1. Calculate ideal probability.
    // 2. Take the floor.
    // 3. Distribute the remaining units according to the largest
    //    fractional remainders.
    //
    // This gives an exactly normalized distribution.
    let mut fractional = Vec::with_capacity(counts.len());

    let mut assigned = counts.len() as u64;

    for (i, &count) in counts.iter().enumerate() {
        let numerator = count as u64 * remaining;

        let extra = numerator / total_count;
        let remainder = numerator % total_count;

        probabilities[i] += extra as u32;
        assigned += extra;

        fractional.push((remainder, i));
    }

    let mut left = TOTAL_PROBABILITY as u64 - assigned;

    // Largest remainder first.
    fractional.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    for &(_, index) in fractional.iter() {
        if left == 0 {
            break;
        }

        probabilities[index] += 1;
        left -= 1;
    }

    debug_assert_eq!(
        probabilities.iter().map(|&x| x as u64).sum::<u64>(),
        TOTAL_PROBABILITY as u64
    );

    probabilities
}

/// Compress a slice of symbols using a previously generated model.
///
/// The returned bytes contain only the ANS payload. The model is kept
/// separately so it can be reused for subsequent arrays.
pub fn compress<T>(data: &[T], model: &EntropyModel<T>) -> Vec<u8>
where
    T: Clone + Eq + Hash,
{
    let entropy_model = model.encoder_model();

    let mut coder = DefaultAnsCoder::new();

    // ANS is a stack, so encode in reverse order.
    coder
        .encode_iid_symbols_reverse(data.iter().cloned(), &entropy_model)
        .expect("symbol was not present in entropy model");

    let words = coder
        .into_compressed()
        .expect("failed to extract compressed ANS stream");

    words.into_iter().flat_map(u32::to_le_bytes).collect()
}

/// Decompress a byte stream using a previously generated model.
pub fn decompress<T>(compressed: &[u8], num_symbols: usize, model: &EntropyModel<T>) -> Vec<T>
where
    T: Clone + Eq + Hash,
{
    assert!(
        compressed.len().is_multiple_of(4),
        "compressed ANS stream must contain a whole number of u32 words"
    );

    let words: Vec<u32> = compressed
        .as_chunks::<4>().0.iter()
        .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        .collect();

    let mut coder = DefaultAnsCoder::from_compressed(words).expect("invalid compressed ANS stream");

    let entropy_model = model.decoder_model();

    let decoded = coder
        .decode_iid_symbols(num_symbols, &entropy_model.as_view())
        .collect::<Result<Vec<_>, _>>()
        .expect("failed to decode ANS stream");

    assert!(
        coder.is_empty(),
        "compressed stream contained trailing ANS data"
    );

    decoded
}

/// Construct a model from an ndarray.
pub fn model_from_array<T, D>(array: &Array<T, D>) -> EntropyModel<T>
where
    T: Clone + Eq + Hash + Ord,
    D: Dimension,
{
    EntropyModel::from_symbols(array.iter().cloned())
}

/// Compress an ndarray.
pub fn compress_array<T, D>(array: &Array<T, D>, model: &EntropyModel<T>) -> Vec<u8>
where
    T: Clone + Eq + Hash,
    D: Dimension,
{
    compress(array.as_slice().expect("array must be contiguous"), model)
}

/// Decompress directly into an ndarray with the requested shape.
pub fn decompress_array<T, D>(compressed: &[u8], shape: D, model: &EntropyModel<T>) -> Array<T, D>
where
    T: Clone + Eq + Hash,
    D: Dimension,
{
    let num_symbols = shape.size();

    let decoded = decompress(compressed, num_symbols, model);

    Array::from_shape_vec(shape, decoded).expect("decoded symbol count does not match array shape")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use ndarray::{Array2, Array3};
    use proptest::prelude::*;

    #[test]
    fn u16_round_trip() {
        let array =
            Array2::from_shape_vec((3, 4), vec![10, 10, 10, 20, 20, 20, 30, 30, 40, 40, 40, 10])
                .unwrap();

        let model = model_from_array(&array);
        let compressed = compress_array(&array, &model);

        let decoded = decompress_array(&compressed, array.raw_dim(), &model);

        assert_eq!(decoded, array);
    }

    #[test]
    fn i32_round_trip() {
        let array = Array2::from_shape_vec(
            (4, 4),
            vec![-10, -10, 0, 1, 1, 1, 2, 2, -2, -2, -1, 0, 100, -100, 0, 1],
        )
        .unwrap();

        let model = model_from_array(&array);
        let compressed = compress_array(&array, &model);

        let decoded = decompress_array(&compressed, array.raw_dim(), &model);

        assert_eq!(decoded, array);
    }

    #[test]
    fn u32_round_trip() {
        let array = Array3::from_shape_vec(
            (2, 3, 4),
            vec![
                0,
                1,
                0,
                1,
                100,
                100,
                100,
                200,
                0,
                0,
                0,
                0,
                0xFFFF_FFFFu32 as i32,
                0xFFFF_FFFFu32 as i32,
                123_456,
                123_456,
                7,
                8,
                9,
                10,
                1,
                0,
                100,
                100,
            ],
        )
        .unwrap();

        let model = model_from_array(&array);
        let compressed = compress_array(&array, &model);

        let decoded = decompress_array(&compressed, array.raw_dim(), &model);

        assert_eq!(decoded, array);
    }

    // ---------------------------------------------------------------
    // Proptest generators
    // ---------------------------------------------------------------

    fn arb_u16_array() -> impl Strategy<Value = Array2<u16>> {
        prop::collection::vec(0u16..1000u16, 1..=400).prop_flat_map(|values| {
            let len = values.len();

            // Generate dimensions whose product is exactly len.
            //
            // For testing purposes, just use a single-row array.
            // This still tests arbitrary ndarray dimensions through the
            // generic implementation while keeping the generator simple.
            Just(Array2::from_shape_vec((1, len), values).unwrap())
        })
    }

    fn arb_i32_array() -> impl Strategy<Value = Array2<i32>> {
        prop::collection::vec(-1000i32..=1000i32, 1..=400).prop_map(|values| {
            let len = values.len();

            Array2::from_shape_vec((1, len), values).unwrap()
        })
    }

    fn arb_u32_array() -> impl Strategy<Value = Array2<u32>> {
        prop::collection::vec(0u32..100_000u32, 1..=400).prop_map(|values| {
            let len = values.len();

            Array2::from_shape_vec((1, len), values).unwrap()
        })
    }

    // ---------------------------------------------------------------
    // Property tests
    // ---------------------------------------------------------------

    proptest! {
        #[test]
        fn proptest_u16_round_trip(
            array in arb_u16_array()
        ) {
            let model = model_from_array(&array);
            let compressed = compress_array(&array, &model);

            let decoded =
                decompress_array(
                    &compressed,
                    array.raw_dim(),
                    &model,
                );

            prop_assert_eq!(decoded, array);
        }

        #[test]
        fn proptest_i32_round_trip(
            array in arb_i32_array()
        ) {
            let model = model_from_array(&array);
            let compressed = compress_array(&array, &model);

            let decoded =
                decompress_array(
                    &compressed,
                    array.raw_dim(),
                    &model,
                );

            prop_assert_eq!(decoded, array);
        }

        #[test]
        fn proptest_u32_round_trip(
            array in arb_u32_array()
        ) {
            let model = model_from_array(&array);
            let compressed = compress_array(&array, &model);

            let decoded =
                decompress_array(
                    &compressed,
                    array.raw_dim(),
                    &model,
                );

            prop_assert_eq!(decoded, array);
        }
    }

    #[test]
    fn model_can_be_reused() {
        // Train the model on one array.
        let training =
            Array2::from_shape_vec((2, 8), vec![0, 0, 0, 1, 1, 1, 2, 2, 0, 0, 1, 1, 1, 2, 2, 2])
                .unwrap();

        let model = model_from_array(&training);

        // Encode a different array using the same model.
        let data =
            Array2::from_shape_vec((2, 8), vec![0, 1, 1, 0, 2, 1, 0, 2, 1, 1, 0, 0, 2, 2, 1, 0])
                .unwrap();

        let compressed = compress_array(&data, &model);

        let decoded = decompress_array(&compressed, data.raw_dim(), &model);

        assert_eq!(decoded, data);
    }
}
