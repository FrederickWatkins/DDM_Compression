#!/usr/bin/env python3
"""Plot benchmark results from a DuckDB file and list the best lossless configs.

Usage:
    python plot_benchmarks.py results.db [--epsilon 1e-9] [--out benchmarks.html]

Dependencies:
    pip install --user duckdb pandas plotly
"""
import argparse

import duckdb
import numpy as np
import pandas as pd
import plotly.express as px

CONFIG_COLUMNS = [
    "dc_adjust",
    "wavelet_stages",
    "quantisation_kind",
    "quantisation_bits",
    "dc_sign_kind",
    "dc_bypass",
    "ac_sign_kind",
    "ac_bypass",
    "significant_encoding",
    "neighbour_encoding",
    "refinement_encoding",
]


def load(path: str) -> pd.DataFrame:
    # read_only so this works while nothing else is writing, and can't modify the file
    con = duckdb.connect(path, read_only=True)
    try:
        return con.sql("SELECT * FROM benchmarks").df()
    finally:
        con.close()


def config_text(df: pd.DataFrame) -> pd.Series:
    """One HTML-ish string per row, used for the hover tooltip."""
    return (
        "DC adjust: " + df["dc_adjust"].astype(str)
        + "<br>Stages: " + df["wavelet_stages"].astype(str)
        + "<br>Quantisation: " + df["quantisation_kind"]
        + " (" + df["quantisation_bits"].astype(str) + " bits)"
        + "<br>DC sign: " + df["dc_sign_kind"] + ", bypass=" + df["dc_bypass"].astype(str)
        + "<br>AC sign: " + df["ac_sign_kind"] + ", bypass=" + df["ac_bypass"].astype(str)
        + "<br>Significant: " + df["significant_encoding"]
        + "<br>Neighbour: " + df["neighbour_encoding"]
        + "<br>Refinement: " + df["refinement_encoding"]
        + "<br>Size: " + df["size"].astype(str) + " B"
        + "<br>PSNR: " + df["psnr"].round(2).astype(str) + " dB"
        + "<br>MSE: " + df["mse"].map("{:.4g}".format)
        + "<br>RT SNR: " + df["rt_snr"].round(3).astype(str)
        + " (ref " + df["ref_snr"].round(3).astype(str) + ")"
    )


def print_best_lossless(df: pd.DataFrame, epsilon: float, n: int = 10) -> None:
    lossless = df[df["mse"] < epsilon].sort_values("size")
    print(f"{len(lossless)} of {len(df)} configs are lossless (MSE < {epsilon:g})")
    if lossless.empty:
        return
    print(f"\nBest {min(n, len(lossless))} lossless configs by size:\n")
    columns = CONFIG_COLUMNS + ["size", "mse"]
    with pd.option_context(
        "display.max_columns", None, "display.width", 250, "display.max_colwidth", None
    ):
        print(lossless[columns].head(n).to_string(index=False))


def plot(df: pd.DataFrame, out: str) -> None:
    # Lossless configs have infinite PSNR, which can't be drawn
    finite = df[np.isfinite(df["psnr"])].copy()
    skipped = len(df) - len(finite)
    if skipped:
        print(f"\nNot plotted ({skipped} rows with infinite/NaN PSNR, i.e. lossless)")

    finite["config"] = config_text(finite)
    finite["stages"] = finite["wavelet_stages"].astype(str)

    fig = px.scatter(
        finite,
        x="size",
        y="psnr",
        color="stages",
        custom_data=["config"],
        render_mode="webgl",  # handles tens of thousands of points
        labels={"size": "Mean encoded size per frame (bytes)", "psnr": "PSNR (dB)"},
        title="Rate-distortion: size vs PSNR for all configs",
    )
    fig.update_traces(
        hovertemplate="%{customdata[0]}<extra></extra>",
        marker={"size": 5, "opacity": 0.7},
    )
    fig.write_html(out)
    print(f"\nWrote {out}")
    fig.show()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("db", help="DuckDB file written by the benchmark")
    parser.add_argument(
        "--epsilon",
        type=float,
        default=1e-9,
        help="MSE below this counts as lossless (default: 1e-9)",
    )
    parser.add_argument("--out", default="benchmarks.html", help="HTML output path")
    args = parser.parse_args()

    df = load(args.db)
    print_best_lossless(df, args.epsilon)
    plot(df, args.out)


if __name__ == "__main__":
    main()
