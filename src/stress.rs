// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Stress suite (port of `stressGen` in the web workbench): design x world matrix, one-at-a-time
//! NPV tornado, break-even and compliance-cliff thresholds, and compliance vs. curtailment intensity.

use crate::error::Error;
use crate::params::Params;
use crate::robust::seed_list;
use crate::sim::{is_compliant, reprice, run, run_cached, Run, RunOptions, REPRICE_KEYS};
use crate::worlds::{design_params, world_params, Design, DESIGNS, WORLDS};
use crate::year::build_year;

/// One matrix cell.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    /// World id.
    pub world: &'static str,
    /// NPV vs. interim-only ($).
    pub npv: f64,
    /// Compliant.
    pub compliant: bool,
    /// Certified (first-minutes test).
    pub certified: bool,
    /// Shortfall (MWh/yr).
    pub short: f64,
    /// BESS CapEx ($).
    pub capex: f64,
}

/// One matrix row (a design under every world).
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Design id (`current` for the input design).
    pub design: String,
    /// Display label.
    pub label: String,
    /// Cells in `WORLDS` order.
    pub cells: Vec<Cell>,
}

/// One side of a tornado bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Side {
    /// Perturbed value.
    pub v: f64,
    /// NPV ($).
    pub npv: f64,
    /// Compliant at that value.
    pub ok: bool,
}

/// One tornado driver.
#[derive(Debug, Clone, PartialEq)]
pub struct Tornado {
    /// Parameter key.
    pub key: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Low perturbation.
    pub lo: Side,
    /// High perturbation.
    pub hi: Side,
    /// |NPV(hi) − NPV(lo)|.
    pub swing: f64,
}

/// One threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct Threshold {
    /// Stable id.
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// `NPV = 0`, `KPI` or `compliance cliff`.
    pub kind: &'static str,
    /// Current value.
    pub current: Option<f64>,
    /// Threshold, or `None` (see `none`).
    pub value: Option<f64>,
    /// Text used when there is no threshold.
    pub none: &'static str,
}

/// Compliance share at one curtailment intensity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Intensity {
    /// Signal hours per year.
    pub stress_hours: f64,
    /// Share of compliant weather years.
    pub p_compliant: f64,
}

/// Full stress report.
#[derive(Debug, Clone)]
pub struct StressReport {
    /// Inputs.
    pub params: Params,
    /// Design x world matrix.
    pub matrix: Vec<Row>,
    /// Tornado, largest swing first.
    pub tornado: Vec<Tornado>,
    /// Break-evens and cliffs.
    pub thresholds: Vec<Threshold>,
    /// Compliance vs. intensity.
    pub intensity: Vec<Intensity>,
    /// Base NPV ($).
    pub base_npv: f64,
    /// Base scenario compliant.
    pub base_ok: bool,
}

type Bound = fn(f64, &Params) -> f64;

/// Tornado drivers: key, label, low and high perturbation.
pub const TORNADO_DRIVERS: &[(&str, &str, Bound, Bound)] = &[
    ("margin_pre_energy", "Gross margin $/MWh", |v: f64, _: &Params| v * 0.5, |v: f64, _: &Params| v * 1.5),
    ("bess_cost", "BESS installed $/kWh", |v: f64, _: &Params| v * 0.8, |v: f64, _: &Params| v * 1.4),
    ("queue_months", "Firm arrival month", |v: f64, p: &Params| (p.interim_month + 1.0).max(v - 14.0), |v: f64, _: &Params| v + 10.0),
    ("bess_lead", "Bridge build months", |v: f64, _: &Params| (v - 3.0).max(1.0), |v: f64, _: &Params| v + 6.0),
    ("stress_hours", "Signal hours / yr", |v: f64, _: &Params| (v * 0.67).round(), |v: f64, _: &Params| (v * 2.0).min(600.0)),
    ("ceiling_mw", "Signal ceiling MW", |v: f64, _: &Params| (v - 2.0).max(0.0), |v: f64, p: &Params| (v + 2.0).min(p.target_mw)),
    ("defer_share", "Deferrable share", |v: f64, _: &Params| (v - 0.05).max(0.0), |v: f64, _: &Params| (v + 0.05).min(0.6)),
    ("max_throttle_h", "Throttle hold h", |v: f64, _: &Params| (v - 4.0).max(1.0), |v: f64, _: &Params| (v + 4.0).min(24.0)),
    ("wacc", "WACC", |v: f64, _: &Params| (v - 0.03).max(0.01), |v: f64, _: &Params| v + 0.05),
    ("defer_cost", "Deferral fee $/MWh", |_: f64, _: &Params| 0.0, |v: f64, _: &Params| v + 200.0),
    ("safety_margin", "Control deadband MW", |_: f64, _: &Params| 0.0, |v: f64, _: &Params| v + 1.0),
    ("soc_window", "Usable SoC window", |v: f64, _: &Params| (v + 0.05).min(1.0), |v: f64, _: &Params| (v - 0.1).max(0.5)),
];

/// Signal-hour levels for the intensity curve.
pub const INTENSITY_LEVELS: [f64; 6] = [180.0, 240.0, 300.0, 360.0, 450.0, 600.0];
/// Weather years per intensity level.
pub const INTENSITY_SEEDS: usize = 6;

/// (npv, compliant) for a parameter set derived from `base`, re-pricing when only cost/timing keys changed.
fn evaluate(base: &Run, q: &Params, keys: &[&str]) -> Result<(f64, bool), Error> {
    if keys.iter().all(|k| REPRICE_KEYS.contains(k)) {
        let (econ, inv) = reprice(base, q);
        let ok = is_compliant(q, &econ, &base.flex);
        Ok((inv.npv, ok))
    } else {
        let r = run(q, false)?;
        Ok((r.inv.npv, r.compliant()))
    }
}

/// NPV with one field set directly (no validation: bisection deliberately probes beyond the input domain).
fn npv_with(base: &Run, p: &Params, key: &str, v: f64) -> Result<f64, Error> {
    let mut q = p.clone();
    q.set_unchecked(key, v);
    Ok(evaluate(base, &q, &[key])?.0)
}

fn bisect(base: &Run, p: &Params, key: &str, lo: f64, hi: f64) -> Result<f64, Error> {
    let (mut a, mut b) = (lo, hi);
    for _ in 0..50 {
        let m = (a + b) / 2.0;
        if npv_with(base, p, key, m)? > 0.0 {
            a = m;
        } else {
            b = m;
        }
    }
    Ok(a)
}

/// Scan from `from` toward `to` in `step` increments; return the last compliant value before the first
/// non-compliant one (`None` if every value is compliant).
fn cliff(p: &Params, key: &str, from: f64, to: f64, step: f64) -> Result<Option<f64>, Error> {
    let n = ((to - from).abs() / step.abs() + 1e-9).floor() as usize;
    let mut last = None;
    for j in 0..=n {
        let v = ((from + j as f64 * step) * 1e4).round() / 1e4;
        let mut q = p.clone();
        q.set_unchecked(key, v);
        q.validate()?;
        let r = run(&q, false)?;
        if !r.compliant() {
            return Ok(last);
        }
        last = Some(v);
    }
    Ok(None)
}

/// Run the full stress suite for `p`.
pub fn stress_suite(p: &Params) -> Result<StressReport, Error> {
    p.validate()?;
    let base = run(p, false)?;
    let base_npv = base.inv.npv;
    let base_ok = base.compliant();

    // 1. design x world matrix
    let current = Design {
        id: "current",
        label: "Current design",
        description: "",
        bess_mw: p.bess_mw,
        bess_h: p.bess_h,
        max_throttle_h: p.max_throttle_h,
        gen_mw: p.gen_mw,
        strategy: p.strategy,
    };
    let mut matrix = Vec::new();
    for d in std::iter::once(&current).chain(DESIGNS.iter()) {
        let dp = design_params(p, d)?;
        let mut cells = Vec::new();
        for w in WORLDS {
            let r = run(&world_params(&dp, w)?, false)?;
            cells.push(Cell {
                world: w.id,
                npv: r.inv.npv,
                compliant: r.compliant(),
                certified: r.econ.certified,
                short: r.flex.short_e,
                capex: r.econ.capex,
            });
        }
        matrix.push(Row { design: d.id.to_string(), label: d.label.to_string(), cells });
    }

    // 2. tornado
    let mut tornado = Vec::new();
    for &(key, label, lo, hi) in TORNADO_DRIVERS {
        let v0 = p.get(key).unwrap_or(f64::NAN);
        let mut sides = [Side { v: 0.0, npv: 0.0, ok: false }; 2];
        for (j, f) in [lo, hi].iter().enumerate() {
            let q = p.perturbed(key, f(v0, p));
            let (npv, ok) = evaluate(&base, &q, &[key])?;
            sides[j] = Side { v: q.get(key).unwrap_or(f64::NAN), npv, ok };
        }
        tornado.push(Tornado { key, label, lo: sides[0], hi: sides[1], swing: (sides[1].npv - sides[0].npv).abs() });
    }
    tornado.sort_by(|a, b| b.swing.total_cmp(&a.swing));

    // 3. thresholds
    let i0 = &base.inv;
    let mut th = vec![
        Threshold {
            id: "margin",
            label: "Gross margin before energy",
            kind: "NPV = 0",
            current: Some(p.margin_pre_energy),
            value: i0.break_even_margin,
            none: "n/a",
        },
        Threshold {
            id: "defer",
            label: "Deferral fee to workload owners",
            kind: "NPV = 0",
            current: Some(p.defer_cost),
            value: i0.break_even_defer_cost,
            none: "n/a",
        },
        Threshold {
            id: "sla",
            label: "SLA penalty on late work",
            kind: "NPV = 0",
            current: Some(p.sla_penalty),
            value: i0.break_even_sla_penalty,
            none: "n/a",
        },
    ];
    if base_npv > 0.0 && p.bess_mw > 0.0 {
        let hi_c = p.bess_cost * 40.0;
        let value =
            if npv_with(&base, p, "bess_cost", hi_c)? > 0.0 { None } else { Some(bisect(&base, p, "bess_cost", p.bess_cost, hi_c)?) };
        th.push(Threshold {
            id: "bessCost",
            label: "BESS installed cost",
            kind: "NPV = 0",
            current: Some(p.bess_cost),
            value,
            none: "> 40x current",
        });
    }
    if base_npv > 0.0 {
        let value = if npv_with(&base, p, "wacc", 3.0)? > 0.0 { None } else { Some(bisect(&base, p, "wacc", p.wacc, 3.0)?) };
        th.push(Threshold { id: "wacc", label: "Discount rate (WACC)", kind: "NPV = 0", current: Some(p.wacc), value, none: "> 300%" });
    }
    let mut q_be = None;
    if base_npv > 0.0 {
        let mut q = p.queue_months;
        while q >= p.interim_month + 1.0 {
            if npv_with(&base, p, "queue_months", q)? <= 0.0 {
                q_be = Some(q + 1.0);
                break;
            }
            q -= 1.0;
        }
    }
    th.push(Threshold {
        id: "queue",
        label: "Firm service arrival (queue compression)",
        kind: "NPV = 0",
        current: Some(p.queue_months),
        value: q_be,
        none: if base_npv > 0.0 { "positive for any firm date" } else { "already <= 0" },
    });
    let mut lead_be = None;
    if base_npv > 0.0 {
        let mut l = p.bess_lead;
        while l <= 36.0 {
            if npv_with(&base, p, "bess_lead", l)? <= 0.0 {
                lead_be = Some(l);
                break;
            }
            l += 1.0;
        }
    }
    th.push(Threshold {
        id: "lead",
        label: "BESS delivery + witness-test delay",
        kind: "NPV = 0",
        current: Some(p.bess_lead),
        value: lead_be,
        none: "positive up to 36 months",
    });
    let bc = base.econ.breakeven_carry;
    th.push(Threshold {
        id: "carry",
        label: "Carry for the case 25% KPI",
        kind: "KPI",
        current: Some(p.carry),
        value: if bc.is_finite() && bc > 0.0 { Some(bc) } else { None },
        none: "unreachable",
    });
    if base_ok {
        let cliffs: [(&'static str, &'static str, &'static str, f64, f64, f64); 5] = [
            ("ceiling", "ceiling_mw", "Signal ceiling (tariff tightening)", p.ceiling_mw, (p.ceiling_mw - 8.0).max(0.0), -0.5),
            ("deferShare", "defer_share", "Deferrable load share", p.defer_share, (p.defer_share - 0.2).max(0.0), -0.01),
            ("hold", "max_throttle_h", "Throttle hold limit", p.max_throttle_h, 1.0, -1.0),
            ("bessMW", "bess_mw", "BESS power", p.bess_mw, (p.bess_mw - 10.0).max(0.0), -0.5),
            ("deadband", "safety_margin", "Control deadband", p.safety_margin, p.safety_margin + 3.0, 0.25),
        ];
        for (id, key, label, from, to, step) in cliffs {
            let value = cliff(p, key, from, to, step)?;
            th.push(Threshold { id, label, kind: "compliance cliff", current: p.get(key), value, none: "no cliff within scan range" });
        }
    } else {
        th.push(Threshold {
            id: "cliffs",
            label: "Physical cliffs",
            kind: "compliance cliff",
            current: None,
            value: None,
            none: "scenario is not compliant; cliffs not applicable",
        });
    }

    // 4. intensity
    let mut intensity = Vec::new();
    for &sh in &INTENSITY_LEVELS {
        let mut ok_n = 0usize;
        for seed in seed_list(p.seed, INTENSITY_SEEDS) {
            let mut q = p.clone();
            q.stress_hours = sh;
            q.seed = seed;
            q.validate()?;
            let y = build_year(&q);
            let r = run_cached(&q, y, RunOptions { trace: false, skip_firm_bess: true }, None, None);
            if r.compliant() {
                ok_n += 1;
            }
        }
        intensity.push(Intensity { stress_hours: sh, p_compliant: ok_n as f64 / INTENSITY_SEEDS as f64 });
    }

    Ok(StressReport { params: p.clone(), matrix, tornado, thresholds: th, intensity, base_npv, base_ok })
}
