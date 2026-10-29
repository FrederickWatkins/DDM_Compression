#!/usr/bin/env python3
"""Plot benchmark results from a DuckDB file and list the best lossless configs.

Usage:
    python plot_benchmarks.py results.db [--epsilon 1e-9] [--out benchmarks.html]
                                         [--budget-db 0.25]

Dependencies:
    pip install --user duckdb pandas plotly
"""
import argparse

import duckdb
import numpy as np
import pandas as pd
import plotly.express as px
import plotly.graph_objects as go

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


DDM_METRICS = ["peak_power", "noise", "snr", "signal"]


def add_metric_errors(df: pd.DataFrame) -> pd.DataFrame:
    """Add err_<metric>_db columns and ddm_err_db (the worst of the four).

    Error is |10*log10(roundtripped / reference)|, i.e. the dB change caused by
    compression. All four are power-like quantities (SNR is a power ratio), so the
    same 10*log10 applies. Where the ratio isn't positive and finite (zero or
    sign-flipped values) the error is NaN/inf and the row fails any budget.
    """
    df = df.copy()
    errors = []
    for metric in DDM_METRICS:
        with np.errstate(divide="ignore", invalid="ignore"):
            err = (10.0 * np.log10(df[f"rt_{metric}"] / df[f"ref_{metric}"])).abs()
        df[f"err_{metric}_db"] = err
        errors.append(err)
    # skipna=False so an undefined metric makes the whole row fail the budget
    df["ddm_err_db"] = pd.concat(errors, axis=1).max(axis=1, skipna=False)
    return df


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
        + "<br>Worst DDM metric error: " + df["ddm_err_db"].round(3).astype(str) + " dB"
        + "<br>RT SNR: " + df["rt_snr"].round(3).astype(str)
        + " (ref " + df["ref_snr"].round(3).astype(str) + ")"
    )


def print_best_lossless(df: pd.DataFrame, epsilon: float, n: int = 10) -> None:
    lossless = df[df["mse"] < epsilon].sort_values("size")
    print(f"{len(lossless)} of {len(df)} configs are lossless (MSE < {epsilon:g})")
    if lossless.empty:
        return
    print(f"\nBest {min(n, len(lossless))} lossless configs by size:\n")
    columns = CONFIG_COLUMNS + ["size", "mse", "ddm_err_db"]
    with pd.option_context(
        "display.max_columns", None, "display.width", 250, "display.max_colwidth", None
    ):
        print(lossless[columns].head(n).to_string(index=False))


def pareto_front(df: pd.DataFrame) -> pd.DataFrame:
    """Rows not dominated on (size, mse), both to be minimised.

    A row is dominated if another row is no worse on both and strictly better on
    at least one. Rows tied exactly on (size, mse) are all kept. MSE is used rather
    than PSNR so lossless rows (infinite PSNR) are handled; PSNR is monotonic in MSE
    here, so the front is the same.
    """
    valid = df.dropna(subset=["size", "mse"])
    # At each size only the lowest-MSE rows can be non-dominated
    best_at_size = valid.groupby("size")["mse"].transform("min")
    candidates = valid[valid["mse"] == best_at_size].sort_values(["size", "mse"])

    keep = []
    best_mse = np.inf
    for size, group in candidates.groupby("size", sort=True):
        group_mse = group["mse"].iloc[0]
        # Must be strictly better than every smaller-size row
        if group_mse < best_mse:
            keep.append(group)
            best_mse = group_mse
    return pd.concat(keep) if keep else candidates.iloc[0:0]


def print_pareto(front: pd.DataFrame) -> None:
    print(f"\n{len(front)} Pareto-optimal configs (minimising size and MSE):\n")
    columns = CONFIG_COLUMNS + ["size", "mse", "psnr", "ddm_err_db"]
    with pd.option_context(
        "display.max_columns", None, "display.width", 250, "display.max_colwidth", None
    ):
        print(front[columns].to_string(index=False))


def plot(df: pd.DataFrame, pareto: pd.DataFrame, out: str) -> None:
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

    # Overlay the Pareto front (finite PSNR only, as lossless can't be drawn)
    front = pareto[np.isfinite(pareto["psnr"])].sort_values("size").copy()
    if not front.empty:
        front["config"] = config_text(front)
        fig.add_trace(
            go.Scattergl(
                x=front["size"],
                y=front["psnr"],
                mode="lines+markers",
                name="Pareto front",
                line={"color": "black", "width": 2},
                marker={"size": 8, "color": "black"},
                customdata=front[["config"]].to_numpy(),
                hovertemplate="%{customdata[0]}<extra>Pareto</extra>",
            )
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
    parser.add_argument(
        "--budget-db",
        type=float,
        default=None,
        help="Only keep configs where all four DDM metrics (peak power, noise, SNR, "
        "signal-only) change by at most this many dB from the original, e.g. 0.25",
    )
    args = parser.parse_args()

    df = add_metric_errors(load(args.db))
    if args.budget_db is not None:
        total = len(df)
        # NaN errors compare False, so undefined metrics are filtered out too
        df = df[df["ddm_err_db"] <= args.budget_db]
        print(f"{len(df)} of {total} configs within the {args.budget_db:g} dB DDM budget")
        if df.empty:
            return

    print_best_lossless(df, args.epsilon)
    pareto = pareto_front(df)
    print_pareto(pareto)
    plot(df, pareto, args.out)


if __name__ == "__main__":
    main()
