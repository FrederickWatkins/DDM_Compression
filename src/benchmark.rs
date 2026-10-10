use crate::decorrelation::bitplane::{from_bitplanes, to_bitplanes};
use crate::decorrelation::wavelet::{dwt53_axis, dwt53_nd, idwt53_axis, idwt53_nd};
use crate::entropy_coding::arithmetic::{compress_array, decompress_array, model_from_array};
use crate::entropy_coding::cabac::{
    compress_array as cabac_compress_array, decompress_array as cabac_decompress_array,
    model_from_array as cabac_model_from_array,
};
use crate::entropy_coding::vrq::{vrq_decode, vrq_encode};
use crate::decorrelation::raster_delta::{raster_code, inverse_raster_code};
use crate::decorrelation::time_delta::{time_code, inverse_time_code};
use anyhow::anyhow;
use comfy_table::Table;
use ndarray::Array3;

pub fn benchmark_all(data: &Vec<Array3<u16>>) -> anyhow::Result<Table> {
    match data.len() {
        0 => Err(anyhow!("At least one source is required for benchmarking")),
        1 => benchmark_single(data[0].clone()),
        _ => todo!(),
    }
}

pub fn benchmark_single(data: Array3<u16>) -> anyhow::Result<Table> {
    let mut table = Table::new();
    let none_none = benchmark_none_none(data.clone())?;
    let raster_none = benchmark_raster_none(data.clone())?;
    let time_none = benchmark_time_none(data.clone())?;
    let wavelet_none = benchmark_wavelet_none(data.clone())?;
    let waveletbp_none = benchmark_waveletbp_none(data.clone())?;
    let time_wavelet_none = benchmark_time_wavelet_none(data.clone())?;
    let none_arithmetic = benchmark_none_arithmetic(data.clone())?;
    let raster_arithmetic = benchmark_raster_arithmetic(data.clone())?;
    let time_arithmetic = benchmark_time_arithmetic(data.clone())?;
    let wavelet_arithmetic = benchmark_wavelet_arithmetic(data.clone())?;
    let waveletbp_arithmetic = benchmark_waveletbp_arithmetic(data.clone())?;
    let time_wavelet_arithmetic = benchmark_time_wavelet_arithmetic(data.clone())?;
    let none_vrq = benchmark_none_vrq(data.clone())?;
    let raster_vrq = benchmark_raster_vrq(data.clone())?;
    let time_vrq = benchmark_time_vrq(data.clone())?;
    let none_zst = benchmark_none_zst(data.clone())?;
    let raster_zst = benchmark_raster_zst(data.clone())?;
    let time_zst = benchmark_time_zst(data.clone())?;
    let wavelet_zst = benchmark_wavelet_zst(data.clone())?;
    let waveletbp_zst = benchmark_waveletbp_zst(data.clone())?;
    let time_wavelet_zst = benchmark_time_wavelet_zst(data.clone())?;
    let none_cabac = benchmark_none_cabac(data.clone())?;
    let raster_cabac = benchmark_raster_cabac(data.clone())?;
    let time_cabac = benchmark_time_cabac(data.clone())?;
    let wavelet_cabac = benchmark_wavelet_cabac(data.clone())?;
    let waveletbp_cabac = benchmark_waveletbp_cabac(data.clone())?;
    let time_wavelet_cabac = benchmark_time_wavelet_cabac(data.clone())?;
    table
        .set_header(vec![
            "Decorrelation\nEntropy coding",
            "None",
            "Raster delta",
            "Time delta",
            "Wavelet",
            "Wavelet + bit plane",
            "Time delta + wavelet",
        ])
        .add_row(vec![
            "None".to_string(),
            format_single_results(none_none, none_none),
            format_single_results(none_none, raster_none),
            format_single_results(none_none, time_none),
            format_single_results(none_none, wavelet_none),
            format_single_results(none_none, waveletbp_none),
            format_single_results(none_none, time_wavelet_none),
        ])
        .add_row(vec![
            "Arithmetic".to_string(),
            format_single_results(none_none, none_arithmetic),
            format_single_results(none_none, raster_arithmetic),
            format_single_results(none_none, time_arithmetic),
            format_single_results(none_none, wavelet_arithmetic),
            format_single_results(none_none, waveletbp_arithmetic),
            format_single_results(none_none, time_wavelet_arithmetic),
        ])
        .add_row(vec![
            "VRQ".to_string(),
            format_single_results(none_none, none_vrq),
            format_single_results(none_none, raster_vrq),
            format_single_results(none_none, time_vrq),
            "-".to_string(),
            "-".to_string(),
            "-".to_string(),
        ])
        .add_row(vec![
            "ZST".to_string(),
            format_single_results(none_none, none_zst),
            format_single_results(none_none, raster_zst),
            format_single_results(none_none, time_zst),
            format_single_results(none_none, wavelet_zst),
            format_single_results(none_none, waveletbp_zst),
            format_single_results(none_none, time_wavelet_zst),
        ])
        .add_row(vec![
            "CABAC".to_string(),
            format_single_results(none_none, none_cabac),
            format_single_results(none_none, raster_cabac),
            format_single_results(none_none, time_cabac),
            format_single_results(none_none, wavelet_cabac),
            format_single_results(none_none, waveletbp_cabac),
            format_single_results(none_none, time_wavelet_cabac),
        ]);
    Ok(table)
}

fn format_single_results(base_size: usize, compressed_size: usize) -> String {
    format!(
        "Bytes: {compressed_size}\nRatio: {:.2}",
        base_size as f32 / compressed_size as f32
    )
}

fn benchmark_none_none(data: Array3<u16>) -> anyhow::Result<usize> {
    Ok(data.len() * size_of::<u16>())
}

fn benchmark_raster_none(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = raster_code(&data);
    let data_size = encoded.len() * size_of::<i16>();

    let decoded = inverse_raster_code(&encoded);

    if decoded != data {
        Err(anyhow!("raster_none round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_none(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = time_code(&data);
    let data_size = encoded.len() * size_of::<i16>();

    let decoded = inverse_time_code(&encoded);

    if decoded != data {
        Err(anyhow!("time_none round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_wavelet_none(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;
    let data_size = data_converted.len() * size_of::<i32>();
    idwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;
    let data_reversed = data_converted.mapv(|x| x as u16);
    if data_reversed != data {
        Err(anyhow!("wavelet_none round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_waveletbp_none(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;
    let data_bitplaned = to_bitplanes(&data_converted);
    let data_size = data_bitplaned.len() * size_of::<u32>();
    let mut data_unbitplaned = from_bitplanes(&data_bitplaned, 52, 20);
    idwt53_nd(&mut data_unbitplaned).map_err(|e| anyhow!("{e:?}"))?;
    let data_reversed = data_unbitplaned
        .mapv(|x| x as u16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;
    if data_reversed != data {
        Err(anyhow!("waveletbp_none round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_none_arithmetic(data: Array3<u16>) -> anyhow::Result<usize> {
    let model = model_from_array(&data);
    let data_compressed = compress_array(&data, &model);
    let data_size = data_compressed.len() * size_of::<u8>();
    let data_decompressed = decompress_array(&data_compressed, data.raw_dim(), &model);
    if data_decompressed != data {
        Err(anyhow!("none_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_raster_arithmetic(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = raster_code(&data);
    let model = model_from_array(&encoded);
    let compressed = compress_array(&encoded, &model);
    let data_size = compressed.len();

    let decoded = decompress_array(&compressed, encoded.raw_dim(), &model);
    let reversed = inverse_raster_code(&decoded);

    if reversed != data {
        Err(anyhow!("raster_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_arithmetic(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = time_code(&data);
    let model = model_from_array(&encoded);
    let compressed = compress_array(&encoded, &model);
    let data_size = compressed.len();

    let decoded = decompress_array(&compressed, encoded.raw_dim(), &model);
    let reversed = inverse_time_code(&decoded);

    if reversed != data {
        Err(anyhow!("time_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_wavelet_arithmetic(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;
    let model = model_from_array(&data_converted);
    let data_compressed = compress_array(&data_converted, &model);
    let data_size = data_compressed.len() * size_of::<u8>();
    let mut data_decompressed =
        decompress_array(&data_compressed, data_converted.raw_dim(), &model);
    idwt53_nd(&mut data_decompressed).map_err(|e| anyhow!("{e:?}"))?;
    let data_reversed = data_decompressed.mapv(|x| x as u16);
    if data_reversed != data {
        Err(anyhow!("wavelet_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_waveletbp_arithmetic(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;

    let data_bitplaned = to_bitplanes(&data_converted);
    let model = model_from_array(&data_bitplaned);
    let data_compressed = compress_array(&data_bitplaned, &model);
    let data_size = data_compressed.len() * size_of::<u8>();

    let data_decompressed = decompress_array(&data_compressed, data_bitplaned.raw_dim(), &model);

    let mut data_unbitplaned = from_bitplanes(&data_decompressed, 52, 20);
    idwt53_nd(&mut data_unbitplaned).map_err(|e| anyhow!("{e:?}"))?;

    let data_reversed = data_unbitplaned
        .mapv(|x| x as u16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;

    if data_reversed != data {
        Err(anyhow!("waveletbp_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_none_vrq(data: Array3<u16>) -> anyhow::Result<usize> {
    let data_compressed = vrq_encode(data.mapv(|x| x as i16).view().into_dyn());
    let data_size = data_compressed.len() * size_of::<u8>();
    let data_decompressed = vrq_decode(&data_compressed, data.shape())
        .map_err(|e| anyhow!("{e}"))?
        .mapv(|x| x as u16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;
    if data_decompressed != data {
        Err(anyhow!("waveletbp_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_raster_vrq(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = raster_code(&data);
    let compressed = vrq_encode(encoded.view().into_dyn());
    let data_size = compressed.len();

    let decoded = vrq_decode(&compressed, encoded.shape())
        .map_err(|e| anyhow!("{e}"))?
        .mapv(|x| x as i16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;

    let reversed = inverse_raster_code(&decoded);
    if reversed != data {
        Err(anyhow!("raster_vrq round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_vrq(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = time_code(&data);
    let compressed = vrq_encode(encoded.view().into_dyn());
    let data_size = compressed.len();

    let decoded = vrq_decode(&compressed, encoded.shape())
        .map_err(|e| anyhow!("{e}"))?
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;

    let reversed = inverse_time_code(&decoded);
    if reversed != data {
        Err(anyhow!("time_vrq round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_none_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let bytes: Vec<u8> = data.iter().flat_map(|&value| value.to_le_bytes()).collect();

    let data_compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = data_compressed.len() * size_of::<u8>();

    let decompressed = zstd::decode_all(&data_compressed[..])?;

    let values: Vec<u16> = decompressed
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    let data_decompressed =
        Array3::from_shape_vec(data.dim(), values).map_err(|e| anyhow!("{e}"))?;

    if data_decompressed != data {
        Err(anyhow!("none_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_raster_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = raster_code(&data);
    let bytes: Vec<u8> = encoded.iter().flat_map(|&x| x.to_le_bytes()).collect();

    let compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = compressed.len();

    let decompressed = zstd::decode_all(&compressed[..])?;
    if decompressed.len() != encoded.len() * size_of::<i16>() {
        return Err(anyhow!("invalid raster_zst decompressed length"));
    }

    let values: Vec<i16> = decompressed
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| i16::from_le_bytes(*chunk))
        .collect();

    let decoded = Array3::from_shape_vec(encoded.dim(), values)
        .map_err(|e| anyhow!("{e}"))?;

    let reversed = inverse_raster_code(&decoded);
    if reversed != data {
        Err(anyhow!("raster_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = time_code(&data);
    let bytes: Vec<u8> = encoded.iter().flat_map(|&x| x.to_le_bytes()).collect();

    let compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = compressed.len();

    let decompressed = zstd::decode_all(&compressed[..])?;
    if decompressed.len() != encoded.len() * size_of::<i16>() {
        return Err(anyhow!("invalid time_zst decompressed length"));
    }

    let values: Vec<i16> = decompressed
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| i16::from_le_bytes(*chunk))
        .collect();

    let decoded = Array3::from_shape_vec(encoded.dim(), values)
        .map_err(|e| anyhow!("{e}"))?;

    let reversed = inverse_time_code(&decoded);
    if reversed != data {
        Err(anyhow!("time_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_wavelet_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;

    let bytes: Vec<u8> = data_converted
        .iter()
        .flat_map(|&value| value.to_le_bytes())
        .collect();

    let data_compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = data_compressed.len();

    let decompressed = zstd::decode_all(&data_compressed[..])?;

    let values: Vec<i32> = decompressed
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    let mut data_decompressed =
        Array3::from_shape_vec(data.dim(), values).map_err(|e| anyhow!("{e}"))?;

    idwt53_nd(&mut data_decompressed).map_err(|e| anyhow!("{e:?}"))?;

    let data_reversed = data_decompressed.mapv(|x| x as u16);

    if data_reversed != data {
        Err(anyhow!("wavelet_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_waveletbp_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut data_converted = data.mapv(|x| x as i32);
    dwt53_nd(&mut data_converted).map_err(|e| anyhow!("{e:?}"))?;

    let data_bitplaned = to_bitplanes(&data_converted);

    let bytes: Vec<u8> = data_bitplaned
        .iter()
        .flat_map(|&value| value.to_le_bytes())
        .collect();

    let data_compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = data_compressed.len();

    let decompressed = zstd::decode_all(&data_compressed[..])?;

    let values: Vec<u32> = decompressed
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();

    let data_decompressed = ndarray::ArrayD::from_shape_vec(data_bitplaned.raw_dim(), values)
        .map_err(|e| anyhow!("{e}"))?;

    let mut data_unbitplaned = from_bitplanes(&data_decompressed, 52, 20);
    idwt53_nd(&mut data_unbitplaned).map_err(|e| anyhow!("{e:?}"))?;

    let data_reversed = data_unbitplaned
        .mapv(|x| x as u16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;

    if data_reversed != data {
        Err(anyhow!("waveletbp_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_none_cabac(data: Array3<u16>) -> anyhow::Result<usize> {
    let model = cabac_model_from_array(&data);
    let compressed = cabac_compress_array(&data, &model);
    let compressed_size = compressed.len();

    let decompressed =
        cabac_decompress_array(&compressed, data.raw_dim(), &model);

    if decompressed != data {
        Err(anyhow!("none_cabac round trip does not match"))
    } else {
        Ok(compressed_size)
    }
}

fn benchmark_raster_cabac(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = raster_code(&data);
    let model = cabac_model_from_array(&encoded);
    let compressed = cabac_compress_array(&encoded, &model);
    let data_size = compressed.len();

    let decoded = cabac_decompress_array(&compressed, encoded.raw_dim(), &model);
    let reversed = inverse_raster_code(&decoded);

    if reversed != data {
        Err(anyhow!("raster_cabac round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_cabac(data: Array3<u16>) -> anyhow::Result<usize> {
    let encoded = time_code(&data);
    let model = cabac_model_from_array(&encoded);
    let compressed = cabac_compress_array(&encoded, &model);
    let data_size = compressed.len();

    let decoded = cabac_decompress_array(&compressed, encoded.raw_dim(), &model);
    let reversed = inverse_time_code(&decoded);

    if reversed != data {
        Err(anyhow!("time_cabac round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_wavelet_cabac(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut transformed = data.mapv(|x| x as i32);
    dwt53_nd(&mut transformed).map_err(|e| anyhow!("{e:?}"))?;

    let model = cabac_model_from_array(&transformed);
    let compressed = cabac_compress_array(&transformed, &model);
    let compressed_size = compressed.len();

    let mut decompressed =
        cabac_decompress_array(&compressed, transformed.raw_dim(), &model);

    idwt53_nd(&mut decompressed).map_err(|e| anyhow!("{e:?}"))?;

    let reversed = decompressed.mapv(|x| x as u16);

    if reversed != data {
        Err(anyhow!("wavelet_cabac round trip does not match"))
    } else {
        Ok(compressed_size)
    }
}

fn benchmark_waveletbp_cabac(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut transformed = data.mapv(|x| x as i32);
    dwt53_nd(&mut transformed).map_err(|e| anyhow!("{e:?}"))?;

    let bitplanes = to_bitplanes(&transformed);

    let model = cabac_model_from_array(&bitplanes);
    let compressed = cabac_compress_array(&bitplanes, &model);
    let compressed_size = compressed.len();

    let decompressed =
        cabac_decompress_array(&compressed, bitplanes.raw_dim(), &model);

    let mut unbitplaned = from_bitplanes(&decompressed, 52, 20);
    idwt53_nd(&mut unbitplaned).map_err(|e| anyhow!("{e:?}"))?;

    let reversed = unbitplaned
        .mapv(|x| x as u16)
        .into_dimensionality::<ndarray::Ix3>()
        .map_err(|e| anyhow!("{e}"))?;

    if reversed != data {
        Err(anyhow!("waveletbp_cabac round trip does not match"))
    } else {
        Ok(compressed_size)
    }
}

fn time_wavelet_code(data: &Array3<u16>) -> anyhow::Result<Array3<i32>> {
    let temporal = time_code(data);
    let mut transformed = temporal.mapv(|x| x as i32);

    dwt53_axis(&mut transformed, ndarray::Axis(1))
        .map_err(|e| anyhow!("{e:?}"))?;
    dwt53_axis(&mut transformed, ndarray::Axis(2))
        .map_err(|e| anyhow!("{e:?}"))?;

    Ok(transformed)
}

fn inverse_time_wavelet_code(
    transformed: &mut Array3<i32>,
) -> anyhow::Result<Array3<u16>> {
    idwt53_axis(transformed, ndarray::Axis(2))
        .map_err(|e| anyhow!("{e:?}"))?;
    idwt53_axis(transformed, ndarray::Axis(1))
        .map_err(|e| anyhow!("{e:?}"))?;

    let temporal = transformed.mapv(|x| x as i16);
    Ok(inverse_time_code(&temporal))
}

fn benchmark_time_wavelet_none(data: Array3<u16>) -> anyhow::Result<usize> {
    let mut transformed = time_wavelet_code(&data)?;
    let data_size = transformed.len() * size_of::<i32>();

    let reversed = inverse_time_wavelet_code(&mut transformed)?;

    if reversed != data {
        Err(anyhow!("time_wavelet_none round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_wavelet_arithmetic(
    data: Array3<u16>,
) -> anyhow::Result<usize> {
    let transformed = time_wavelet_code(&data)?;

    let model = model_from_array(&transformed);
    let compressed = compress_array(&transformed, &model);
    let data_size = compressed.len();

    let mut decompressed =
        decompress_array(&compressed, transformed.raw_dim(), &model);

    let reversed = inverse_time_wavelet_code(&mut decompressed)?;

    if reversed != data {
        Err(anyhow!("time_wavelet_arithmetic round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_wavelet_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let transformed = time_wavelet_code(&data)?;

    let bytes: Vec<u8> = transformed
        .iter()
        .flat_map(|&value| value.to_le_bytes())
        .collect();

    let compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = compressed.len();

    let decompressed = zstd::decode_all(&compressed[..])?;

    if decompressed.len() != transformed.len() * size_of::<i32>() {
        return Err(anyhow!("invalid time_wavelet_zst decompressed length"));
    }

    let values: Vec<i32> = decompressed
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| i32::from_le_bytes(*chunk))
        .collect();

    let mut decoded = Array3::from_shape_vec(transformed.dim(), values)
        .map_err(|e| anyhow!("{e}"))?;

    let reversed = inverse_time_wavelet_code(&mut decoded)?;

    if reversed != data {
        Err(anyhow!("time_wavelet_zst round trip does not match"))
    } else {
        Ok(data_size)
    }
}

fn benchmark_time_wavelet_cabac(
    data: Array3<u16>,
) -> anyhow::Result<usize> {
    let transformed = time_wavelet_code(&data)?;

    let model = cabac_model_from_array(&transformed);
    let compressed = cabac_compress_array(&transformed, &model);
    let data_size = compressed.len();

    let mut decompressed =
        cabac_decompress_array(&compressed, transformed.raw_dim(), &model);

    let reversed = inverse_time_wavelet_code(&mut decompressed)?;

    if reversed != data {
        Err(anyhow!("time_wavelet_cabac round trip does not match"))
    } else {
        Ok(data_size)
    }
}
