use std::{
    fmt::Display,
    io::{BufReader, BufWriter, Write as _},
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use color_eyre::eyre::{self, Context as _};
use ddm_app::{L0bFormatConfig, init_count_progress, load_l0b};
use itertools::Itertools;
use prodash::{Count as _, NestedProgress, Progress as _};

use crate::{
    L0bFileData,
    args::{BenchmarkL0bFormatConfig, BenchmarkParameter, Sweep},
};

pub fn benchmark<P>(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    sweep: &[Sweep],
    config: &[BenchmarkL0bFormatConfig],
    progress: &mut P,
) -> eyre::Result<()>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let mut load_progress =
        progress.add_child(format!("Loading {}", input.as_ref().to_string_lossy()));

    let reader =
        BufReader::new(std::fs::File::open(input.as_ref()).wrap_err_with(|| {
            format!("Failed to open file {}", input.as_ref().to_string_lossy())
        })?);
    let original_frames: L0bFileData = load_l0b(reader, &mut load_progress)?;

    let ranges: Vec<_> = sweep.iter().map(|sweep| sweep.range.clone()).collect();
    let config = config
        .iter()
        .enumerate()
        .map(|(i, c)| -> eyre::Result<BenchmarkL0bVars> {
            replace_parameters(c, sweep)
                .wrap_err_with(|| format!("error replacing parameters in config index={i}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut sweep_values = vec![vec![]; sweep.len()];
    let mut results = vec![vec![]; config.len()];

    let mut benchmark_progress = progress.add_child("Benchmarking configurations");
    let num_benchmarks = ranges
        .iter()
        .cloned()
        .map(|range| range.count())
        .product::<usize>()
        * config.len();
    benchmark_progress.init(Some(num_benchmarks), None);
    for values in ranges.iter().cloned().multi_cartesian_product() {
        for (sweep_value, value) in sweep_values.iter_mut().zip(&values) {
            sweep_value.push(*value);
        }
        for (config, column) in config.iter().zip(results.iter_mut()) {
            benchmark_progress.inc();
            let curr_config = substitute_vars(config, &values)
                .wrap_err_with(|| format!("value substitution failed on {values:#?}"))?;
            let mut config_progress =
                benchmark_progress.add_child(format!("Check config {curr_config}"));
            column.push(ddm_app::benchmark(
                &original_frames.frames,
                curr_config,
                &mut config_progress,
            )?);
            config_progress.done("Done".to_owned());
        }
    }
    let mut csv = BufWriter::new(std::fs::File::create(output.as_ref())?);
    for sweep_var in sweep {
        write!(csv, "{},", sweep_var.name)?;
    }
    for _ in config {
        write!(csv, "size,mse,rmse,psnr,")?;
    }
    writeln!(csv)?;
    for i in 0..sweep_values[0].len() {
        for sweep_var in &sweep_values {
            write!(csv, "{},", sweep_var[i])?;
        }
        for result in &results {
            write!(
                csv,
                "{},{},{},{},",
                result[i].0,
                result[i].1.mean.mse(),
                result[i].1.mean.rmse(),
                result[i].1.mean.psnr()
            )?;
        }
        writeln!(csv)?;
    }
    Ok(())
}

fn substitute_vars(
    benchmark_vars: &BenchmarkL0bVars,
    values: &[i64],
) -> eyre::Result<L0bFormatConfig> {
    match benchmark_vars {
        BenchmarkL0bVars::MP00 { quantisation } => Ok(L0bFormatConfig::MP00 {
            quantisation: substitute_var(quantisation, values)
                .try_into()
                .wrap_err_with(|| "quantisation does not fit into u8")?,
        }),
        BenchmarkL0bVars::MQ00 {
            quantisation,
            wavelet_stages,
            gop_length,
            dc_adjust,
        } => Ok(L0bFormatConfig::MQ00 {
            quantisation: substitute_var(quantisation, values)
                .try_into()
                .wrap_err_with(|| "quantisation does not fit into u8")?,
            dc_adjust: *dc_adjust,
            wavelet_stages: substitute_var(wavelet_stages, values)
                .try_into()
                .wrap_err_with(|| "wavelet_stages does not fit into u8")?,
            gop_length: substitute_var(gop_length, values)
                .try_into()
                .wrap_err_with(|| "gop_length does not fit into u8")?,
        }),
    }
}

fn substitute_var(var: &BenchmarkVariable, values: &[i64]) -> i64 {
    match var {
        BenchmarkVariable::Literal(value) => *value,
        BenchmarkVariable::SweepVariable(index) => values[*index],
    }
}

fn replace_parameters(
    config: &BenchmarkL0bFormatConfig,
    sweep: &[Sweep],
) -> eyre::Result<BenchmarkL0bVars> {
    match config {
        BenchmarkL0bFormatConfig::MP00 { quantisation } => Ok(BenchmarkL0bVars::MP00 {
            quantisation: replace_parameter(quantisation, sweep)?,
        }),
        BenchmarkL0bFormatConfig::MQ00 {
            quantisation,
            dc_adjust,
            wavelet_stages,
            gop_length,
        } => Ok(BenchmarkL0bVars::MQ00 {
            quantisation: replace_parameter(quantisation, sweep)?,
            dc_adjust: *dc_adjust,
            wavelet_stages: replace_parameter(wavelet_stages, sweep)?,
            gop_length: replace_parameter(gop_length, sweep)?,
        }),
    }
}

fn replace_parameter<T: Copy + Into<i64>>(
    parameter: &BenchmarkParameter<T>,
    sweep: &[Sweep],
) -> eyre::Result<BenchmarkVariable> {
    match parameter {
        BenchmarkParameter::Value(value) => Ok(BenchmarkVariable::Literal((*value).into())),
        BenchmarkParameter::Variable(name) => sweep
            .iter()
            .enumerate()
            .filter(|(_, sw)| *sw.name == *name.as_str())
            .exactly_one()
            .map(|(i, _)| BenchmarkVariable::SweepVariable(i))
            .map_err(|mut e| {
                if e.next().is_some() {
                    eyre::eyre!(format!("Variable name matched more than once: {name}"))
                } else {
                    eyre::eyre!(format!("No matches for variable name {name}"))
                }
            }),
    }
}

enum BenchmarkVariable {
    Literal(i64),
    SweepVariable(usize),
}

impl BenchmarkVariable {
    fn value(&self, sweep_vars: &[i64]) -> i64 {
        match self {
            BenchmarkVariable::Literal(value) => *value,
            BenchmarkVariable::SweepVariable(index) => sweep_vars[*index],
        }
    }
}

enum BenchmarkL0bVars {
    MP00 {
        quantisation: BenchmarkVariable,
    },
    MQ00 {
        quantisation: BenchmarkVariable,
        wavelet_stages: BenchmarkVariable,
        gop_length: BenchmarkVariable,
        dc_adjust: bool,
    },
}

fn cross_product(ranges: &[RangeInclusive<i64>]) -> impl Iterator<Item = Vec<i64>> + '_ {
    let mut state: Option<Vec<i64>> = if ranges.iter().any(|r| r.is_empty()) {
        None // any empty range makes the whole product empty
    } else {
        Some(ranges.iter().map(|r| *r.start()).collect())
    };

    std::iter::from_fn(move || {
        let out = state.clone()?;
        if !advance(state.as_mut().unwrap(), ranges) {
            state = None;
        }
        Some(out)
    })
}

fn advance(cur: &mut [i64], ranges: &[RangeInclusive<i64>]) -> bool {
    for (v, r) in cur.iter_mut().zip(ranges).rev() {
        if *v < *r.end() {
            *v += 1;
            return true;
        }
        *v = *r.start(); // wrap and carry to the previous position
    }
    false // every position wrapped, so we're done
}
