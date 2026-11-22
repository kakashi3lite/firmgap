// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Hourly dispatch engine and the four standard simulation configurations.
//!
//! ## Dispatch rules (compliance mode)
//!
//! For each curtailment-signal hour with load `L` above the ceiling `C`:
//!
//! 1. `gap = L - C`.
//! 2. In the event's first hour only, the first `ramp_min` minutes are a
//!    separate segment where throttling is limited to instant GPU
//!    power-capping (`fast_cap x L`). This segment is the witness test: any
//!    uncovered gap there fails the event's sub-5-minute check.
//! 3. Each segment covers the gap in strategy order (throttle then BESS, or
//!    BESS then throttle), then genset (until `gen_cap_h` run hours), and
//!    any remainder is **shortfall** billed at `penalty`.
//! 4. Throttled energy joins a FIFO backlog with deadline
//!    `t + sla_window + 1`; it is replayed only in hours with headroom, and
//!    expired backlog is an **SLA breach** (billed at base price when it is
//!    finally run).
//! 5. No new deferrals after `max_throttle_h` hours into an event.
//!
//! Outside signals, backlog is replayed first, then the BESS recharges from the
//! grid at `rte`. A BESS never charges during a signal.

use crate::economics::{economics, Economics};
use crate::error::Error;
use crate::invest::{investor, Investor};
use crate::params::{Params, Strategy};
use crate::year::{build_year, Year};
use crate::HOURS_PER_YEAR as HRS;

/// What the grid connection is constrained by in a simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Non-firm service: draw must stay under the ceiling during signals.
    Compliance,
    /// Firm service: BESS discharges during stress hours for price arbitrage.
    Arbitrage,
    /// Firm service with no flexible resources.
    Plain,
}

/// Physical configuration of one simulation run.
#[derive(Debug, Clone, Copy)]
pub struct SimConfig {
    /// Energized capacity (MW) that load scales with.
    pub cap: f64,
    /// Grid import limit used for BESS charging headroom (MW).
    pub grid_cap: f64,
    /// Grid ceiling during a curtailment signal (MW).
    pub ceiling: f64,
    /// BESS power (MW).
    pub bess_mw: f64,
    /// BESS energy (MWh).
    pub bess_mwh: f64,
    /// Genset capacity (MW).
    pub gen_mw: f64,
    /// Constraint mode.
    pub mode: Mode,
    /// Record hourly series.
    pub trace: bool,
}

impl SimConfig {
    /// Interim-only operation at `interim_mw` with no bridge assets (effective ceiling after deadband).
    pub fn interim(p: &Params) -> Self {
        let e = p.effective();
        Self {
            cap: p.interim_mw,
            grid_cap: p.interim_mw,
            ceiling: e.ceiling_mw,
            bess_mw: 0.0,
            bess_mwh: 0.0,
            gen_mw: 0.0,
            mode: Mode::Compliance,
            trace: false,
        }
    }
    /// Full campus on non-firm service with derated BESS, genset and throttle.
    pub fn flex(p: &Params, trace: bool) -> Self {
        let e = p.effective();
        Self {
            cap: p.target_mw,
            grid_cap: p.target_mw,
            ceiling: e.ceiling_mw,
            bess_mw: e.bess_mw,
            bess_mwh: e.bess_mwh,
            gen_mw: p.gen_mw,
            mode: Mode::Compliance,
            trace,
        }
    }
    /// Full campus on firm service, no bridge assets.
    pub fn firm(p: &Params) -> Self {
        Self {
            cap: p.target_mw,
            grid_cap: p.target_mw,
            ceiling: p.target_mw,
            bess_mw: 0.0,
            bess_mwh: 0.0,
            gen_mw: 0.0,
            mode: Mode::Plain,
            trace: false,
        }
    }
    /// Full campus on firm service, derated BESS kept for stress-hour arbitrage.
    pub fn firm_bess(p: &Params) -> Self {
        let e = p.effective();
        Self {
            cap: p.target_mw,
            grid_cap: p.target_mw,
            ceiling: p.target_mw,
            bess_mw: e.bess_mw,
            bess_mwh: e.bess_mwh,
            gen_mw: 0.0,
            mode: Mode::Arbitrage,
            trace: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Deferred {
    e: f64,
    dl: f64,
    ev: i32,
}

/// Per-event outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct EventOutcome {
    /// Event id.
    pub id: usize,
    /// First hour.
    pub start: usize,
    /// Duration (h).
    pub dur: usize,
    /// Largest hourly gap above the ceiling (MW).
    pub peak_gap: f64,
    /// Gap energy (MWh).
    pub gap_e: f64,
    /// Throttled (deferred) energy (MWh).
    pub thr: f64,
    /// BESS discharge (MWh).
    pub bes: f64,
    /// Genset output (MWh).
    pub gen: f64,
    /// Energy drawn above the ceiling (MWh).
    pub short: f64,
    /// Deferred energy from this event that missed its SLA deadline (MWh).
    pub breach: f64,
    /// Whether the first `ramp_min` minutes were fully covered.
    pub pass5: bool,
    /// Uncovered energy inside the ramp segment (MWh).
    pub ramp_short: f64,
}

/// Hourly series (only when `SimConfig::trace`).
#[derive(Debug, Clone, Default)]
pub struct Trace {
    /// Compute demand (MW).
    pub dem: Vec<f64>,
    /// Grid import (MW).
    pub grid: Vec<f64>,
    /// Throttled energy (MWh).
    pub thr: Vec<f64>,
    /// BESS discharge (MWh).
    pub bes: Vec<f64>,
    /// Genset output (MWh).
    pub gen: Vec<f64>,
    /// Shortfall (MWh).
    pub short: Vec<f64>,
    /// Backlog catch-up (MWh).
    pub cu: Vec<f64>,
    /// BESS charging from grid (MWh).
    pub chg: Vec<f64>,
    /// BESS state of charge at hour end (MWh).
    pub soc: Vec<f64>,
}

/// Annual result of one simulation.
#[derive(Debug, Clone)]
pub struct SimResult {
    /// Compute energy demanded (MWh/yr).
    pub demand: f64,
    /// Grid energy cost incl. late replay of breached work ($/yr).
    pub energy_cost: f64,
    /// Throttled energy (MWh/yr).
    pub thr_e: f64,
    /// BESS discharge (MWh/yr).
    pub bes_e: f64,
    /// Genset output (MWh/yr).
    pub gen_e: f64,
    /// Genset fuel + O&M ($/yr).
    pub gen_fuel: f64,
    /// Hours with genset output.
    pub gen_hrs: u32,
    /// Energy above the ceiling during signals (MWh/yr).
    pub short_e: f64,
    /// `short_e x penalty` ($/yr).
    pub penalty_cost: f64,
    /// Deferred energy that missed its deadline (MWh/yr).
    pub breach: f64,
    /// `breach / demand`.
    pub breach_pct: f64,
    /// `thr_e / demand`.
    pub defer_pct: f64,
    /// Backlog replayed within deadline (MWh/yr).
    pub catch_e: f64,
    /// Total gap above the ceiling (MWh/yr).
    pub gap_e: f64,
    /// Largest hourly gap (MW).
    pub max_gap: f64,
    /// Gap (MW) of the hardest first-hour ramp.
    pub worst_ramp_gap: f64,
    /// Lowest first-minutes coverage ratio across events (1.0 if none).
    pub worst_ramp_cover: f64,
    /// Events with a gap whose ramp segment was not fully covered.
    pub failed5: usize,
    /// `failed5 == 0`: the utility can certify the non-firm expansion.
    pub pass5_all: bool,
    /// Per-event detail.
    pub events: Vec<EventOutcome>,
    /// Hourly series when requested.
    pub trace: Option<Trace>,
}

fn replay(q: &mut [Deferred], qh: &mut usize, mut head: f64) -> f64 {
    let mut cu = 0.0;
    while head > 1e-9 && *qh < q.len() {
        let take = head.min(q[*qh].e);
        q[*qh].e -= take;
        head -= take;
        cu += take;
        if q[*qh].e <= 1e-9 {
            *qh += 1;
        }
    }
    cu
}

/// Simulate one year hour by hour.
pub fn simulate(p: &Params, y: &Year, c: &SimConfig) -> SimResult {
    let mut soc = c.bess_mwh;
    let (mut energy_cost, mut demand, mut thr_e, mut bes_e, mut gen_e, mut short_e, mut breach) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut catch_e, mut gap_e, mut max_gap, mut worst_ramp_gap) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut worst_ramp_cover = f64::INFINITY;
    let mut gen_hrs: u32 = 0;
    let mut q: Vec<Deferred> = Vec::new();
    let mut qh: usize = 0;
    let pw = c.bess_mw;
    let e_cap = c.bess_mwh;
    let rte = p.rte;
    let mut evs: Vec<EventOutcome> = y
        .events
        .iter()
        .map(|e| EventOutcome {
            id: e.id,
            start: e.start,
            dur: e.dur,
            peak_gap: 0.0,
            gap_e: 0.0,
            thr: 0.0,
            bes: 0.0,
            gen: 0.0,
            short: 0.0,
            breach: 0.0,
            pass5: true,
            ramp_short: 0.0,
        })
        .collect();
    let mut tr = if c.trace {
        Some(Trace {
            dem: Vec::with_capacity(HRS),
            grid: Vec::with_capacity(HRS),
            thr: Vec::with_capacity(HRS),
            bes: Vec::with_capacity(HRS),
            gen: Vec::with_capacity(HRS),
            short: Vec::with_capacity(HRS),
            cu: Vec::with_capacity(HRS),
            chg: Vec::with_capacity(HRS),
            soc: Vec::with_capacity(HRS),
        })
    } else {
        None
    };

    for t in 0..HRS {
        let tf = t as f64;
        let l = c.cap * y.u[t];
        demand += l;
        let id = y.ev[t];
        let stress = id >= 0;
        let price = if stress { p.stress_price } else { p.base_price };

        while qh < q.len() && q[qh].dl <= tf {
            let it = q[qh];
            qh += 1;
            breach += it.e;
            energy_cost += it.e * p.base_price;
            if it.ev >= 0 {
                evs[it.ev as usize].breach += it.e;
            }
        }

        let mut grid: f64;
        let (mut th, mut b, mut g, mut sh, mut cu, mut ch) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);

        if stress && c.mode == Mode::Compliance {
            let idu = id as usize;
            let kk = y.k[t];
            let gap = (l - c.ceiling).max(0.0);
            evs[idu].peak_gap = evs[idu].peak_gap.max(gap);
            evs[idu].gap_e += gap;
            gap_e += gap;
            max_gap = max_gap.max(gap);
            if gap > 1e-9 {
                let gen_p = if c.gen_mw > 0.0 && f64::from(gen_hrs) < p.gen_cap_h { c.gen_mw } else { 0.0 };
                let can_thr = f64::from(kk) <= p.max_throttle_h;
                let thr_p = if can_thr { p.defer_share * l } else { 0.0 };
                let fast_p = if can_thr { p.fast_cap.min(p.defer_share) * l } else { 0.0 };
                let mut segs: Vec<(f64, f64, bool)> = Vec::with_capacity(2);
                if kk == 1 {
                    let f = (p.ramp_min / 60.0).min(1.0);
                    segs.push((f, fast_p, true));
                    if f < 1.0 {
                        segs.push((1.0 - f, thr_p, false));
                    }
                } else {
                    segs.push((1.0, thr_p, false));
                }
                for &(d, tp, is_ramp) in &segs {
                    let mut rem = gap;
                    let b_avail = pw.min(soc / d);
                    let x: f64;
                    let yv: f64;
                    if p.strategy == Strategy::BessFirst {
                        yv = rem.min(b_avail);
                        rem -= yv;
                        x = rem.min(tp);
                        rem -= x;
                    } else {
                        x = rem.min(tp);
                        rem -= x;
                        yv = rem.min(b_avail);
                        rem -= yv;
                    }
                    let z = rem.min(gen_p);
                    rem -= z;
                    soc -= yv * d;
                    th += x * d;
                    b += yv * d;
                    g += z * d;
                    sh += rem * d;
                    if is_ramp {
                        let cover = ((gap - rem) / gap).min(1.0);
                        if gap > worst_ramp_gap {
                            worst_ramp_gap = gap;
                        }
                        worst_ramp_cover = worst_ramp_cover.min(cover);
                        if rem > 1e-6 {
                            evs[idu].pass5 = false;
                            evs[idu].ramp_short += rem * d;
                        }
                    }
                }
                if g > 0.0 {
                    gen_hrs += 1;
                }
                grid = l - th - b - g;
                if th > 0.0 {
                    q.push(Deferred { e: th, dl: tf + p.sla_window + 1.0, ev: id });
                }
                let s = &mut evs[idu];
                s.thr += th;
                s.bes += b;
                s.gen += g;
                s.short += sh;
            } else {
                let head = (c.cap.min(c.ceiling) - l).max(0.0);
                cu = replay(&mut q, &mut qh, head);
                grid = l + cu;
            }
        } else if stress && c.mode == Mode::Arbitrage && pw > 0.0 {
            b = pw.min(soc).min(l);
            soc -= b;
            grid = l - b;
        } else {
            let head = (c.cap - l).max(0.0);
            cu = replay(&mut q, &mut qh, head);
            grid = l + cu;
            if e_cap > 0.0 && soc < e_cap && !stress {
                let room = c.grid_cap - grid;
                ch = pw.min(room).min((e_cap - soc) / rte).max(0.0);
                soc += ch * rte;
                grid += ch;
            }
        }

        energy_cost += grid * price;
        thr_e += th;
        bes_e += b;
        gen_e += g;
        short_e += sh;
        catch_e += cu;
        if let Some(tr) = tr.as_mut() {
            tr.dem.push(l);
            tr.grid.push(grid);
            tr.thr.push(th);
            tr.bes.push(b);
            tr.gen.push(g);
            tr.short.push(sh);
            tr.cu.push(cu);
            tr.chg.push(ch);
            tr.soc.push(soc);
        }
    }

    let failed5 = evs.iter().filter(|e| e.peak_gap > 1e-9 && !e.pass5).count();
    SimResult {
        demand,
        energy_cost,
        thr_e,
        bes_e,
        gen_e,
        gen_fuel: gen_e * p.gen_cost,
        gen_hrs,
        short_e,
        penalty_cost: short_e * p.penalty,
        breach,
        breach_pct: if demand > 0.0 { breach / demand } else { 0.0 },
        defer_pct: if demand > 0.0 { thr_e / demand } else { 0.0 },
        catch_e,
        gap_e,
        max_gap,
        worst_ramp_gap,
        worst_ramp_cover: if worst_ramp_cover.is_infinite() { 1.0 } else { worst_ramp_cover },
        failed5,
        pass5_all: failed5 == 0,
        events: evs,
        trace: tr,
    }
}

/// Everything computed for one parameter set.
#[derive(Debug, Clone)]
pub struct Run {
    /// Inputs used.
    pub params: Params,
    /// Modelled year.
    pub year: Year,
    /// Interim-only operation.
    pub interim: SimResult,
    /// Full campus on non-firm service with the bridge.
    pub flex: SimResult,
    /// Firm service, no BESS.
    pub firm: SimResult,
    firm_bess: Option<SimResult>,
    /// Pathway economics (case KPIs).
    pub econ: Economics,
    /// Investor cash-flow view vs. interim-only (v0.2).
    pub inv: Investor,
}

impl Run {
    /// Firm service with the BESS retained for arbitrage (or `firm` if there is no BESS or it was skipped).
    pub fn firm_bess(&self) -> &SimResult {
        self.firm_bess.as_ref().unwrap_or(&self.firm)
    }

    /// Annual energy-cost saving from post-firm BESS arbitrage ($/yr).
    pub fn arbitrage_value(&self) -> f64 {
        self.firm.energy_cost - self.firm_bess().energy_cost
    }

    /// Compliant = certified, shortfall < 0.5 MWh/yr and SLA-late share below `sla_target`.
    pub fn compliant(&self) -> bool {
        is_compliant(&self.params, &self.econ, &self.flex)
    }
}

/// The compliance rule used everywhere (optimisers, Monte Carlo, stress suite, guardrails).
pub fn is_compliant(p: &Params, econ: &Economics, flex: &SimResult) -> bool {
    econ.certified && flex.short_e < 0.5 && flex.breach_pct < p.sla_target
}

/// How to assemble a [`Run`].
#[derive(Debug, Clone, Copy, Default)]
pub struct RunOptions {
    /// Record hourly series for the flex simulation.
    pub trace: bool,
    /// Do not simulate post-firm BESS arbitrage (treated as zero). Used by search loops for speed.
    pub skip_firm_bess: bool,
}

/// Validate inputs, build the year, run all four simulations, the economics and the investor view.
pub fn run(p: &Params, trace: bool) -> Result<Run, Error> {
    p.validate()?;
    let year = build_year(p);
    Ok(run_on_year(p, year, trace))
}

/// Same as [`run`] on a pre-built year (inputs are assumed validated).
pub fn run_on_year(p: &Params, year: Year, trace: bool) -> Run {
    run_cached(p, year, RunOptions { trace, skip_firm_bess: false }, None, None)
}

/// Assemble a run, optionally reusing configuration-independent simulations.
///
/// `firm` may be reused whenever `target_mw` and the year are unchanged; `interim` only when the
/// year, `interim_mw`, the effective ceiling and the throttle policy are unchanged.
pub fn run_cached(p: &Params, year: Year, opts: RunOptions, firm: Option<&SimResult>, interim: Option<&SimResult>) -> Run {
    let interim = match interim {
        Some(s) => s.clone(),
        None => simulate(p, &year, &SimConfig::interim(p)),
    };
    let flex = simulate(p, &year, &SimConfig::flex(p, opts.trace));
    let firm = match firm {
        Some(s) => s.clone(),
        None => simulate(p, &year, &SimConfig::firm(p)),
    };
    let firm_bess =
        if !opts.skip_firm_bess && p.effective().bess_mwh > 0.0 { Some(simulate(p, &year, &SimConfig::firm_bess(p))) } else { None };
    let fb = firm_bess.as_ref().unwrap_or(&firm);
    let econ = economics(p, &interim, &flex, &firm, fb);
    let inv = investor(p, &interim, &flex, &firm, fb, &econ);
    Run { params: p.clone(), year, interim, flex, firm, firm_bess, econ, inv }
}

/// Interim-only simulation on a given year (used by search loops that cache it per throttle policy).
pub fn simulate_interim(p: &Params, year: &Year) -> SimResult {
    simulate(p, year, &SimConfig::interim(p))
}

/// Re-price a run for parameters that do not change the hourly simulation (costs, finance, timing).
/// Bit-identical to a full re-run for those parameters, and far cheaper.
pub fn reprice(base: &Run, q: &Params) -> (Economics, Investor) {
    let fix = |s: &SimResult| {
        let mut c = s.clone();
        c.penalty_cost = c.short_e * q.penalty;
        c.gen_fuel = c.gen_e * q.gen_cost;
        c.trace = None;
        c
    };
    let interim = fix(&base.interim);
    let flex = fix(&base.flex);
    let fb = base.firm_bess();
    let econ = economics(q, &interim, &flex, &base.firm, fb);
    let inv = investor(q, &interim, &flex, &base.firm, fb, &econ);
    (econ, inv)
}

/// Parameters that [`reprice`] handles exactly.
pub const REPRICE_KEYS: &[&str] = &[
    "bess_cost",
    "bess_power_cost",
    "margin_pre_energy",
    "defer_cost",
    "sla_penalty",
    "salvage",
    "wacc",
    "carry",
    "penalty",
    "gen_cost",
    "gen_rental",
    "queue_months",
    "interim_month",
    "bess_lead",
    "horizon",
    "life",
    "sw_setup",
    "sw_price_mw_month",
    "sla_target",
    "mc_seeds",
];
