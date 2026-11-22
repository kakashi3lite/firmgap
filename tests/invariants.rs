// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Physical and accounting invariants that must hold for any valid input.
//! These catch modelling regressions that golden values alone would miss.

use firmgap::sim::Mode;
use firmgap::{apply_design, apply_world, presets, run, Params};

const EPS: f64 = 1e-6;

fn scenarios() -> Vec<Params> {
    let mut out = Vec::new();
    for pr in presets::PRESETS {
        let mut p = Params::default();
        presets::apply(&mut p, pr.name).unwrap();
        out.push(p);
    }
    // Different weather years and an extreme but valid corner.
    for seed in [1u32, 42, 7777] {
        out.push(Params { seed, ..Params::default() });
    }
    out.push(Params {
        stress_hours: 400.0,
        avg_event_h: 8.0,
        ceiling_mw: 10.0,
        bess_mw: 30.0,
        bess_h: 6.0,
        gen_mw: 5.0,
        ..Params::default()
    });
    // Model 0.2 derated worlds.
    for (w, d) in [("base", "robust"), ("base", "eng"), ("harsh", "robust"), ("tariff", "robust"), ("workload", "diesel")] {
        out.push(apply_world(&apply_design(&Params::default(), d).unwrap(), w).unwrap());
    }
    out
}

#[test]
fn hourly_balance_and_ceiling_compliance() {
    for p in scenarios() {
        let r = run(&p, true).unwrap();
        let tr = r.flex.trace.as_ref().unwrap();
        let eff = p.effective();
        let soc_max = eff.bess_mwh;
        for t in 0..firmgap::HOURS_PER_YEAR {
            // Demand is met by grid (net of catch-up and charging) + throttle + BESS + genset.
            let lhs = tr.dem[t];
            let rhs = (tr.grid[t] - tr.cu[t] - tr.chg[t]) + tr.thr[t] + tr.bes[t] + tr.gen[t];
            assert!((lhs - rhs).abs() < EPS, "balance hour {t}: {lhs} vs {rhs}");
            // State of charge stays within physical limits.
            assert!((-EPS..=soc_max + EPS).contains(&tr.soc[t]), "soc hour {t}: {}", tr.soc[t]);
            // During a signal, grid draw above the ceiling is exactly the shortfall.
            if r.year.ev[t] >= 0 {
                let over = (tr.grid[t] - eff.ceiling_mw).max(0.0);
                assert!(over <= tr.short[t] + EPS, "hour {t}: over ceiling {over} > shortfall {}", tr.short[t]);
                assert!(tr.chg[t] == 0.0, "BESS charged during a signal at hour {t}");
            }
        }
    }
}

#[test]
fn deferred_work_is_conserved() {
    for p in scenarios() {
        let f = run(&p, false).unwrap().flex;
        // Every deferred MWh is either caught up, breached, or still queued at year end.
        assert!(f.catch_e + f.breach <= f.thr_e + EPS, "catch {} + breach {} > thr {}", f.catch_e, f.breach, f.thr_e);
        assert!(f.short_e <= f.gap_e + EPS);
        assert!((f.thr_e + f.bes_e + f.gen_e + f.short_e - f.gap_e).abs() < 1e-3, "gap not fully attributed");
    }
}

#[test]
fn pathways_are_ordered_by_energized_capacity() {
    for p in scenarios() {
        let e = run(&p, false).unwrap().econ;
        for m in 0..e.a.series.len() {
            assert!(e.a.series[m] <= e.b.series[m] + EPS);
            assert!(e.b.series[m] <= e.c.series[m] + EPS);
        }
        assert!(e.a.carry + EPS >= e.b.carry && e.b.carry + EPS >= e.c.carry);
        if !e.certified {
            assert_eq!(e.pull_forward_months, 0.0);
        }
    }
}

#[test]
fn deterministic_for_a_seed() {
    let p = Params::default();
    let a = run(&p, false).unwrap();
    let b = run(&p, false).unwrap();
    assert_eq!(a.flex.energy_cost.to_bits(), b.flex.energy_cost.to_bits());
    assert_eq!(a.econ.c.total.to_bits(), b.econ.c.total.to_bits());
}

#[test]
fn modes_are_wired() {
    use firmgap::sim::SimConfig;
    let p = Params::default();
    assert_eq!(SimConfig::flex(&p, false).mode, Mode::Compliance);
    assert_eq!(SimConfig::firm(&p).mode, Mode::Plain);
    assert_eq!(SimConfig::firm_bess(&p).mode, Mode::Arbitrage);
}

#[test]
fn investor_metamorphic_properties() {
    let p = apply_world(&apply_design(&Params::default(), "robust").unwrap(), "base").unwrap();
    let base = run(&p, false).unwrap();
    let with = |k: &str, v: f64| {
        let mut q = p.clone();
        q.set_f64(k, v).unwrap();
        run(&q, false).unwrap()
    };
    assert!(with("bess_cost", p.bess_cost * 1.2).inv.npv <= base.inv.npv);
    assert!(with("margin_pre_energy", p.margin_pre_energy * 1.2).inv.npv >= base.inv.npv);
    assert!(with("defer_cost", p.defer_cost + 100.0).inv.npv <= base.inv.npv);
    assert!(with("bess_mw", p.bess_mw + 2.0).flex.short_e <= base.flex.short_e + 1e-9);
    assert!(with("ceiling_mw", p.ceiling_mw - 1.0).flex.short_e >= base.flex.short_e - 1e-9);
    // re-pricing is exact for cost-only changes
    let mut q = p.clone();
    q.set_f64("bess_cost", 400.0).unwrap();
    q.set_f64("wacc", 0.12).unwrap();
    let (econ, inv) = firmgap::reprice(&base, &q);
    let full = run(&q, false).unwrap();
    assert_eq!(inv.npv.to_bits(), full.inv.npv.to_bits());
    assert_eq!(econ.c.total.to_bits(), full.econ.c.total.to_bits());
}

#[test]
fn downside_is_never_better_than_certified_bridge() {
    let p = apply_world(&apply_design(&Params::default(), "robust").unwrap(), "base").unwrap();
    let r = run(&p, false).unwrap();
    assert!(r.econ.certified);
    assert!(r.inv.npv_fail < r.inv.npv);
    assert!(r.inv.npv_fail >= -r.inv.capex_at_risk - 1e-6, "downside cannot lose more than the BESS capex");
}
