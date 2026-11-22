// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Weather-year robustness: Monte Carlo over synthetic congestion years and the robust optimiser.
//!
//! Both are exact ports of `monteCarlo` and `robustOptimiseGen` in the web reference
//! (`web/index.html`, FIRMGAP-CORE block) and are golden-tested against it.

use crate::error::Error;
use crate::params::Params;
use crate::sim::{is_compliant, run, run_cached, simulate_interim, RunOptions, SimResult};
use crate::year::{build_year, Year};

/// Seeds for `n` weather years derived from a base seed (golden-ratio stride, mod 2^32).
pub fn seed_list(base: u32, n: usize) -> Vec<u32> {
    (0..n).map(|i| base.wrapping_add((i as u32).wrapping_mul(2_654_435_761))).collect()
}

/// One weather year of a Monte Carlo run.
#[derive(Debug, Clone, PartialEq)]
pub struct McRow {
    /// Weather-year seed.
    pub seed: u32,
    /// Passed the first-minutes test.
    pub certified: bool,
    /// Certified, ~zero shortfall and SLA under target.
    pub compliant: bool,
    /// Shortfall (MWh/yr).
    pub short_e: f64,
    /// SLA-late share of compute.
    pub breach_pct: f64,
    /// Penalties ($/yr).
    pub penalty: f64,
    /// Investor NPV ($).
    pub npv: f64,
    /// Number of curtailment events.
    pub events: usize,
    /// Longest event (h).
    pub max_event: usize,
}

/// Monte Carlo summary.
#[derive(Debug, Clone)]
pub struct MonteCarlo {
    /// Weather years.
    pub n: usize,
    /// Per-year results in seed order.
    pub rows: Vec<McRow>,
    /// Share of compliant years.
    pub p_compliant: f64,
    /// Share of certifiable years.
    pub p_certified: f64,
    /// NPV P10 ($).
    pub npv_p10: f64,
    /// NPV P50 ($).
    pub npv_p50: f64,
    /// NPV P90 ($).
    pub npv_p90: f64,
    /// Shortfall P50 (MWh/yr).
    pub short_p50: f64,
    /// Shortfall P90 (MWh/yr).
    pub short_p90: f64,
    /// Mean penalty ($/yr).
    pub penalty_mean: f64,
}

/// Nearest-rank quantile on a sorted copy (index `round(x * (len - 1))`).
pub fn quantile(values: &[f64], x: f64) -> f64 {
    let mut s: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if s.is_empty() {
        return f64::NAN;
    }
    s.sort_by(|a, b| a.total_cmp(b));
    let idx = (x * (s.len() - 1) as f64).round().clamp(0.0, (s.len() - 1) as f64) as usize;
    s[idx]
}

/// Run the same design over `n` independent synthetic weather years.
pub fn monte_carlo(p: &Params, n: usize) -> Result<MonteCarlo, Error> {
    p.validate()?;
    let n = n.max(1);
    let mut rows = Vec::with_capacity(n);
    for seed in seed_list(p.seed, n) {
        let mut q = p.clone();
        q.seed = seed;
        let r = run(&q, false)?;
        rows.push(McRow {
            seed,
            certified: r.econ.certified,
            compliant: r.compliant(),
            short_e: r.flex.short_e,
            breach_pct: r.flex.breach_pct,
            penalty: r.flex.penalty_cost,
            npv: r.inv.npv,
            events: r.year.events.len(),
            max_event: r.year.events.iter().map(|e| e.dur).max().unwrap_or(0),
        });
    }
    let nf = n as f64;
    let npvs: Vec<f64> = rows.iter().map(|r| r.npv).collect();
    let shorts: Vec<f64> = rows.iter().map(|r| r.short_e).collect();
    let mut penalty_sum = 0.0;
    for r in &rows {
        penalty_sum += r.penalty;
    }
    Ok(MonteCarlo {
        n,
        p_compliant: rows.iter().filter(|r| r.compliant).count() as f64 / nf,
        p_certified: rows.iter().filter(|r| r.certified).count() as f64 / nf,
        npv_p10: quantile(&npvs, 0.1),
        npv_p50: quantile(&npvs, 0.5),
        npv_p90: quantile(&npvs, 0.9),
        short_p50: quantile(&shorts, 0.5),
        short_p90: quantile(&shorts, 0.9),
        penalty_mean: penalty_sum / nf,
        rows,
    })
}

/// Largest BESS power the robust optimiser will propose (MW).
pub const OPT_MAX_BESS_MW: f64 = 60.0;
/// Throttle-hold policies searched (only those up to the current hold are allowed).
pub const OPT_HOLDS: [f64; 7] = [3.0, 4.0, 6.0, 8.0, 12.0, 16.0, 24.0];
/// BESS durations searched (h).
pub const OPT_DURATIONS: [f64; 3] = [1.0, 2.0, 4.0];
/// Minimum share of compliant weather years for a robust design.
pub const ROBUST_THRESHOLD: f64 = 0.9;

/// One evaluated design on the base weather year.
#[derive(Debug, Clone, PartialEq)]
pub struct RobustCandidate {
    /// BESS power (MW).
    pub bess_mw: f64,
    /// BESS duration (h).
    pub bess_h: f64,
    /// Throttle hold (h).
    pub max_throttle_h: f64,
    /// Compliant on the base year.
    pub ok: bool,
    /// Ranking NPV (post-firm arbitrage excluded) ($).
    pub npv: f64,
    /// BESS CapEx ($).
    pub capex: f64,
}

/// A design verified across weather years.
#[derive(Debug, Clone, PartialEq)]
pub struct RobustDesign {
    /// BESS power (MW).
    pub bess_mw: f64,
    /// BESS duration (h).
    pub bess_h: f64,
    /// Throttle hold (h).
    pub max_throttle_h: f64,
    /// Full NPV incl. post-firm value ($).
    pub npv: f64,
    /// BESS CapEx ($).
    pub capex: f64,
    /// Share of compliant verification years.
    pub p_ok: f64,
    /// Verification years.
    pub n: usize,
    /// 0.5 MW steps added above the single-year optimum.
    pub bumps: u32,
}

/// Robust optimiser output.
#[derive(Debug, Clone)]
pub struct Robust {
    /// Best design on the base year (before robustness verification).
    pub best: Option<RobustCandidate>,
    /// Verified robust design.
    pub verified: Option<RobustDesign>,
    /// Configurations evaluated on the base year.
    pub evals: usize,
    /// Hold policies searched.
    pub holds: Vec<f64>,
}

fn eval_design(p: &Params, year: &Year, firm: &SimResult, interim: &SimResult, mw: f64, h: f64, mt: f64) -> RobustCandidate {
    let mut q = p.clone();
    q.bess_mw = mw;
    q.bess_h = h;
    q.max_throttle_h = mt;
    let r = run_cached(&q, year.clone(), RunOptions { trace: false, skip_firm_bess: true }, Some(firm), Some(interim));
    RobustCandidate { bess_mw: mw, bess_h: h, max_throttle_h: mt, ok: r.compliant(), npv: r.inv.npv, capex: r.econ.capex }
}

fn better(c: &RobustCandidate, best: Option<&RobustCandidate>) -> bool {
    if !c.ok {
        return false;
    }
    match best {
        None => true,
        Some(b) => c.npv > b.npv + 1.0,
    }
}

/// Maximise NPV subject to compliance on the base year, then add BESS power in 0.5 MW steps until the
/// design is compliant in at least 90% of `min(16, mc_seeds)` weather years.
pub fn robust_optimise(p: &Params) -> Result<Robust, Error> {
    p.validate()?;
    let mut holds: Vec<f64> = OPT_HOLDS.iter().copied().filter(|&h| h <= p.max_throttle_h).collect();
    holds.push(p.max_throttle_h);
    holds.sort_by(|a, b| a.total_cmp(b));
    holds.dedup();
    let base = run(p, false)?;
    let year = base.year.clone();
    let mut best: Option<RobustCandidate> = None;
    let mut evals = 0usize;
    for &mt in &holds {
        let mut pi = p.clone();
        pi.max_throttle_h = mt;
        let interim = simulate_interim(&pi, &year);
        for &h in &OPT_DURATIONS {
            let mut pb: Option<RobustCandidate> = None;
            for k in 0..=20u32 {
                let c = eval_design(p, &year, &base.firm, &interim, 2.0 * f64::from(k), h, mt);
                evals += 1;
                if better(&c, pb.as_ref()) {
                    pb = Some(c);
                }
            }
            if let Some(centre) = pb.as_ref().map(|c| c.bess_mw) {
                for k in -3i32..=3 {
                    if k == 0 {
                        continue;
                    }
                    let mw = centre + 0.5 * f64::from(k);
                    if mw < 0.0 {
                        continue;
                    }
                    let c = eval_design(p, &year, &base.firm, &interim, mw, h, mt);
                    evals += 1;
                    if better(&c, pb.as_ref()) {
                        pb = Some(c);
                    }
                }
            }
            if let Some(c) = pb {
                if better(&c, best.as_ref()) {
                    best = Some(c);
                }
            }
        }
    }
    let mut verified = None;
    if let Some(b) = best.as_ref() {
        let n = p.mc_years().min(16);
        let seeds = seed_list(p.seed, n);
        for bump in 0..=20u32 {
            let mw = b.bess_mw + f64::from(bump) * 0.5;
            if mw > OPT_MAX_BESS_MW {
                break;
            }
            let mut ok_n = 0usize;
            for &seed in &seeds {
                let mut q = p.clone();
                q.bess_mw = mw;
                q.bess_h = b.bess_h;
                q.max_throttle_h = b.max_throttle_h;
                q.seed = seed;
                let y = build_year(&q);
                let r = run_cached(&q, y, RunOptions { trace: false, skip_firm_bess: true }, None, None);
                if is_compliant(&q, &r.econ, &r.flex) {
                    ok_n += 1;
                }
            }
            let share = ok_n as f64 / n as f64;
            if share >= ROBUST_THRESHOLD {
                let mut q = p.clone();
                q.bess_mw = mw;
                q.bess_h = b.bess_h;
                q.max_throttle_h = b.max_throttle_h;
                let r = run(&q, false)?;
                verified = Some(RobustDesign {
                    bess_mw: mw,
                    bess_h: b.bess_h,
                    max_throttle_h: b.max_throttle_h,
                    npv: r.inv.npv,
                    capex: r.econ.capex,
                    p_ok: share,
                    n,
                    bumps: bump,
                });
                break;
            }
        }
    }
    Ok(Robust { best, verified, evals, holds })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_match_javascript_imul_stride() {
        assert_eq!(seed_list(2026, 5), vec![2026, 2654437787, 1013906252, 3668342013, 2027810478]);
    }

    #[test]
    fn quantile_nearest_rank() {
        let v = [5.0, 1.0, 3.0, 2.0, 4.0];
        assert_eq!(quantile(&v, 0.0), 1.0);
        assert_eq!(quantile(&v, 0.5), 3.0);
        assert_eq!(quantile(&v, 1.0), 5.0);
        assert!(quantile(&[], 0.5).is_nan());
    }
}
