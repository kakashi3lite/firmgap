// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Golden parity tests for model 0.2 (derates, investor NPV, Monte Carlo, guardrails, robust
//! optimiser, stress suite). Expected values come from the web reference (`web/index.html`,
//! FIRMGAP-CORE block) via `node scripts/golden.mjs --write`, which writes `tests/golden_v2.json`;
//! this file is generated from that JSON by `scripts/gen_golden_v2.py`. Do not edit by hand.

use firmgap::guard::{codes, guardrails, verdict};
use firmgap::{apply_design, apply_world, monte_carlo, robust_optimise, run, stress_suite, Params, Strategy};

fn close(name: &str, got: f64, want: f64) {
    if want.is_nan() {
        assert!(got.is_nan(), "{name}: got {got}, want NaN/none");
        return;
    }
    if want.is_infinite() {
        assert!(got.is_infinite(), "{name}: got {got}, want infinite");
        return;
    }
    let tol = 1e-6 * want.abs().max(1.0);
    assert!((got - want).abs() <= tol, "{name}: got {got}, want {want} (tol {tol})");
}

fn close_opt(name: &str, got: Option<f64>, want: Option<f64>) {
    match (got, want) {
        (None, None) => {}
        (Some(g), Some(w)) => close(name, g, w),
        _ => panic!("{name}: got {got:?}, want {want:?}"),
    }
}

fn world_design(world: &str, design: &str) -> Params {
    apply_world(&apply_design(&Params::default(), design).unwrap(), world).unwrap()
}

fn assert_params(p: &Params, want: &[(&str, f64)], strategy: Strategy) {
    for (k, v) in want {
        let got = p.get(k).unwrap_or_else(|| panic!("unknown key {k}"));
        assert!((got - v).abs() <= 1e-12 * v.abs().max(1.0), "param {k}: got {got}, want {v}");
    }
    assert_eq!(p.strategy, strategy);
}

#[test]
fn v2_seed_list() {
    assert_eq!(firmgap::seed_list(2026, 5), vec![2026, 2654437787, 1013906252, 3668342013, 2027810478]);
}

#[test]
fn v2_base_robust() {
    let p = world_design("base", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 0.0);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030510420703473394);
    close("flex.thr_e", r.flex.thr_e, 2279.1276111602783);
    close("flex.energy_cost", r.flex.energy_cost, 21849186.210006095);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 12857500.0);
    close("econ.reduction", r.econ.reduction, 0.053427502804098914);
    close("econ.c.total", r.econ.c.total, 106530266.96486253);
    let i = &r.inv;
    close("inv.npv", i.npv, 66649148.34701935);
    close("inv.npv_fail", i.npv_fail, -11113119.915360477);
    assert_eq!(i.payback, Some(9));
    close_opt("inv.irr", i.irr, Some(3.462558748264504));
    close("inv.bridge_monthly", i.bridge_monthly, 3282267.367078833);
    close("inv.arb_monthly", i.arb_monthly, 26837.333333333645);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1380291.2668965082);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(17431.074697217573));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(105.19007654748921));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(14836.950179779935));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(35984.99664116611));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.015659295410447957));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6350077652867436));
    close("inv.capex_pv", i.capex_pv, 12493411.18225699);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "PENDING");
}

#[test]
fn v2_base_edge() {
    let p = world_design("base", "edge");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 13.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 11.283367622097334);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030510420703473394);
    close("flex.thr_e", r.flex.thr_e, 2279.1276111602783);
    close("flex.energy_cost", r.flex.energy_cost, 21851812.157379966);
    assert_eq!(r.flex.failed5, 12);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(!r.econ.certified);
    close("econ.full_month", r.econ.full_month, 38.0);
    close("econ.capex", r.econ.capex, 9382500.0);
    close("econ.reduction", r.econ.reduction, 0.08529630784963094);
    close("econ.c.total", r.econ.c.total, 75111698.12162553);
    let i = &r.inv;
    close("inv.npv", i.npv, -8109573.992290086);
    close("inv.npv_fail", i.npv_fail, -8109573.992290086);
    assert_eq!(i.payback, None);
    close_opt("inv.irr", i.irr, None);
    close("inv.bridge_monthly", i.bridge_monthly, 3281625.4118451816);
    close("inv.arb_monthly", i.arb_monthly, 19584.00000000031);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1007239.5731406995);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, None);
    close_opt("inv.break_even_margin", i.break_even_margin, None);
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, None);
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, None);
    close("inv.sw_pv", i.sw_pv, 0.0);
    close_opt("inv.sw_share", i.sw_share, None);
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.635523228453116));
    close("inv.capex_pv", i.capex_pv, 9116813.565430777);
    close("inv.pv_factor", i.pv_factor, 0.0);
    close("inv.bridge_months", i.bridge_months, 0.0);
    close("inv.mw_months", i.mw_months, 0.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:CERT", "v:NPV", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_base_eng() {
    let p = world_design("base", "eng");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 4.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 23.0),
            ("bess_h", 4.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 154.70910623669624);
    close("flex.breach_pct", r.flex.breach_pct, 0.0020799347700613136);
    close("flex.thr_e", r.flex.thr_e, 1559.7767682820559);
    close("flex.energy_cost", r.flex.energy_cost, 21891861.58254605);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 26220000.0);
    close("econ.reduction", r.econ.reduction, -0.021455852826879562);
    close("econ.c.total", r.econ.c.total, 114957876.98968866);
    let i = &r.inv;
    close("inv.npv", i.npv, 54671164.53498632);
    close("inv.npv_fail", i.npv_fail, -22904905.867885508);
    assert_eq!(i.payback, Some(14));
    close_opt("inv.irr", i.irr, Some(1.3989072338516855));
    close("inv.bridge_monthly", i.bridge_monthly, 3274408.14213929);
    close("inv.arb_monthly", i.arb_monthly, 50019.99409947358);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 2572616.2941816426);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(35899.59909588913));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(143.7950677528014));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(17778.425338725523));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(43299.578429312416));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.019024842129437124));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6413323032935727));
    close("inv.capex_pv", i.capex_pv, 25477522.16206714);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:SHORT", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_base_sw() {
    let p = world_design("base", "sw");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 0.0),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 461.37588000297546);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030510420703473394);
    close("flex.thr_e", r.flex.thr_e, 2279.1276111602783);
    close("flex.energy_cost", r.flex.energy_cost, 21956560.960261345);
    assert_eq!(r.flex.failed5, 51);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(!r.econ.certified);
    close("econ.full_month", r.econ.full_month, 38.0);
    close("econ.capex", r.econ.capex, 0.0);
    close("econ.reduction", r.econ.reduction, 0.08004947857787803);
    close("econ.c.total", r.econ.c.total, 75542546.12162553);
    let i = &r.inv;
    close("inv.npv", i.npv, 0.0);
    close("inv.npv_fail", i.npv_fail, 0.0);
    assert_eq!(i.payback, None);
    close_opt("inv.irr", i.irr, None);
    close("inv.bridge_monthly", i.bridge_monthly, 3256017.8757241173);
    close("inv.arb_monthly", i.arb_monthly, 0.0);
    assert_eq!(i.disposition.to_string(), "none");
    assert_eq!(i.fail_disposition.to_string(), "none");
    close("inv.post_pv", i.post_pv, 0.0);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, None);
    close_opt("inv.break_even_margin", i.break_even_margin, None);
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, None);
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, None);
    close("inv.sw_pv", i.sw_pv, 0.0);
    close_opt("inv.sw_share", i.sw_share, None);
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.656250773216828));
    close("inv.capex_pv", i.capex_pv, 0.0);
    close("inv.pv_factor", i.pv_factor, 0.0);
    close("inv.bridge_months", i.bridge_months, 0.0);
    close("inv.mw_months", i.mw_months, 0.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:CERT", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_base_diesel() {
    let p = world_design("base", "diesel");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 4.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 0.0),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 17.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 899.0721456855536);
    close("flex.breach_pct", r.flex.breach_pct, 0.0020799347700613136);
    close("flex.thr_e", r.flex.thr_e, 1559.7767682820559);
    close("flex.energy_cost", r.flex.energy_cost, 22043330.945307225);
    assert_eq!(r.flex.failed5, 37);
    assert_eq!(r.flex.gen_hrs, 50);
    assert!(!r.econ.certified);
    close("econ.full_month", r.econ.full_month, 38.0);
    close("econ.capex", r.econ.capex, 0.0);
    close("econ.reduction", r.econ.reduction, 0.08004947857787803);
    close("econ.c.total", r.econ.c.total, 75542546.12162553);
    let i = &r.inv;
    close("inv.npv", i.npv, 0.0);
    close("inv.npv_fail", i.npv_fail, 0.0);
    assert_eq!(i.payback, None);
    close_opt("inv.irr", i.irr, None);
    close("inv.bridge_monthly", i.bridge_monthly, 3134982.095647632);
    close("inv.arb_monthly", i.arb_monthly, 0.0);
    assert_eq!(i.disposition.to_string(), "none");
    assert_eq!(i.fail_disposition.to_string(), "none");
    close("inv.post_pv", i.post_pv, 0.0);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, None);
    close_opt("inv.break_even_margin", i.break_even_margin, None);
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, None);
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, None);
    close("inv.sw_pv", i.sw_pv, 0.0);
    close_opt("inv.sw_share", i.sw_share, None);
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.75880363463872));
    close("inv.capex_pv", i.capex_pv, 0.0);
    close("inv.pv_factor", i.pv_factor, 0.0);
    close("inv.bridge_months", i.bridge_months, 0.0);
    close("inv.mw_months", i.mw_months, 0.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:CERT", "w:HOLD", "w:DEFER", "w:PERMIT", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_harsh_robust() {
    let p = world_design("harsh", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 360.0),
            ("avg_event_h", 5.0),
            ("base_price", 68.0),
            ("stress_price", 600.0),
            ("penalty", 900.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 0.0);
    close("flex.breach_pct", r.flex.breach_pct, 0.007259051984689873);
    close("flex.thr_e", r.flex.thr_e, 4548.134711876512);
    close("flex.energy_cost", r.flex.energy_cost, 25033982.47634914);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 12857500.0);
    close("econ.reduction", r.econ.reduction, 0.0929809019709108);
    close("econ.c.total", r.econ.c.total, 123655058.53991169);
    let i = &r.inv;
    close("inv.npv", i.npv, 68987876.64146836);
    close("inv.npv_fail", i.npv_fail, -7579824.974657457);
    assert_eq!(i.payback, Some(9));
    close_opt("inv.irr", i.irr, Some(3.400563172951723));
    close("inv.bridge_monthly", i.bridge_monthly, 3231845.9067946444);
    close("inv.arb_monthly", i.arb_monthly, 95536.03219696849);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 4913586.207599528);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(12118.493525153477));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(97.65236513831888));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(7707.909886628711));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(15655.535591530373));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.015136471713540576));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6761176892180196));
    close("inv.capex_pv", i.capex_pv, 12493411.18225699);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "PENDING");
}

#[test]
fn v2_fasttrack_robust() {
    let p = world_design("fasttrack", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 18.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 0.0);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030510420703473394);
    close("flex.thr_e", r.flex.thr_e, 2279.1276111602783);
    close("flex.energy_cost", r.flex.energy_cost, 21849186.210006095);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 12857500.0);
    close("econ.reduction", r.econ.reduction, -0.044031347518995956);
    close("econ.c.total", r.econ.c.total, 107091726.35178927);
    let i = &r.inv;
    close("inv.npv", i.npv, 13162965.048237225);
    close("inv.npv_fail", i.npv_fail, -10672186.915714443);
    assert_eq!(i.payback, Some(9));
    close_opt("inv.irr", i.irr, Some(1.544341344227841));
    close("inv.bridge_monthly", i.bridge_monthly, 3282267.367078833);
    close("inv.arb_monthly", i.arb_monthly, 26837.333333333645);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1821224.2665425432);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(58188.40892759048));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(181.59095913966624));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(9568.822672175653));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(23186.307179925196));
    close("inv.sw_pv", i.sw_pv, 493452.6110441513);
    close_opt("inv.sw_share", i.sw_share, Some(0.0361333860281275));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6350077652867436));
    close("inv.capex_pv", i.capex_pv, 12493411.18225699);
    close("inv.pv_factor", i.pv_factor, 7.261794759018849);
    close("inv.bridge_months", i.bridge_months, 8.0);
    close("inv.mw_months", i.mw_months, 184.0);
    assert!(r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "PENDING");
}

#[test]
fn v2_crunch_robust() {
    let p = world_design("crunch", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 9.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 400.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.14),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 240.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 0.0);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030510420703473394);
    close("flex.thr_e", r.flex.thr_e, 2279.1276111602783);
    close("flex.energy_cost", r.flex.energy_cost, 21849186.210006095);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 13.0);
    close("econ.capex", r.econ.capex, 17112500.0);
    close("econ.reduction", r.econ.reduction, 0.0067727046348864395);
    close("econ.c.total", r.econ.c.total, 108542958.63036503);
    let i = &r.inv;
    close("inv.npv", i.npv, 27323308.63505175);
    close("inv.npv_fail", i.npv_fail, -15327559.60924633);
    assert_eq!(i.payback, Some(16));
    close_opt("inv.irr", i.irr, Some(1.0797540351675319));
    close("inv.bridge_monthly", i.bridge_monthly, 2234571.3672140953);
    close("inv.arb_monthly", i.arb_monthly, 26837.333333333645);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1053620.198168222);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(26814.238832702285));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(130.6913974546885));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(7562.238266743563));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(18311.396569689616));
    close("inv.sw_pv", i.sw_pv, 897811.3889598593);
    close_opt("inv.sw_share", i.sw_share, Some(0.031813457020698224));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(3.8704514552080336));
    close("inv.capex_pv", i.capex_pv, 16381179.807414547);
    close("inv.pv_factor", i.pv_factor, 19.08682303464407);
    close("inv.bridge_months", i.bridge_months, 25.0);
    close("inv.mw_months", i.mw_months, 575.0);
    assert!(r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "PENDING");
}

#[test]
fn v2_tariff_robust() {
    let p = world_design("tariff", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 18.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 1.0),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 25.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 309.0970895320176);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030521019776394483);
    close("flex.thr_e", r.flex.thr_e, 2281.9398070424795);
    close("flex.energy_cost", r.flex.energy_cost, 21732586.365026288);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 12857500.0);
    close("econ.reduction", r.econ.reduction, 0.053154536485503834);
    close("econ.c.total", r.econ.c.total, 106560987.45893826);
    let i = &r.inv;
    close("inv.npv", i.npv, 66518657.921446115);
    close("inv.npv_fail", i.npv_fail, -11113119.915360477);
    assert_eq!(i.payback, Some(9));
    close_opt("inv.irr", i.irr, Some(3.4554635321900014));
    close("inv.bridge_monthly", i.bridge_monthly, 3276759.4970649327);
    close("inv.arb_monthly", i.arb_monthly, 26837.333333333645);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1380291.2668965082);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(17857.707986952355));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(105.61064663763841));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(14789.73221849225));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(35902.07057635013));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.015689532465446535));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.639436921674271));
    close("inv.capex_pv", i.capex_pv, 12493411.18225699);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:SHORT", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_workload_robust() {
    let p = world_design("workload", "robust");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.25),
            ("fast_cap", 0.05),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 4.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 18.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.5),
            ("bess_derate", 0.95),
            ("soc_window", 0.9),
            ("defer_cost", 100.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 722.4336778044702);
    close("flex.breach_pct", r.flex.breach_pct, 0.0012427921474644668);
    close("flex.thr_e", r.flex.thr_e, 1111.405998927355);
    close("flex.energy_cost", r.flex.energy_cost, 22028144.193608586);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 12857500.0);
    close("econ.reduction", r.econ.reduction, 0.04297707726739508);
    close("econ.c.total", r.econ.c.total, 107706390.95496307);
    let i = &r.inv;
    close("inv.npv", i.npv, 65547061.93451532);
    close("inv.npv_fail", i.npv_fail, -11113119.915360477);
    assert_eq!(i.payback, Some(9));
    close_opt("inv.irr", i.irr, Some(3.4027065595038497));
    close("inv.bridge_monthly", i.bridge_monthly, 3235749.4047264806);
    close("inv.arb_monthly", i.arb_monthly, 26837.333333333645);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1380291.2668965082);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(18593.759788546537));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(108.74209639739672));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(29972.170128360736));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(86881.99556267133));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.015918394417992997));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6728893119370243));
    close("inv.capex_pv", i.capex_pv, 12493411.18225699);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:SHORT", "w:HOLD", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_stated_eng() {
    let p = world_design("stated", "eng");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 4.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 23.0),
            ("bess_h", 4.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.0),
            ("bess_derate", 1.0),
            ("soc_window", 1.0),
            ("defer_cost", 0.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 91.95314557850361);
    close("flex.breach_pct", r.flex.breach_pct, 0.0020799347700613136);
    close("flex.thr_e", r.flex.thr_e, 1559.7767682820559);
    close("flex.energy_cost", r.flex.energy_cost, 21898202.013520163);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 26220000.0);
    close("econ.reduction", r.econ.reduction, -0.020160366927152618);
    close("econ.c.total", r.econ.c.total, 114812078.90327059);
    let i = &r.inv;
    close("inv.npv", i.npv, 55012775.28307322);
    close("inv.npv_fail", i.npv_fail, -22683518.63946559);
    assert_eq!(i.payback, Some(13));
    close_opt("inv.irr", i.irr, Some(1.4022227601864587));
    close("inv.bridge_monthly", i.bridge_monthly, 3279482.6563500506);
    close("inv.arb_monthly", i.arb_monthly, 54324.4789479555);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 2794003.522601549);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(35349.709845123965));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(142.69405775541884));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(17864.35695985801));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(43570.13424250748));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.018908938148625553));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.6372452323397284));
    close("inv.capex_pv", i.capex_pv, 25477522.16206714);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(!r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["v:SHORT", "w:HOLD", "w:DEFER", "w:FREE", "w:DEADBAND", "w:SOC", "w:AVAIL", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_stated_edge() {
    let p = world_design("stated", "edge");
    assert_params(
        &p,
        &[
            ("queue_months", 38.0),
            ("interim_month", 4.0),
            ("interim_mw", 22.0),
            ("ceiling_mw", 22.0),
            ("bess_lead", 6.0),
            ("horizon", 60.0),
            ("target_mw", 45.0),
            ("util", 0.78),
            ("defer_share", 0.35),
            ("fast_cap", 0.1),
            ("ramp_min", 4.0),
            ("sla_window", 4.0),
            ("max_throttle_h", 12.0),
            ("stress_hours", 180.0),
            ("avg_event_h", 3.5),
            ("base_price", 68.0),
            ("stress_price", 310.0),
            ("penalty", 450.0),
            ("bess_mw", 13.5),
            ("bess_h", 2.0),
            ("bess_cost", 285.0),
            ("bess_power_cost", 250.0),
            ("rte", 0.88),
            ("gen_mw", 0.0),
            ("gen_cap_h", 50.0),
            ("gen_cost", 290.0),
            ("gen_rental", 65.0),
            ("carry", 145000.0),
            ("wacc", 0.09),
            ("life", 10.0),
            ("sla_target", 0.015),
            ("safety_margin", 0.0),
            ("bess_derate", 1.0),
            ("soc_window", 1.0),
            ("defer_cost", 0.0),
            ("sla_penalty", 0.0),
            ("salvage", 0.0),
            ("margin_pre_energy", 320.0),
            ("sw_setup", 250000.0),
            ("sw_price_mw_month", 1500.0),
            ("mc_seeds", 24.0),
            ("seed", 2026.0),
        ],
        Strategy::ThrottleFirst,
    );
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, 0.18643172085285187);
    close("flex.breach_pct", r.flex.breach_pct, 0.0030463907063872893);
    close("flex.thr_e", r.flex.thr_e, 2275.6974283754826);
    close("flex.energy_cost", r.flex.energy_cost, 21870206.859446965);
    assert_eq!(r.flex.failed5, 0);
    assert_eq!(r.flex.gen_hrs, 0);
    assert!(r.econ.certified);
    close("econ.full_month", r.econ.full_month, 10.0);
    close("econ.capex", r.econ.capex, 9382500.0);
    close("econ.reduction", r.econ.reduction, 0.07198749033816132);
    close("econ.c.total", r.econ.c.total, 104441467.18172349);
    let i = &r.inv;
    close("inv.npv", i.npv, 69820473.24584651);
    close("inv.npv_fail", i.npv_fail, -8012620.450490419);
    assert_eq!(i.payback, Some(8));
    close_opt("inv.irr", i.irr, Some(5.091642580916755));
    close("inv.bridge_monthly", i.bridge_monthly, 3285256.8376258123);
    close("inv.arb_monthly", i.arb_monthly, 21469.090909091447);
    assert_eq!(i.disposition.to_string(), "retain for post-firm arbitrage");
    assert_eq!(i.fail_disposition.to_string(), "retain for post-firm arbitrage");
    close("inv.post_pv", i.post_pv, 1104193.1149403488);
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, Some(12442.214414868204));
    close_opt("inv.break_even_margin", i.break_even_margin, Some(94.96890980109418));
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, Some(15540.12651947073));
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, Some(37754.806139920445));
    close("inv.sw_pv", i.sw_pv, 1060281.9714297357);
    close_opt("inv.sw_share", i.sw_share, Some(0.01495867204263797));
    close_opt("inv.sw_payback_days", i.sw_payback_days, Some(2.632609998994876));
    close("inv.capex_pv", i.capex_pv, 9116813.565430777);
    close("inv.pv_factor", i.pv_factor, 23.69163129193434);
    close("inv.bridge_months", i.bridge_months, 28.0);
    close("inv.mw_months", i.mw_months, 644.0);
    assert!(r.compliant());
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "w:FREE", "w:DEADBAND", "w:SOC", "w:AVAIL", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, false).to_string(), "PENDING");
}

#[test]
fn v2_monte_carlo_base_robust() {
    let p = world_design("base", "robust");
    let mc = monte_carlo(&p, 24).unwrap();
    close("p_compliant", mc.p_compliant, 1.0);
    close("p_certified", mc.p_certified, 1.0);
    close("npv_p10", mc.npv_p10, 66595505.48160707);
    close("npv_p50", mc.npv_p50, 66694853.04467521);
    close("npv_p90", mc.npv_p90, 66772119.845648125);
    close("short_p50", mc.short_p50, 0.0);
    close("short_p90", mc.short_p90, 0.0);
    close("penalty_mean", mc.penalty_mean, 0.0);
    let r = run(&p, false).unwrap();
    let g = guardrails(&p, &r, Some(&mc));
    assert_eq!(codes(&g), vec!["w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, true).to_string(), "CONDITIONAL");
}

#[test]
fn v2_monte_carlo_base_eng() {
    let p = world_design("base", "eng");
    let mc = monte_carlo(&p, 24).unwrap();
    close("p_compliant", mc.p_compliant, 0.0);
    close("p_certified", mc.p_certified, 1.0);
    close("npv_p10", mc.npv_p10, 54651540.47143313);
    close("npv_p50", mc.npv_p50, 54999315.18998807);
    close("npv_p90", mc.npv_p90, 55240404.58178729);
    close("short_p50", mc.short_p50, 84.34919819831849);
    close("short_p90", mc.short_p90, 145.06971478164198);
    close("penalty_mean", mc.penalty_mean, 35178.76625256613);
    let r = run(&p, false).unwrap();
    let g = guardrails(&p, &r, Some(&mc));
    assert_eq!(codes(&g), vec!["v:SHORT", "v:MC", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, true).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_monte_carlo_base_edge() {
    let p = world_design("base", "edge");
    let mc = monte_carlo(&p, 24).unwrap();
    close("p_compliant", mc.p_compliant, 0.0);
    close("p_certified", mc.p_certified, 0.0);
    close("npv_p10", mc.npv_p10, -8143911.705010783);
    close("npv_p50", mc.npv_p50, -8075236.279569351);
    close("npv_p90", mc.npv_p90, -8029452.662608372);
    close("short_p50", mc.short_p50, 5.546553958654403);
    close("short_p90", mc.short_p90, 13.756643955707544);
    close("penalty_mean", mc.penalty_mean, 2848.578975141048);
    let r = run(&p, false).unwrap();
    let g = guardrails(&p, &r, Some(&mc));
    assert_eq!(codes(&g), vec!["v:CERT", "v:NPV", "v:MC", "v:P10", "w:HOLD", "w:DEFER", "i:KPI", "i:MARGIN"]);
    assert_eq!(verdict(&g, true).to_string(), "NOT INVESTABLE");
}

#[test]
fn v2_robust_optimiser() {
    let p = world_design("base", "robust");
    let o = robust_optimise(&p).unwrap();
    assert_eq!(o.evals, 351);
    assert_eq!(o.holds, vec![3.0, 4.0, 6.0, 8.0, 12.0]);
    let b = o.best.expect("single-year best exists");
    assert_eq!((b.bess_mw, b.bess_h, b.max_throttle_h, b.ok), (18.5, 2.0, 12.0, true));
    close("best.npv", b.npv, 65268857.08012283);
    let v = o.verified.expect("a robust design exists for the Base Case");
    assert_eq!((v.bess_mw, v.bess_h, v.max_throttle_h, v.n, v.bumps), (18.5, 2.0, 12.0, 16, 0));
    close("verified.npv", v.npv, 66649148.34701935);
    close("verified.capex", v.capex, 12857500.0);
    close("verified.p_ok", v.p_ok, 1.0);
}

#[test]
fn v2_stress_suite() {
    let p = world_design("base", "robust");
    let s = stress_suite(&p).unwrap();
    close("base_npv", s.base_npv, 66649148.34701935);
    assert!(s.base_ok);
    let cells: &[(&str, &str, f64, bool, bool, f64, f64)] = &[
        ("current", "stated", 66852694.280095115, true, true, 0.0, 12857500.0),
        ("current", "base", 66649148.34701935, true, true, 0.0, 12857500.0),
        ("current", "harsh", 68987876.64146836, true, true, 0.0, 12857500.0),
        ("current", "fasttrack", 13162965.048237225, true, true, 0.0, 12857500.0),
        ("current", "crunch", 27323308.63505175, true, true, 0.0, 17112500.0),
        ("current", "tariff", 66518657.921446115, false, true, 309.0970895320176, 12857500.0),
        ("current", "workload", 65547061.93451532, false, true, 722.4336778044702, 12857500.0),
        ("robust", "stated", 66852694.280095115, true, true, 0.0, 12857500.0),
        ("robust", "base", 66649148.34701935, true, true, 0.0, 12857500.0),
        ("robust", "harsh", 68987876.64146836, true, true, 0.0, 12857500.0),
        ("robust", "fasttrack", 13162965.048237225, true, true, 0.0, 12857500.0),
        ("robust", "crunch", 27323308.63505175, true, true, 0.0, 17112500.0),
        ("robust", "tariff", 66518657.921446115, false, true, 309.0970895320176, 12857500.0),
        ("robust", "workload", 65547061.93451532, false, true, 722.4336778044702, 12857500.0),
        ("edge", "stated", 69820473.24584651, true, true, 0.18643172085285187, 9382500.0),
        ("edge", "base", -8109573.992290086, false, false, 11.283367622097334, 9382500.0),
        ("edge", "harsh", -5531223.630155316, false, false, 42.3897903861602, 9382500.0),
        ("edge", "fasttrack", -7787812.073629463, false, false, 11.283367622097334, 9382500.0),
        ("edge", "crunch", -11184975.931071643, false, false, 11.283367622097334, 12487500.0),
        ("edge", "tariff", -8109573.992290086, false, false, 465.3433115756512, 9382500.0),
        ("edge", "workload", -8109573.992290086, false, false, 867.1456521469354, 9382500.0),
        ("eng", "stated", 55012775.28307322, false, true, 91.95314557850361, 26220000.0),
        ("eng", "base", 54671164.53498632, false, true, 154.70910623669624, 26220000.0),
        ("eng", "harsh", 59920780.87590205, false, true, 449.03460409343234, 26220000.0),
        ("eng", "fasttrack", 1694994.3440219949, false, true, 154.70910623669624, 26220000.0),
        ("eng", "crunch", 9237306.043850562, false, true, 154.70910623669624, 36800000.0),
        ("eng", "tariff", 54548850.1443979, false, true, 490.75414007306097, 26220000.0),
        ("eng", "workload", 54382550.508921765, false, true, 257.0655021607876, 26220000.0),
        ("sw", "stated", 0.0, false, false, 374.8060627877712, 0.0),
        ("sw", "base", 0.0, false, false, 461.37588000297546, 0.0),
        ("sw", "harsh", 0.0, false, false, 855.6791130751371, 0.0),
        ("sw", "fasttrack", 0.0, false, false, 461.37588000297546, 0.0),
        ("sw", "crunch", 0.0, false, false, 461.37588000297546, 0.0),
        ("sw", "tariff", 0.0, false, false, 1268.5636841207743, 0.0),
        ("sw", "workload", 0.0, false, false, 1629.0974922358985, 0.0),
        ("diesel", "stated", 0.0, false, false, 834.0721456855536, 0.0),
        ("diesel", "base", 0.0, false, false, 899.0721456855536, 0.0),
        ("diesel", "harsh", 0.0, false, false, 2485.4211582243443, 0.0),
        ("diesel", "fasttrack", 0.0, false, false, 899.0721456855536, 0.0),
        ("diesel", "crunch", 0.0, false, false, 899.0721456855536, 0.0),
        ("diesel", "tariff", 0.0, false, false, 1494.4642038275797, 0.0),
        ("diesel", "workload", 0.0, false, false, 1230.6126474529503, 0.0),
    ];
    let got: Vec<_> = s.matrix.iter().flat_map(|r| r.cells.iter().map(move |c| (r.design.clone(), c.clone()))).collect();
    assert_eq!(got.len(), cells.len());
    for ((d, c), (wd, ww, npv, ok, cert, short, capex)) in got.iter().zip(cells) {
        assert_eq!((d.as_str(), c.world, c.compliant, c.certified), (*wd, *ww, *ok, *cert), "matrix {wd} x {ww}");
        close("matrix.npv", c.npv, *npv);
        close("matrix.short", c.short, *short);
        close("matrix.capex", c.capex, *capex);
    }
    let torn: &[(&str, f64, f64, bool, f64, f64, bool)] = &[
        ("margin_pre_energy", 160.0, 17005893.677359626, true, 480.0, 116292403.016679, true),
        ("queue_months", 24.0, 30022878.68487737, true, 48.0, 90645685.4447102, true),
        ("bess_lead", 3.0, 75914226.84586431, true, 12.0, 48707210.01697548, true),
        ("wacc", 0.06, 71105269.73544265, true, 0.14, 60107117.625322334, true),
        ("bess_cost", 228.0, 68698427.30353343, true, 399.0, 62550590.43399116, true),
        ("stress_hours", 121.0, 66377803.169844925, true, 360.0, 67487976.31370278, true),
        ("defer_cost", 0.0, 66761640.53666698, true, 225.0, 65749210.82983806, true),
        ("ceiling_mw", 20.0, 66710267.458241634, false, 24.0, 66485158.60374321, true),
        ("max_throttle_h", 8.0, 66454838.45667844, false, 16.0, 66649148.34701935, true),
        ("soc_window", 0.9500000000000001, 66706352.931609906, true, 0.8, 66529904.9898148, false),
        ("safety_margin", 0.0, 66607902.19002388, true, 1.5, 66705824.5202671, false),
        ("defer_share", 0.3, 66570739.274783224, false, 0.39999999999999997, 66640064.81623823, true),
    ];
    assert_eq!(s.tornado.len(), torn.len());
    for (t, (k, lv, lnpv, lok, hv, hnpv, hok)) in s.tornado.iter().zip(torn) {
        assert_eq!((t.key, t.lo.ok, t.hi.ok), (*k, *lok, *hok), "tornado order/compliance");
        close("tornado.lo.v", t.lo.v, *lv);
        close("tornado.lo.npv", t.lo.npv, *lnpv);
        close("tornado.hi.v", t.hi.v, *hv);
        close("tornado.hi.npv", t.hi.npv, *hnpv);
    }
    let ths: &[(&str, Option<f64>)] = &[
        ("margin", Some(105.19007654748921)),
        ("defer", Some(14836.950179779935)),
        ("sla", Some(35984.99664116611)),
        ("bessCost", Some(2138.8234844524686)),
        ("wacc", None),
        ("queue", Some(14.0)),
        ("lead", Some(30.0)),
        ("carry", Some(504686.36637060286)),
        ("ceiling", Some(22.0)),
        ("deferShare", Some(0.35)),
        ("hold", Some(12.0)),
        ("bessMW", Some(18.5)),
        ("deadband", Some(0.5)),
    ];
    assert_eq!(s.thresholds.len(), ths.len());
    for (t, (id, v)) in s.thresholds.iter().zip(ths) {
        assert_eq!(t.id, *id);
        close_opt(id, t.value, *v);
    }
    let ints: &[(f64, f64)] = &[(180.0, 1.0), (240.0, 1.0), (300.0, 1.0), (360.0, 1.0), (450.0, 1.0), (600.0, 1.0)];
    assert_eq!(s.intensity.len(), ints.len());
    for (x, (sh, pc)) in s.intensity.iter().zip(ints) {
        assert_eq!(x.stress_hours, *sh);
        close("intensity", x.p_compliant, *pc);
    }
}
