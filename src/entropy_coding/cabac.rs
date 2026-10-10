
use std::collections::BTreeMap;
use std::hash::Hash;
use std::io::Cursor;

use cabac::{
    CabacReader, CabacWriter,
    fpaq0::{Fpaq0Decoder, Fpaq0Encoder},
    vp8::VP8Context,
};
use ndarray::{Array, Dimension};

/// A reusable symbol model.
///
/// Symbols are sorted, allowing each value to be mapped to an integer
/// before binary entropy coding. The model must be shared with the decoder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntropyModel<T> {
    symbols: Vec<T>,
}

impl<T> EntropyModel<T>
where
    T: Clone + Eq + Hash + Ord,
{
    pub fn from_symbols<I>(data: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut counts = BTreeMap::<T, usize>::new();

        for symbol in data {
            *counts.entry(symbol).or_insert(0) += 1;
        }

        assert!(
            !counts.is_empty(),
            "cannot construct an entropy model from empty data"
        );

        Self {
            symbols: counts.into_keys().collect(),
        }
    }

    pub fn symbols(&self) -> &[T] {
        &self.symbols
    }

    pub fn support_size(&self) -> usize {
        self.symbols.len()
    }
}

/// Number of bits required to represent a symbol index.
fn index_width(support_size: usize) -> usize {
    if support_size <= 1 {
        0
    } else {
        usize::BITS as usize - (support_size - 1).leading_zeros() as usize
    }
}

/// Compress symbols using FPAQ0's adaptive binary arithmetic coder.
///
/// The symbol model is external to the encoded payload.
pub fn compress<T>(data: &[T], model: &EntropyModel<T>) -> Vec<u8>
where
    T: Clone + Eq + Hash + Ord,
{
    assert!(!model.symbols.is_empty(), "empty entropy model");

    let width = index_width(model.support_size());
    let mut contexts = vec![VP8Context::default(); width];

    let mut output = Cursor::new(Vec::new());
    let mut encoder = Fpaq0Encoder::new(&mut output);

    for symbol in data {
        let index = model
            .symbols
            .binary_search(symbol)
            .expect("symbol was not present in entropy model");

        // Encode most-significant bit first.
        //
        // Each bit position has its own adaptive probability context.
        // Contexts are updated by the coder as bits are encoded.
        for (bit_position, context) in contexts.iter_mut().enumerate() {
            let shift = width - bit_position - 1;
            let bit = ((index >> shift) & 1) != 0;

            encoder
                .put(bit, context)
                .expect("FPAQ0 encoding failed");
        }
    }

    encoder.finish().expect("FPAQ0 finalization failed");
    output.into_inner()
}

/// Decompress a payload using the same model and expected symbol count.
pub fn decompress<T>(
    compressed: &[u8],
    num_symbols: usize,
    model: &EntropyModel<T>,
) -> Vec<T>
where
    T: Clone + Eq + Hash + Ord,
{
    assert!(!model.symbols.is_empty(), "empty entropy model");

    let width = index_width(model.support_size());
    let mut contexts = vec![VP8Context::default(); width];

    let mut input = Cursor::new(compressed);
    let mut decoder = Fpaq0Decoder::new(&mut input).unwrap();

    let mut decoded = Vec::with_capacity(num_symbols);

    for _ in 0..num_symbols {
        let mut index = 0usize;

        for context in contexts.iter_mut() {
            let bit = decoder
                .get(context)
                .expect("FPAQ0 decoding failed");

            index = (index << 1) | usize::from(bit);
        }

        assert!(
            index < model.support_size(),
            "decoded symbol index is outside the model"
        );

        decoded.push(model.symbols[index].clone());
    }

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

/// Compress a contiguous ndarray.
pub fn compress_array<T, D>(
    array: &Array<T, D>,
    model: &EntropyModel<T>,
) -> Vec<u8>
where
    T: Clone + Eq + Hash + Ord,
    D: Dimension,
{
    compress(
        array.as_slice().expect("array must be contiguous"),
        model,
    )
}

/// Decompress directly into an ndarray with the requested shape.
pub fn decompress_array<T, D>(
    compressed: &[u8],
    shape: D,
    model: &EntropyModel<T>,
) -> Array<T, D>
where
    T: Clone + Eq + Hash + Ord,
    D: Dimension,
{
    let decoded = decompress(compressed, shape.size(), model);

    Array::from_shape_vec(shape, decoded)
        .expect("decoded symbol count does not match array shape")
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array2;

    #[test]
    fn round_trip() {
        let array = Array2::from_shape_vec(
            (3, 4),
            vec![10u16, 10, 10, 20, 20, 20, 30, 30, 40, 40, 40, 10],
        )
        .unwrap();

        let model = model_from_array(&array);
        let compressed = compress_array(&array, &model);
        let decoded = decompress_array(&compressed, array.raw_dim(), &model);

        assert_eq!(decoded, array);
    }

    #[test]
    fn single_symbol_round_trip() {
        let array = Array2::from_elem((4, 4), 42u16);

        let model = model_from_array(&array);
        let compressed = compress_array(&array, &model);
        let decoded = decompress_array(&compressed, array.raw_dim(), &model);

        assert_eq!(decoded, array);
    }
}