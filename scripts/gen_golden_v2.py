#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Swanand Tanavade
"""Generate tests/golden_v2.rs from tests/golden_v2.json.

tests/golden_v2.json is written by `node scripts/golden.mjs --write` from the web reference model.
The crate has zero dependencies, so the Rust tests embed expected values as literals instead of
parsing JSON at test time. Run this after an intentional model change, then `cargo test`.
"""
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parent.parent
g = json.loads((ROOT / "tests" / "golden_v2.json").read_text())


def lit(v):
    if v is None:
        return "f64::NAN"
    if v == "inf":
        return "f64::INFINITY"
    if isinstance(v, bool):
        return "true" if v else "false"
    r = repr(float(v))
    if r == "inf":
        return "f64::INFINITY"
    return r if ("." in r or "e" in r) else r + ".0"


def opt(v):
    if v is None:
        return "None"
    if v == "inf":
        return "Some(f64::INFINITY)"
    return f"Some({lit(v)})"


HEADER = '''// SPDX-License-Identifier: AGPL-3.0-only
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
'''

out = [HEADER]
out.append(f'''
#[test]
fn v2_seed_list() {{
    assert_eq!(firmgap::seed_list(2026, 5), vec![{", ".join(str(x) for x in g["seeds"])}]);
}}
''')

for key, sc in g["worlds_designs"].items():
    name = key.replace("__", "_")
    params = sc["params"]
    strat = "BessFirst" if params["strategy"] == "bess" else "ThrottleFirst"
    plist = ", ".join(f'("{k}", {lit(v)})' for k, v in params.items() if k != "strategy")
    f, e, i = sc["flex"], sc["econ"], sc["inv"]
    pay = "None" if i["payback"] is None else f"Some({int(i['payback'])})"
    gcodes = ", ".join(f'"{c}"' for c in sc["guard"])
    out.append(f'''
#[test]
fn v2_{name}() {{
    let p = world_design("{sc['world']}", "{sc['design']}");
    assert_params(&p, &[{plist}], Strategy::{strat});
    let r = run(&p, false).unwrap();
    close("flex.short_e", r.flex.short_e, {lit(f['shortE'])});
    close("flex.breach_pct", r.flex.breach_pct, {lit(f['breachPct'])});
    close("flex.thr_e", r.flex.thr_e, {lit(f['thrE'])});
    close("flex.energy_cost", r.flex.energy_cost, {lit(f['energyCost'])});
    assert_eq!(r.flex.failed5, {f['failed5']});
    assert_eq!(r.flex.gen_hrs, {f['genHrs']});
    assert_eq!(r.econ.certified, {lit(e['certified'])});
    close("econ.full_month", r.econ.full_month, {lit(e['F'])});
    close("econ.capex", r.econ.capex, {lit(e['capex'])});
    close("econ.reduction", r.econ.reduction, {lit(e['reduction'])});
    close("econ.c.total", r.econ.c.total, {lit(e['Ctotal'])});
    let i = &r.inv;
    close("inv.npv", i.npv, {lit(i['npv'])});
    close("inv.npv_fail", i.npv_fail, {lit(i['npvFail'])});
    assert_eq!(i.payback, {pay});
    close_opt("inv.irr", i.irr, {opt(i['irr'])});
    close("inv.bridge_monthly", i.bridge_monthly, {lit(i['bridgeMonthly'])});
    close("inv.arb_monthly", i.arb_monthly, {lit(i['arbMonthly'])});
    assert_eq!(i.disposition.to_string(), "{i['disposition']}");
    assert_eq!(i.fail_disposition.to_string(), "{i['failDisposition']}");
    close("inv.post_pv", i.post_pv, {lit(i['postPV'])});
    close_opt("inv.cost_per_mw_month", i.cost_per_mw_month, {opt(i['costPerMWMonth'])});
    close_opt("inv.break_even_margin", i.break_even_margin, {opt(i['breakEvenMargin'])});
    close_opt("inv.break_even_defer_cost", i.break_even_defer_cost, {opt(i['breakEvenDeferCost'])});
    close_opt("inv.break_even_sla_penalty", i.break_even_sla_penalty, {opt(i['breakEvenSlaPenalty'])});
    close("inv.sw_pv", i.sw_pv, {lit(i['swPV'])});
    close_opt("inv.sw_share", i.sw_share, {opt(i['swShare'])});
    close_opt("inv.sw_payback_days", i.sw_payback_days, {opt(i['swPaybackDays'])});
    close("inv.capex_pv", i.capex_pv, {lit(i['capexPV'])});
    close("inv.pv_factor", i.pv_factor, {lit(i['pvFactor'])});
    close("inv.bridge_months", i.bridge_months, {lit(i['bridgeMonths'])});
    close("inv.mw_months", i.mw_months, {lit(i['mwMonths'])});
    assert_eq!(r.compliant(), {lit(sc['compliant'])});
    let g = guardrails(&p, &r, None);
    assert_eq!(codes(&g), vec![{gcodes}]);
    assert_eq!(verdict(&g, false).to_string(), "{sc['verdictNoMc']}");
}}
''')

for key in [k for k in g if k.startswith("mc__")]:
    m = g[key]
    _, w, d = key.split("__")
    gcodes = ", ".join(f'"{c}"' for c in m["guard"])
    out.append(f'''
#[test]
fn v2_monte_carlo_{w}_{d}() {{
    let p = world_design("{w}", "{d}");
    let mc = monte_carlo(&p, {m['n']}).unwrap();
    close("p_compliant", mc.p_compliant, {lit(m['pCompliant'])});
    close("p_certified", mc.p_certified, {lit(m['pCertified'])});
    close("npv_p10", mc.npv_p10, {lit(m['npvP10'])});
    close("npv_p50", mc.npv_p50, {lit(m['npvP50'])});
    close("npv_p90", mc.npv_p90, {lit(m['npvP90'])});
    close("short_p50", mc.short_p50, {lit(m['shortP50'])});
    close("short_p90", mc.short_p90, {lit(m['shortP90'])});
    close("penalty_mean", mc.penalty_mean, {lit(m['penaltyMean'])});
    let r = run(&p, false).unwrap();
    let g = guardrails(&p, &r, Some(&mc));
    assert_eq!(codes(&g), vec![{gcodes}]);
    assert_eq!(verdict(&g, true).to_string(), "{m['verdict']}");
}}
''')

ro = g["robust"]
v, b = ro["verified"], ro["best"]
out.append(f'''
#[test]
fn v2_robust_optimiser() {{
    let p = world_design("base", "robust");
    let o = robust_optimise(&p).unwrap();
    assert_eq!(o.evals, {ro['evals']});
    assert_eq!(o.holds, vec![{", ".join(lit(h) for h in ro['holds'])}]);
    let b = o.best.expect("single-year best exists");
    assert_eq!((b.bess_mw, b.bess_h, b.max_throttle_h, b.ok), ({lit(b['mw'])}, {lit(b['h'])}, {lit(b['mt'])}, true));
    close("best.npv", b.npv, {lit(b['npv'])});
    let v = o.verified.expect("a robust design exists for the Base Case");
    assert_eq!((v.bess_mw, v.bess_h, v.max_throttle_h, v.n, v.bumps), ({lit(v['mw'])}, {lit(v['h'])}, {lit(v['mt'])}, {v['n']}, {v['bumps']}));
    close("verified.npv", v.npv, {lit(v['npv'])});
    close("verified.capex", v.capex, {lit(v['capex'])});
    close("verified.p_ok", v.p_ok, {lit(v['pOk'])});
}}
''')

st = g["stress"]
cells = [
    f'        ("{row["design"]}", "{c["world"]}", {lit(c["npv"])}, {lit(c["compliant"])}, {lit(c["certified"])}, {lit(c["short"])}, {lit(c["capex"])}),'
    for row in st["matrix"]
    for c in row["cells"]
]
torn = [
    f'        ("{t["k"]}", {lit(t["lo"]["v"])}, {lit(t["lo"]["npv"])}, {lit(t["lo"]["ok"])}, {lit(t["hi"]["v"])}, {lit(t["hi"]["npv"])}, {lit(t["hi"]["ok"])}),'
    for t in st["tornado"]
]
ths = [f'        ("{x["id"]}", {opt(x["value"])}),' for x in st["be"]]
ints = [f'        ({lit(x["sh"])}, {lit(x["p"])}),' for x in st["intensity"]]
NL = "\n"
out.append(f'''
#[test]
fn v2_stress_suite() {{
    let p = world_design("base", "robust");
    let s = stress_suite(&p).unwrap();
    close("base_npv", s.base_npv, {lit(st['baseNpv'])});
    assert_eq!(s.base_ok, {lit(st['baseOk'])});
    let cells: &[(&str, &str, f64, bool, bool, f64, f64)] = &[
{NL.join(cells)}
    ];
    let got: Vec<_> = s.matrix.iter().flat_map(|r| r.cells.iter().map(move |c| (r.design.clone(), c.clone()))).collect();
    assert_eq!(got.len(), cells.len());
    for ((d, c), (wd, ww, npv, ok, cert, short, capex)) in got.iter().zip(cells) {{
        assert_eq!((d.as_str(), c.world, c.compliant, c.certified), (*wd, *ww, *ok, *cert), "matrix {{wd}} x {{ww}}");
        close("matrix.npv", c.npv, *npv);
        close("matrix.short", c.short, *short);
        close("matrix.capex", c.capex, *capex);
    }}
    let torn: &[(&str, f64, f64, bool, f64, f64, bool)] = &[
{NL.join(torn)}
    ];
    assert_eq!(s.tornado.len(), torn.len());
    for (t, (k, lv, lnpv, lok, hv, hnpv, hok)) in s.tornado.iter().zip(torn) {{
        assert_eq!((t.key, t.lo.ok, t.hi.ok), (*k, *lok, *hok), "tornado order/compliance");
        close("tornado.lo.v", t.lo.v, *lv);
        close("tornado.lo.npv", t.lo.npv, *lnpv);
        close("tornado.hi.v", t.hi.v, *hv);
        close("tornado.hi.npv", t.hi.npv, *hnpv);
    }}
    let ths: &[(&str, Option<f64>)] = &[
{NL.join(ths)}
    ];
    assert_eq!(s.thresholds.len(), ths.len());
    for (t, (id, v)) in s.thresholds.iter().zip(ths) {{
        assert_eq!(t.id, *id);
        close_opt(id, t.value, *v);
    }}
    let ints: &[(f64, f64)] = &[
{NL.join(ints)}
    ];
    assert_eq!(s.intensity.len(), ints.len());
    for (x, (sh, pc)) in s.intensity.iter().zip(ints) {{
        assert_eq!(x.stress_hours, *sh);
        close("intensity", x.p_compliant, *pc);
    }}
}}
''')

src = "".join(out)
# clippy::bool_assert_comparison: use assert!(x) / assert!(!x) for bare booleans
src = re.sub(r"assert_eq!\(([^,()]+(?:\([^()]*\))?), true\);", r"assert!(\1);", src)
src = re.sub(r"assert_eq!\(([^,()]+(?:\([^()]*\))?), false\);", r"assert!(!\1);", src)
(ROOT / "tests" / "golden_v2.rs").write_text(src)
print(f"wrote tests/golden_v2.rs ({src.count('#[test]')} tests)")
