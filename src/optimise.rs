// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Constrained least-cost search for the bridge configuration.
//!
//! **Objective:** minimise pathway-C all-in cost over the horizon.
//!
//! **Constraints** (a configuration is *feasible* only if all hold):
//! 1. every event passes the first-minutes test (`certified`),
//! 2. annual shortfall `< 0.5 MWh` (effectively zero penalties),
//! 3. SLA-breached energy `< sla_target x demand`.
//!
//! **Search space:** throttle-hold policy `{3, 4, 6, 12} h` (or the current
//! value when the policy search is disabled) x BESS duration `{1, 2, 4} h` x
//! BESS power from a coarse 0–32 MW grid (2 MW steps) refined to 0.5 MW
//! around the coarse optimum. Each evaluation is a full 8,760-hour simulation.
//! A candidate must beat the incumbent by more than $1 to replace it, which
//! makes the result deterministic under floating-point ties.

use crate::economics::economics;
use crate::error::Error;
use crate::params::Params;
use crate::sim::{run, simulate, simulate_interim, SimConfig, SimResult};
use crate::year::Year;

/// One evaluated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    /// BESS power (MW).
    pub bess_mw: f64,
    /// BESS duration (h).
    pub bess_h: f64,
    /// Throttle hold limit (h).
    pub max_throttle_h: f64,
    /// Meets all constraints.
    pub feasible: bool,
    /// Pathway-C all-in cost ($).
    pub total: f64,
    /// Pathway-C all-in $/MWh.
    pub per_mwh: f64,
    /// Annual shortfall (MWh).
    pub short_e: f64,
    /// SLA-breached share of compute.
    pub breach_pct: f64,
    /// BESS CapEx ($).
    pub capex: f64,
    /// All-in $/MWh reduction vs. waiting.
    pub reduction: f64,
}

/// Search output.
#[derive(Debug, Clone)]
pub struct Optimisation {
    /// Least-cost feasible configuration, if any exists in the search space.
    pub best: Option<Candidate>,
    /// Every configuration evaluated, in search order.
    pub evaluated: Vec<Candidate>,
}

/// Evaluate one configuration against shared, configuration-independent runs.
pub fn evaluate(
    base: &Params,
    year: &Year,
    firm: &SimResult,
    interim: &SimResult,
    bess_mw: f64,
    bess_h: f64,
    max_throttle_h: f64,
) -> Candidate {
    let mut q = base.clone();
    q.bess_mw = bess_mw;
    q.bess_h = bess_h;
    q.max_throttle_h = max_throttle_h;
    let flex = simulate(&q, year, &SimConfig::flex(&q, false));
    let fb = if q.effective().bess_mwh > 0.0 { Some(simulate(&q, year, &SimConfig::firm_bess(&q))) } else { None };
    let econ = economics(&q, interim, &flex, firm, fb.as_ref().unwrap_or(firm));
    let feasible = econ.certified && flex.short_e < 0.5 && flex.breach_pct < q.sla_target;
    Candidate {
        bess_mw,
        bess_h,
        max_throttle_h,
        feasible,
        total: econ.c.total,
        per_mwh: econ.c.per_mwh,
        short_e: flex.short_e,
        breach_pct: flex.breach_pct,
        capex: econ.capex,
        reduction: econ.reduction,
    }
}

fn better(c: &Candidate, best: Option<&Candidate>) -> bool {
    if !c.feasible {
        return false;
    }
    match best {
        None => true,
        Some(b) => c.total < b.total - 1.0,
    }
}

/// Run the search. `search_policy = false` keeps `max_throttle_h` fixed.
pub fn optimise(p: &Params, search_policy: bool) -> Result<Optimisation, Error> {
    let base = run(p, false)?;
    let policies: Vec<f64> = if search_policy { vec![3.0, 4.0, 6.0, 12.0] } else { vec![p.max_throttle_h] };
    let mut best: Option<Candidate> = None;
    let mut evaluated = Vec::new();
    for &mt in &policies {
        // The interim-only simulation depends on the hold policy whenever the effective ceiling is
        // below interim capacity, so it is recomputed per policy (identical across policies otherwise).
        let mut pi = p.clone();
        pi.max_throttle_h = mt;
        let interim = simulate_interim(&pi, &base.year);
        for h in [1.0, 2.0, 4.0] {
            let mut pair_best: Option<Candidate> = None;
            let mut mw = 0.0;
            while mw <= 32.0 {
                let c = evaluate(p, &base.year, &base.firm, &interim, mw, h, mt);
                if better(&c, pair_best.as_ref()) {
                    pair_best = Some(c.clone());
                }
                evaluated.push(c);
                mw += 2.0;
            }
            if let Some(centre) = pair_best.as_ref().map(|c| c.bess_mw) {
                let mut mw = (centre - 1.5).max(0.0);
                while mw <= centre + 1.5 {
                    if mw != centre {
                        let c = evaluate(p, &base.year, &base.firm, &interim, mw, h, mt);
                        if better(&c, pair_best.as_ref()) {
                            pair_best = Some(c.clone());
                        }
                        evaluated.push(c);
                    }
                    mw += 0.5;
                }
            }
            if let Some(pb) = pair_best {
                if better(&pb, best.as_ref()) {
                    best = Some(pb);
                }
            }
        }
    }
    Ok(Optimisation { best, evaluated })
}

/// BESS power frontier at the current duration and throttle policy (0–30 MW, 1 MW steps).
pub fn sweep(p: &Params) -> Result<Vec<Candidate>, Error> {
    let base = run(p, false)?;
    let mut out = Vec::with_capacity(31);
    for mw in 0..=30u32 {
        out.push(evaluate(p, &base.year, &base.firm, &base.interim, f64::from(mw), p.bess_h, p.max_throttle_h));
    }
    Ok(out)
}
