use crate::decorrelation::bitplane::{from_bitplanes, to_bitplanes};
use crate::decorrelation::wavelet::{dwt53_nd, idwt53_nd};
use crate::entropy_coding::arithmetic::{compress_array, decompress_array, model_from_array};
use crate::entropy_coding::vrq::{vrq_decode, vrq_encode};
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
    let wavelet_none = benchmark_wavelet_none(data.clone())?;
    let waveletbp_none = benchmark_waveletbp_none(data.clone())?;
    let none_arithmetic = benchmark_none_arithmetic(data.clone())?;
    let wavelet_arithmetic = benchmark_wavelet_arithmetic(data.clone())?;
    let waveletbp_arithmetic = benchmark_waveletbp_arithmetic(data.clone())?;
    let none_vrq = benchmark_none_vrq(data.clone())?;
    let none_zst = benchmark_none_zst(data.clone())?;
    let wavelet_zst = benchmark_wavelet_zst(data.clone())?;
    let waveletbp_zst = benchmark_waveletbp_zst(data.clone())?;
    table
        .set_header(vec![
            "Decorrelation\nEntropy coding",
            "None",
            "Raster delta",
            "Time delta",
            "Wavelet",
            "Wavelet + bit plane",
        ])
        .add_row(vec![
            "None".to_string(),
            format_single_results(none_none, none_none),
            "".to_string(),
            "".to_string(),
            format_single_results(none_none, wavelet_none),
            format_single_results(none_none, waveletbp_none),
        ])
        .add_row(vec![
            "Arithmetic".to_string(),
            format_single_results(none_none, none_arithmetic),
            "".to_string(),
            "".to_string(),
            format_single_results(none_none, wavelet_arithmetic),
            format_single_results(none_none, waveletbp_arithmetic),
        ])
        .add_row(vec![
            "VRQ".to_string(),
            format_single_results(none_none, none_vrq),
            "".to_string(),
            "".to_string(),
            "-".to_string(),
            "-".to_string(),
        ])
        .add_row(vec![
            "ZST".to_string(),
            format_single_results(none_none, none_zst),
            "".to_string(),
            "".to_string(),
            format_single_results(none_none, wavelet_zst),
            format_single_results(none_none, waveletbp_zst),
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

#[allow(unused)]
fn benchmark_raster_none(data: Array3<u16>) -> anyhow::Result<usize> {
    todo!()
}

#[allow(unused)]
fn benchmark_time_none(data: Array3<u16>) -> anyhow::Result<usize> {
    todo!()
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

fn benchmark_none_zst(data: Array3<u16>) -> anyhow::Result<usize> {
    let bytes: Vec<u8> = data.iter().flat_map(|&value| value.to_le_bytes()).collect();

    let data_compressed = zstd::encode_all(&bytes[..], 3)?;
    let data_size = data_compressed.len() * size_of::<u8>();

    let decompressed = zstd::decode_all(&data_compressed[..])?;

    let values: Vec<u16> = decompressed
        .as_chunks::<2>().0.iter()
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
        .as_chunks::<4>().0.iter()
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
        .as_chunks::<4>().0.iter()
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
