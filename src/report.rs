// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Human-readable and JSON reports. Text output is plain ASCII so it renders in
//! any terminal, CI log or email.

use crate::audit_bundle::Dossier;
use crate::calendar::hour_to_date;
use crate::compliance::ComplianceVerdict;
use crate::economics::Pathway;
use crate::guard::{verdict, Finding};
use crate::invest::Investor;
use crate::json::Json;
use crate::optimise::{Candidate, Optimisation};
use crate::params::{Params, SPECS};
use crate::presets::PRESETS;
use crate::robust::{MonteCarlo, Robust};
use crate::sim::{EventOutcome, Run};
use crate::stress::StressReport;
use crate::worlds::{DESIGNS, WORLDS};
use std::fmt::Write as _;

/// Compact money formatting: $1.23M, $45k, $780.
pub fn money(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".into();
    }
    let a = v.abs();
    let sg = if v < 0.0 { "-" } else { "" };
    if a >= 1e9 {
        format!("{sg}${:.2}B", a / 1e9)
    } else if a >= 1e6 {
        format!("{sg}${:.2}M", a / 1e6)
    } else if a >= 1e3 {
        format!("{sg}${:.0}k", a / 1e3)
    } else {
        format!("{sg}${a:.0}")
    }
}

/// Percentage with `d` decimals.
pub fn pct(v: f64, d: usize) -> String {
    if v.is_finite() {
        format!("{:.*}%", d, v * 100.0)
    } else {
        "n/a".into()
    }
}

/// "n/a" or "N months".
pub fn opt_months(v: Option<u32>) -> String {
    v.map_or_else(|| "n/a".to_string(), |m| format!("{m} months"))
}

/// IRR text (">100%" above 100%, "n/a" if undefined).
pub fn irr_text(v: Option<f64>) -> String {
    match v {
        None => "n/a".into(),
        Some(x) if !x.is_finite() || x > 1.0 => ">100%".into(),
        Some(x) => pct(x, 0),
    }
}

/// "$X/MWh" or "n/a".
pub fn per_mwh(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".to_string(), |x| format!("${x:.0}/MWh"))
}

fn status(ok: bool) -> &'static str {
    if ok {
        "PASS"
    } else {
        "FAIL"
    }
}

/// Events that had a gap above the ceiling.
fn gapped(r: &Run) -> usize {
    r.flex.events.iter().filter(|e| e.peak_gap > 1e-9).count()
}

/// Full text report for a run.
pub fn run_text(r: &Run, label: &str, with_events: bool) -> String {
    let p = &r.params;
    let e = &r.econ;
    let f = &r.flex;
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} - interim non-firm energization economics", crate::VERSION);
    let _ = writeln!(o, "scenario: {label}   seed: {}   strategy: {}-first", p.seed, p.strategy.as_str());
    let _ = writeln!(
        o,
        "bridge: BESS {:.1} MW / {:.1} h ({:.1} MWh, capex {}), genset {:.1} MW, throttle hold {:.0} h\n",
        p.bess_mw,
        p.bess_h,
        e.bess_mwh,
        money(e.capex),
        p.gen_mw,
        p.max_throttle_h
    );
    let placed = r.year.placed_hours();
    let _ = writeln!(
        o,
        "year: {} curtailment events, {} signal hours{}; peak gap {:.2} MW; gap energy {:.0} MWh/yr",
        r.year.events.len(),
        placed,
        if placed < r.year.requested_hours {
            format!(" (WARNING: {} requested; windows saturated)", r.year.requested_hours)
        } else {
            String::new()
        },
        f.max_gap,
        f.gap_e
    );

    let _ = writeln!(o, "\nTARGET KPIs");
    let breach = if e.certified { f.breach_pct } else { 0.0 };
    let kpi = |o: &mut String, name: &str, ok: bool, value: String, target: &str| {
        let _ = writeln!(o, "  [{}] {:<34} {:>16}   target {}", status(ok), name, value, target);
    };
    kpi(
        &mut o,
        "capacity pull-forward",
        e.certified && e.pull_forward_months >= 24.0,
        format!("{:.0} months", e.pull_forward_months),
        "24-34 months",
    );
    kpi(&mut o, "SLA-breach deferrals", breach < p.sla_target, pct(breach, 3), &format!("< {}", pct(p.sla_target, 1)));
    kpi(
        &mut o,
        "non-compliance penalties",
        !e.certified || f.short_e < 0.5,
        if e.certified { format!("{}/yr", money(f.penalty_cost)) } else { "n/a".into() },
        "avoid",
    );
    kpi(
        &mut o,
        "first-minutes response test",
        f.pass5_all,
        format!("{}/{} events", gapped(r) - f.failed5, gapped(r)),
        &format!("all within {:.1} min", p.ramp_min),
    );
    kpi(&mut o, "all-in $/MWh vs. waiting", e.reduction >= 0.25, format!("{:+.1}%", -e.reduction * 100.0), "<= -25%");

    let _ = writeln!(o, "\nPATHWAYS over {:.0} months", p.horizon);
    let _ = writeln!(o, "  {:<26}{:>15}{:>15}{:>15}", "", "A wait firm", "B interim", "C flex bridge");
    let row = |o: &mut String, name: &str, g: &dyn Fn(&Pathway) -> String| {
        let _ = writeln!(o, "  {:<26}{:>15}{:>15}{:>15}", name, g(&e.a), g(&e.b), g(&e.c));
    };
    row(&mut o, "compute served (GWh)", &|s| format!("{:.1}", s.mwh / 1000.0));
    row(&mut o, "energy", &|s| money(s.energy));
    row(&mut o, "BESS annualized capex", &|s| money(s.bess));
    row(&mut o, "genset", &|s| money(s.gen));
    row(&mut o, "penalties", &|s| money(s.penalty));
    row(&mut o, "queue-delay carry", &|s| money(s.carry));
    row(&mut o, "all-in total", &|s| money(s.total));
    row(&mut o, "all-in $/MWh", &|s| format!("${:.2}", s.per_mwh));
    row(&mut o, "overhead $/MWh (non-energy)", &|s| format!("${:.2}", s.overhead_per_mwh()));

    let _ = writeln!(o, "\nDISPATCH (full campus, non-firm year)");
    let share = |x: f64| if f.gap_e > 0.0 { pct(x / f.gap_e, 0) } else { "-".into() };
    let _ = writeln!(
        o,
        "  throttled {:.0} MWh ({})  BESS {:.0} MWh ({})  genset {:.0} MWh ({}, {} h)  shortfall {:.2} MWh",
        f.thr_e,
        share(f.thr_e),
        f.bes_e,
        share(f.bes_e),
        f.gen_e,
        share(f.gen_e),
        f.gen_hrs,
        f.short_e
    );
    let _ = writeln!(
        o,
        "  deferred {} of compute, SLA-breached {:.0} MWh ({}), worst first-minutes coverage {}",
        pct(f.defer_pct, 2),
        f.breach,
        pct(f.breach_pct, 3),
        pct(f.worst_ramp_cover, 0)
    );

    let _ = writeln!(o, "\nDIAGNOSTICS");
    let _ = writeln!(o, "  25% all-in reduction requires carry >= {}/month (case uses {}/month)", money(e.breakeven_carry), money(p.carry));
    let _ = writeln!(o, "  post-firm BESS arbitrage value: {}/yr", money(r.arbitrage_value()));
    let iv = &r.inv;
    let _ = writeln!(o, "\nINVESTOR VIEW vs. interim-only (model 0.2; carry excluded, see docs/MODEL.md)");
    let _ = writeln!(
        o,
        "  NPV {}   downside if witness test fails {}   payback {}   IRR {}",
        money(iv.npv),
        money(iv.npv_fail),
        opt_months(iv.payback),
        irr_text(iv.irr)
    );
    let _ = writeln!(
        o,
        "  bridge {:.0} months, {:.0} MW-months unlocked, cost per MW-month {}",
        iv.bridge_months,
        iv.mw_months,
        iv.cost_per_mw_month.map_or_else(|| "n/a".to_string(), money)
    );
    let _ = writeln!(
        o,
        "  BESS capex {} (PV {}), post-firm: {} ({})",
        money(iv.capex_at_risk),
        money(iv.capex_pv),
        iv.disposition,
        money(iv.post_pv)
    );
    let _ = writeln!(
        o,
        "  break-evens: margin {}  deferral fee {}  SLA penalty {}",
        per_mwh(iv.break_even_margin),
        per_mwh(iv.break_even_defer_cost),
        per_mwh(iv.break_even_sla_penalty)
    );
    let _ = writeln!(
        o,
        "  software value capture {} of value (PV {}), customer payback {}",
        iv.sw_share.map_or_else(|| "n/a".to_string(), |v| pct(v, 1)),
        money(iv.sw_pv),
        iv.sw_payback_days.map_or_else(|| "n/a".to_string(), |v| format!("{v:.1} days"))
    );
    if !e.certified {
        let _ = writeln!(o, "  NOTE: not certifiable, so pathway C falls back to interim-only (B).");
    }

    if with_events {
        let _ = writeln!(o, "\nEVENT LEDGER");
        let _ = writeln!(
            o,
            "  {:>3}  {:<12} {:>4} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}  status",
            "#", "start", "dur", "gap MW", "thr", "bess", "gen", "short", "breach"
        );
        for ev in &f.events {
            let stamp = hour_to_date(ev.start).to_string();
            let _ = writeln!(
                o,
                "  {:>3}  {:<12} {:>4} {:>8.2} {:>8.1} {:>8.1} {:>8.1} {:>8.2} {:>8.1}  {}",
                ev.id + 1,
                stamp,
                ev.dur,
                ev.peak_gap,
                ev.thr,
                ev.bes,
                ev.gen,
                ev.short,
                ev.breach,
                event_status(ev)
            );
        }
    }
    o
}

/// Classification used by the ledger and the web UI.
pub fn event_status(e: &EventOutcome) -> &'static str {
    if e.peak_gap <= 1e-9 {
        "no-gap"
    } else if e.short > 0.05 {
        "shortfall"
    } else if !e.pass5 {
        "ramp-fail"
    } else if e.breach > 0.5 {
        "sla-breach"
    } else {
        "compliant"
    }
}

fn pathway_json(j: &mut Json, name: &str, s: &Pathway) {
    j.key(name).begin_obj();
    j.kv_num("mwh", s.mwh)
        .kv_num("energy", s.energy)
        .kv_num("bess", s.bess)
        .kv_num("gen", s.gen)
        .kv_num("penalty", s.penalty)
        .kv_num("carry", s.carry)
        .kv_num("total", s.total)
        .kv_num("per_mwh", s.per_mwh)
        .kv_num("overhead_per_mwh", s.overhead_per_mwh());
    j.key("energized_mw").begin_arr();
    for v in &s.series {
        j.num(*v);
    }
    j.end_arr();
    j.end_obj();
}

fn params_json(j: &mut Json, p: &Params) {
    j.key("params").begin_obj();
    for sp in SPECS {
        j.kv_num(sp.key, p.get(sp.key).unwrap_or(f64::NAN));
    }
    j.key("strategy").string(p.strategy.as_str());
    j.key("seed").uint(u64::from(p.seed));
    j.end_obj();
}

/// Machine-readable report for a run.
pub fn run_json(r: &Run, label: &str, with_events: bool) -> String {
    let e = &r.econ;
    let f = &r.flex;
    let mut j = Json::new();
    j.begin_obj();
    j.key("tool").string("firmgap").key("version").string(crate::VERSION).key("scenario").string(label);
    params_json(&mut j, &r.params);
    j.key("year").begin_obj();
    j.key("events").uint(r.year.events.len() as u64);
    j.key("requested_hours").uint(r.year.requested_hours as u64);
    j.key("placed_hours").uint(r.year.placed_hours() as u64);
    j.end_obj();
    j.key("kpis").begin_obj();
    j.key("certified").boolean(e.certified);
    j.kv_num("pull_forward_months", e.pull_forward_months);
    j.kv_num("full_capacity_month", e.full_month);
    j.kv_num("sla_breach_pct", if e.certified { f.breach_pct } else { 0.0 });
    j.kv_num("penalty_per_year", f.penalty_cost);
    j.kv_num("shortfall_mwh", f.short_e);
    j.key("events_with_gap").uint(gapped(r) as u64);
    j.key("events_failing_first_minutes").uint(f.failed5 as u64);
    j.kv_num("all_in_reduction_vs_wait", e.reduction);
    j.kv_num("breakeven_carry_per_month", e.breakeven_carry);
    j.kv_num("bess_capex", e.capex);
    j.kv_num("bess_annualized", e.bess_ann);
    j.kv_num("arbitrage_value_per_year", r.arbitrage_value());
    j.end_obj();
    j.key("dispatch").begin_obj();
    j.kv_num("demand_mwh", f.demand)
        .kv_num("gap_mwh", f.gap_e)
        .kv_num("max_gap_mw", f.max_gap)
        .kv_num("throttled_mwh", f.thr_e)
        .kv_num("bess_mwh", f.bes_e)
        .kv_num("genset_mwh", f.gen_e)
        .kv_num("shortfall_mwh", f.short_e)
        .kv_num("sla_breached_mwh", f.breach)
        .kv_num("caught_up_mwh", f.catch_e)
        .kv_num("worst_first_minutes_cover", f.worst_ramp_cover);
    j.key("genset_hours").uint(u64::from(f.gen_hrs));
    j.end_obj();
    investor_json(&mut j, &r.inv);
    j.key("pathways").begin_obj();
    pathway_json(&mut j, "A_wait_for_firm", &e.a);
    pathway_json(&mut j, "B_interim_only", &e.b);
    pathway_json(&mut j, "C_flex_bridge", &e.c);
    j.end_obj();
    if with_events {
        j.key("events").begin_arr();
        for ev in &f.events {
            j.begin_obj();
            j.key("id").uint(ev.id as u64 + 1);
            j.key("start_hour").uint(ev.start as u64);
            j.key("start").string(&hour_to_date(ev.start).to_string());
            j.key("duration_h").uint(ev.dur as u64);
            j.kv_num("peak_gap_mw", ev.peak_gap)
                .kv_num("throttled_mwh", ev.thr)
                .kv_num("bess_mwh", ev.bes)
                .kv_num("genset_mwh", ev.gen)
                .kv_num("shortfall_mwh", ev.short)
                .kv_num("sla_breached_mwh", ev.breach);
            j.key("first_minutes_ok").boolean(ev.pass5);
            j.key("status").string(event_status(ev));
            j.end_obj();
        }
        j.end_arr();
    }
    j.end_obj();
    j.finish()
}

fn opt_num(j: &mut Json, k: &str, v: Option<f64>) {
    j.key(k).num(v.unwrap_or(f64::NAN));
}

fn investor_json(j: &mut Json, i: &Investor) {
    j.key("investor").begin_obj();
    j.kv_num("npv", i.npv).kv_num("npv_downside", i.npv_fail).kv_num("capex_at_risk", i.capex_at_risk).kv_num("capex_pv", i.capex_pv);
    j.key("payback_months");
    match i.payback {
        Some(m) => {
            j.uint(u64::from(m));
        }
        None => {
            j.num(f64::NAN);
        }
    }
    j.key("irr");
    match i.irr {
        Some(x) if x.is_finite() => {
            j.num(x);
        }
        Some(_) => {
            j.string("above search range");
        }
        None => {
            j.num(f64::NAN);
        }
    }
    j.kv_num("bridge_months", i.bridge_months).kv_num("mw_months", i.mw_months).kv_num("bridge_cash_per_month", i.bridge_monthly);
    opt_num(j, "cost_per_mw_month", i.cost_per_mw_month);
    opt_num(j, "break_even_margin", i.break_even_margin);
    opt_num(j, "break_even_defer_cost", i.break_even_defer_cost);
    opt_num(j, "break_even_sla_penalty", i.break_even_sla_penalty);
    j.key("disposition").string(&i.disposition.to_string());
    j.key("downside_disposition").string(&i.fail_disposition.to_string());
    j.kv_num("post_firm_pv", i.post_pv).kv_num("arbitrage_per_month", i.arb_monthly).kv_num("software_pv", i.sw_pv);
    opt_num(j, "software_share", i.sw_share);
    j.key("parts_per_month").begin_obj();
    let c = &i.parts;
    j.kv_num("margin", c.margin)
        .kv_num("energy", c.energy)
        .kv_num("penalty", c.penalty)
        .kv_num("genset", c.genset)
        .kv_num("deferral", c.deferral)
        .kv_num("sla", c.sla);
    j.end_obj();
    j.end_obj();
}

fn candidate_json(j: &mut Json, c: &Candidate) {
    j.begin_obj();
    j.kv_num("bess_mw", c.bess_mw)
        .kv_num("bess_h", c.bess_h)
        .kv_num("bess_mwh", c.bess_mw * c.bess_h)
        .kv_num("max_throttle_h", c.max_throttle_h);
    j.key("feasible").boolean(c.feasible);
    j.kv_num("capex", c.capex)
        .kv_num("total", c.total)
        .kv_num("per_mwh", c.per_mwh)
        .kv_num("shortfall_mwh", c.short_e)
        .kv_num("sla_breach_pct", c.breach_pct)
        .kv_num("reduction_vs_wait", c.reduction);
    j.end_obj();
}

/// Text report for an optimisation.
pub fn optimise_text(o: &Optimisation, reference: &Run) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "firmgap {} - least-cost compliant bridge search", crate::VERSION);
    let _ = writeln!(s, "evaluated {} configurations (each a full 8,760-hour simulation)\n", o.evaluated.len());
    match &o.best {
        None => {
            let _ = writeln!(s, "No feasible configuration up to 33.5 MW BESS. Raise the ceiling, add genset capacity (check permit hours), deepen GPU power-capping or widen the SLA window.");
        }
        Some(b) => {
            let _ = writeln!(
                s,
                "RECOMMENDED  BESS {:.1} MW / {:.0} h ({:.1} MWh) + throttle hold {:.0} h",
                b.bess_mw,
                b.bess_h,
                b.bess_mw * b.bess_h,
                b.max_throttle_h
            );
            let _ = writeln!(
                s,
                "  capex {}   all-in ${:.2}/MWh   shortfall {:.2} MWh/yr   SLA breach {}",
                money(b.capex),
                b.per_mwh,
                b.short_e,
                pct(b.breach_pct, 3)
            );
            let rp = &reference.params;
            let _ = writeln!(
                s,
                "\nversus input configuration ({:.1} MW / {:.1} h, hold {:.0} h): capex {} -> {} ({}), all-in ${:.2} -> ${:.2}/MWh",
                rp.bess_mw,
                rp.bess_h,
                rp.max_throttle_h,
                money(reference.econ.capex),
                money(b.capex),
                if reference.econ.capex > 0.0 { format!("{:+.0}%", (b.capex / reference.econ.capex - 1.0) * 100.0) } else { "n/a".into() },
                reference.econ.c.per_mwh,
                b.per_mwh
            );
            let _ = writeln!(
                s,
                "apply with: firmgap run --set bess_mw={} --set bess_h={} --set max_throttle_h={}",
                b.bess_mw, b.bess_h, b.max_throttle_h
            );
        }
    }
    s
}

/// JSON report for an optimisation.
pub fn optimise_json(o: &Optimisation) -> String {
    let mut j = Json::new();
    j.begin_obj();
    j.key("tool").string("firmgap").key("version").string(crate::VERSION);
    j.key("evaluated").uint(o.evaluated.len() as u64);
    j.key("best");
    match &o.best {
        Some(b) => candidate_json(&mut j, b),
        None => {
            j.num(f64::NAN);
        }
    }
    j.key("candidates").begin_arr();
    for c in &o.evaluated {
        candidate_json(&mut j, c);
    }
    j.end_arr();
    j.end_obj();
    j.finish()
}

/// Text table for a BESS power sweep.
pub fn sweep_text(rows: &[Candidate]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{:>8} {:>6} {:>12} {:>12} {:>12} {:>10}  status", "BESS MW", "h", "capex", "$/MWh", "short MWh", "SLA");
    for c in rows {
        let _ = writeln!(
            s,
            "{:>8.1} {:>6.1} {:>12} {:>12.2} {:>12.2} {:>10}  {}",
            c.bess_mw,
            c.bess_h,
            money(c.capex),
            c.per_mwh,
            c.short_e,
            pct(c.breach_pct, 3),
            if c.feasible { "compliant" } else { "non-compliant" }
        );
    }
    s
}

/// JSON for a sweep.
pub fn sweep_json(rows: &[Candidate]) -> String {
    let mut j = Json::new();
    j.begin_arr();
    for c in rows {
        candidate_json(&mut j, c);
    }
    j.end_arr();
    j.finish()
}

/// Parameter reference table.
pub fn params_text() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "{:<16} {:>12} {:>12} {:>12} {:<9} description", "key", "default", "min", "max", "unit");
    for sp in SPECS {
        let _ = writeln!(s, "{:<16} {:>12} {:>12} {:>12} {:<9} {}", sp.key, sp.default, sp.min, sp.max, sp.unit, sp.help);
    }
    let _ = writeln!(s, "{:<16} {:>12} {:>12} {:>12} {:<9} Dispatch priority: throttle | bess", "strategy", "throttle", "-", "-", "enum");
    let _ = writeln!(s, "{:<16} {:>12} {:>12} {:>12} {:<9} Synthetic weather-year seed", "seed", 2026, 0, u32::MAX, "u32");
    s
}

/// Preset list.
pub fn presets_text() -> String {
    let mut s = String::new();
    for p in PRESETS {
        let _ = writeln!(s, "{:<18} {}", p.name, p.description);
    }
    s
}

fn findings_text(o: &mut String, g: &[Finding]) {
    for f in g {
        let tag = match f.sev {
            crate::guard::Severity::Violation => "VIOLATION",
            crate::guard::Severity::Warning => "WARNING  ",
            crate::guard::Severity::Info => "INFO     ",
        };
        let _ = writeln!(o, "  [{}] {} ({})\n      {}", tag, f.title, f.code, f.detail);
    }
}

fn findings_json(j: &mut Json, g: &[Finding]) {
    j.key("guardrails").begin_arr();
    for f in g {
        j.begin_obj();
        j.key("severity").string(match f.sev {
            crate::guard::Severity::Violation => "violation",
            crate::guard::Severity::Warning => "warning",
            crate::guard::Severity::Info => "info",
        });
        j.key("code").string(f.code).key("title").string(&f.title).key("detail").string(&f.detail);
        j.end_obj();
    }
    j.end_arr();
}

fn mc_summary_text(o: &mut String, mc: &MonteCarlo) {
    let _ = writeln!(
        o,
        "  compliant {}/{} years ({}), certifiable {}, NPV P10/P50/P90 {} / {} / {}, shortfall P50/P90 {:.1} / {:.1} MWh/yr, mean penalty {}/yr",
        mc.rows.iter().filter(|r| r.compliant).count(),
        mc.n,
        pct(mc.p_compliant, 0),
        pct(mc.p_certified, 0),
        money(mc.npv_p10),
        money(mc.npv_p50),
        money(mc.npv_p90),
        mc.short_p50,
        mc.short_p90,
        money(mc.penalty_mean)
    );
}

fn mc_json(j: &mut Json, mc: &MonteCarlo, rows: bool) {
    j.key("monte_carlo").begin_obj();
    j.key("years").uint(mc.n as u64);
    j.kv_num("p_compliant", mc.p_compliant).kv_num("p_certified", mc.p_certified);
    j.kv_num("npv_p10", mc.npv_p10).kv_num("npv_p50", mc.npv_p50).kv_num("npv_p90", mc.npv_p90);
    j.kv_num("shortfall_p50", mc.short_p50).kv_num("shortfall_p90", mc.short_p90).kv_num("penalty_mean", mc.penalty_mean);
    if rows {
        j.key("rows").begin_arr();
        for r in &mc.rows {
            j.begin_obj();
            j.key("seed").uint(u64::from(r.seed));
            j.key("compliant").boolean(r.compliant).key("certified").boolean(r.certified);
            j.kv_num("npv", r.npv).kv_num("shortfall_mwh", r.short_e).kv_num("sla_late_pct", r.breach_pct).kv_num("penalty", r.penalty);
            j.key("events").uint(r.events as u64).key("longest_event_h").uint(r.max_event as u64);
            j.end_obj();
        }
        j.end_arr();
    }
    j.end_obj();
}

/// Investment-committee report: run + weather-year robustness + guardrails + verdict.
pub fn diligence_text(r: &Run, mc: &MonteCarlo, g: &[Finding], label: &str) -> String {
    let mut o = run_text(r, label, false);
    let _ = writeln!(o, "\nWEATHER-YEAR ROBUSTNESS ({} synthetic congestion years)", mc.n);
    mc_summary_text(&mut o, mc);
    let _ = writeln!(o, "\nGUARDRAILS");
    findings_text(&mut o, g);
    let _ = writeln!(o, "\nVERDICT: {}", verdict(g, true));
    let _ = writeln!(o, "  rule: any violation = NOT INVESTABLE; otherwise any warning = CONDITIONAL; robustness needs >= 90% compliant years and P10 NPV > 0.");
    o
}

/// JSON investment-committee report.
pub fn diligence_json(r: &Run, mc: &MonteCarlo, g: &[Finding], label: &str) -> String {
    let base = run_json(r, label, false);
    let mut j = Json::new();
    j.begin_obj();
    j.key("verdict").string(&verdict(g, true).to_string());
    findings_json(&mut j, g);
    mc_json(&mut j, mc, true);
    j.end_obj();
    let extra = j.finish();
    // Merge: drop exactly one closing brace of the base object and append the extra fields.
    let head = base.strip_suffix('}').unwrap_or(&base);
    format!("{},{}", head, &extra[1..])
}

/// Monte Carlo text report.
pub fn mc_text(mc: &MonteCarlo) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} - weather-year Monte Carlo ({} years)", crate::VERSION, mc.n);
    mc_summary_text(&mut o, mc);
    let _ = writeln!(
        o,
        "\n  {:>10} {:>10} {:>9} {:>14} {:>12} {:>9} {:>7} {:>8}",
        "seed", "compliant", "certified", "NPV", "short MWh", "SLA", "events", "longest"
    );
    for r in &mc.rows {
        let _ = writeln!(
            o,
            "  {:>10} {:>10} {:>9} {:>14} {:>12.2} {:>9} {:>7} {:>7}h",
            r.seed,
            if r.compliant { "yes" } else { "no" },
            if r.certified { "yes" } else { "no" },
            money(r.npv),
            r.short_e,
            pct(r.breach_pct, 2),
            r.events,
            r.max_event
        );
    }
    o
}

/// Monte Carlo JSON.
pub fn mc_json_doc(mc: &MonteCarlo) -> String {
    let mut j = Json::new();
    j.begin_obj();
    j.key("tool").string("firmgap").key("version").string(crate::VERSION);
    mc_json(&mut j, mc, true);
    j.end_obj();
    j.finish()
}

/// Robust optimiser text.
pub fn robust_text(o: &Robust, reference: &Run) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "firmgap {} - robust bridge optimiser (>= 90% compliant weather years)", crate::VERSION);
    let _ = writeln!(s, "searched {} configurations on the base year, hold policies {:?} h\n", o.evals, o.holds);
    match &o.verified {
        None => {
            let _ = writeln!(s, "No design up to 60 MW BESS (1, 2 or 4 h) is compliant in >= 90% of weather years. Raise the hold limit, deferrable share or ceiling, add genset hours, or reduce the deadband.");
        }
        Some(v) => {
            let _ = writeln!(
                s,
                "RECOMMENDED  BESS {:.1} MW / {:.0} h ({:.1} MWh) + throttle hold {:.0} h",
                v.bess_mw,
                v.bess_h,
                v.bess_mw * v.bess_h,
                v.max_throttle_h
            );
            let _ =
                writeln!(s, "  NPV {}   capex {}   compliant in {} of {} weather years", money(v.npv), money(v.capex), pct(v.p_ok, 0), v.n);
            if v.bumps > 0 {
                let _ = writeln!(s, "  +{:.1} MW above the single-year optimum for robustness", f64::from(v.bumps) * 0.5);
            }
            let rp = &reference.params;
            let _ = writeln!(
                s,
                "\nversus input design ({:.1} MW / {:.1} h, hold {:.0} h): NPV {} -> {}, capex {} -> {}",
                rp.bess_mw,
                rp.bess_h,
                rp.max_throttle_h,
                money(reference.inv.npv),
                money(v.npv),
                money(reference.econ.capex),
                money(v.capex)
            );
            let _ = writeln!(
                s,
                "apply with: firmgap diligence --set bess_mw={} --set bess_h={} --set max_throttle_h={}",
                v.bess_mw, v.bess_h, v.max_throttle_h
            );
        }
    }
    s
}

/// Robust optimiser JSON.
pub fn robust_json(o: &Robust) -> String {
    let mut j = Json::new();
    j.begin_obj();
    j.key("tool").string("firmgap").key("version").string(crate::VERSION);
    j.key("evaluated").uint(o.evals as u64);
    j.key("holds").begin_arr();
    for h in &o.holds {
        j.num(*h);
    }
    j.end_arr();
    j.key("recommended");
    match &o.verified {
        None => {
            j.num(f64::NAN);
        }
        Some(v) => {
            j.begin_obj();
            j.kv_num("bess_mw", v.bess_mw)
                .kv_num("bess_h", v.bess_h)
                .kv_num("max_throttle_h", v.max_throttle_h)
                .kv_num("npv", v.npv)
                .kv_num("capex", v.capex)
                .kv_num("p_compliant", v.p_ok);
            j.key("years").uint(v.n as u64).key("robustness_steps").uint(u64::from(v.bumps));
            j.end_obj();
        }
    }
    j.end_obj();
    j.finish()
}

fn threshold_value(t: &crate::stress::Threshold) -> String {
    match t.value {
        None => t.none.to_string(),
        Some(v) => match t.id {
            "margin" | "defer" | "sla" => format!("${v:.0}/MWh"),
            "bessCost" => format!("${v:.0}/kWh"),
            "wacc" => pct(v, 0),
            "queue" => format!("month {v:.0}"),
            "lead" => format!("{v:.0} months"),
            "carry" => format!("{}/mo", money(v)),
            "ceiling" | "bessMW" => format!("{v:.1} MW minimum"),
            "deferShare" => format!("{} minimum", pct(v, 0)),
            "hold" => format!("{v:.0} h minimum"),
            "deadband" => format!("{v:.2} MW maximum"),
            _ => format!("{v}"),
        },
    }
}

/// Stress suite text.
pub fn stress_text(s: &StressReport) -> String {
    let mut o = String::new();
    let _ = writeln!(
        o,
        "firmgap {} - stress suite (base NPV {}, base {})",
        crate::VERSION,
        money(s.base_npv),
        if s.base_ok { "compliant" } else { "NOT compliant" }
    );
    let _ = writeln!(o, "\nDESIGN x WORLD  NPV $M  (+ compliant, ~ certified but breaching, x not certifiable)");
    let _ = write!(o, "  {:<34}", "design");
    for w in WORLDS {
        let _ = write!(o, "{:>11}", w.id);
    }
    let _ = writeln!(o);
    for row in &s.matrix {
        let _ = write!(o, "  {:<34}", row.label);
        for c in &row.cells {
            let mark = if c.compliant {
                '+'
            } else if c.certified {
                '~'
            } else {
                'x'
            };
            let _ = write!(o, "{:>10.1}{}", c.npv / 1e6, mark);
        }
        let _ = writeln!(o);
    }
    let _ = writeln!(o, "\nNPV TORNADO (largest swing first; ! = loses compliance)");
    for t in &s.tornado {
        let _ = writeln!(
            o,
            "  {:<22} {:>10.4} -> {:<10.4} NPV {:>9}{} .. {:>9}{}",
            t.label,
            t.lo.v,
            t.hi.v,
            money(t.lo.npv),
            if t.lo.ok { ' ' } else { '!' },
            money(t.hi.npv),
            if t.hi.ok { ' ' } else { '!' }
        );
    }
    let _ = writeln!(o, "\nBREAK-EVENS AND CLIFFS");
    for t in &s.thresholds {
        let _ = writeln!(
            o,
            "  {:<42} {:<17} current {:>12}  threshold {}",
            t.label,
            t.kind,
            t.current.map_or_else(|| "-".to_string(), |v| format!("{v}")),
            threshold_value(t)
        );
    }
    let _ = writeln!(o, "\nCOMPLIANCE vs. CURTAILMENT INTENSITY ({} weather years per level)", crate::stress::INTENSITY_SEEDS);
    for x in &s.intensity {
        let _ = writeln!(o, "  {:>5.0} signal h/yr  {}", x.stress_hours, pct(x.p_compliant, 0));
    }
    o
}

/// Stress suite JSON.
pub fn stress_json(s: &StressReport) -> String {
    let mut j = Json::new();
    j.begin_obj();
    j.key("tool").string("firmgap").key("version").string(crate::VERSION);
    j.kv_num("base_npv", s.base_npv);
    j.key("base_compliant").boolean(s.base_ok);
    j.key("matrix").begin_arr();
    for row in &s.matrix {
        j.begin_obj();
        j.key("design").string(&row.design).key("label").string(&row.label);
        j.key("cells").begin_arr();
        for c in &row.cells {
            j.begin_obj();
            j.key("world").string(c.world);
            j.kv_num("npv", c.npv).kv_num("shortfall_mwh", c.short).kv_num("capex", c.capex);
            j.key("compliant").boolean(c.compliant).key("certified").boolean(c.certified);
            j.end_obj();
        }
        j.end_arr();
        j.end_obj();
    }
    j.end_arr();
    j.key("tornado").begin_arr();
    for t in &s.tornado {
        j.begin_obj();
        j.key("key").string(t.key);
        j.kv_num("low", t.lo.v).kv_num("low_npv", t.lo.npv).kv_num("high", t.hi.v).kv_num("high_npv", t.hi.npv).kv_num("swing", t.swing);
        j.key("low_compliant").boolean(t.lo.ok).key("high_compliant").boolean(t.hi.ok);
        j.end_obj();
    }
    j.end_arr();
    j.key("thresholds").begin_arr();
    for t in &s.thresholds {
        j.begin_obj();
        j.key("id").string(t.id).key("label").string(t.label).key("kind").string(t.kind);
        j.kv_num("current", t.current.unwrap_or(f64::NAN)).kv_num("threshold", t.value.unwrap_or(f64::NAN));
        j.key("note").string(t.none);
        j.end_obj();
    }
    j.end_arr();
    j.key("intensity").begin_arr();
    for x in &s.intensity {
        j.begin_obj();
        j.kv_num("stress_hours", x.stress_hours).kv_num("p_compliant", x.p_compliant);
        j.end_obj();
    }
    j.end_arr();
    j.end_obj();
    j.finish()
}

/// World and design list.
pub fn worlds_text() -> String {
    let mut s = String::from("WORLDS (--world)\n");
    for w in WORLDS {
        let _ = writeln!(s, "  {:<10} {:<32} {}", w.id, w.label, w.description);
    }
    s.push_str("\nDESIGNS (--design)\n");
    for d in DESIGNS {
        let _ = writeln!(s, "  {:<10} {:<34} {}", d.id, d.label, d.description);
    }
    s
}

/// Human-readable compliance audit report.
pub fn audit_text(d: &Dossier) -> String {
    let rep = &d.report;
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} - compliance audit ({})", crate::VERSION, rep.rulepack.name());
    let _ = writeln!(o, "rulepack {}   gross peak {:.1} MW   scenario_sha256 {}", rep.rulepack_id, d.site.gross_peak_mw, d.scenario_sha256);
    let _ = writeln!(
        o,
        "\nVERDICT: {}",
        match rep.verdict {
            ComplianceVerdict::BankablePass => "BankablePass (no failed blockers or warnings)",
            ComplianceVerdict::ConditionalWarning => "ConditionalWarning (diligence items open)",
            ComplianceVerdict::StatutoryBlocker => "StatutoryBlocker (at least one blocking rule fails)",
        }
    );
    let _ = writeln!(o, "\nCHECKS");
    for c in &rep.checks {
        let tag = if c.passed { "PASS" } else { "FAIL" };
        let _ = writeln!(o, "  [{}] {:<26} {:<7} {}", tag, c.rule_id.code(), c.severity.as_str(), c.summary);
        let _ = writeln!(o, "         cite: {}", c.rule_id.citation());
    }
    if !rep.remediations.is_empty() {
        let _ = writeln!(o, "\nPATH TO COMPLIANCE (counterfactual remediation)");
        for r in &rep.remediations {
            let _ = writeln!(o, "  {} {} [{}]", if r.recommended { "*" } else { "-" }, r.title, r.rule_id.code());
            let _ = writeln!(o, "      {}", r.action);
            if r.value_usd_per_year != 0.0 || r.delta_vs_other_usd_per_year != 0.0 {
                let _ = writeln!(
                    o,
                    "      value {}/yr (5-yr PV {}), {} {}/yr vs. the other path",
                    money(r.value_usd_per_year),
                    money(r.pv_5y_usd),
                    if r.delta_vs_other_usd_per_year >= 0.0 { "+" } else { "-" },
                    money(r.delta_vs_other_usd_per_year.abs())
                );
            }
            if r.capex_usd > 0.0 {
                let _ = writeln!(o, "      additional BESS capex {}", money(r.capex_usd));
            }
            if !r.resolves {
                let _ = writeln!(o, "      (does not resolve the rule on its own)");
            }
        }
    }
    let fd = &rep.financial_drag;
    let _ = writeln!(o, "\nFINANCIAL DRAG");
    let _ = writeln!(
        o,
        "  security {} + study fee {}; working-capital carry {} ({:.1} full-balance months); forfeiture exposure {} (expected {}); SLA liquidated damages {}/yr",
        money(fd.security_locked_usd),
        money(fd.study_fee_usd),
        money(fd.working_capital_carry_usd),
        fd.security_months_equivalent,
        money(fd.forfeiture_exposure_usd),
        money(fd.expected_forfeiture_usd),
        money(fd.sla_ld_exposure_usd_per_year)
    );
    let _ = writeln!(
        o,
        "  investor NPV {} -> {} after security carry and expected forfeiture",
        money(d.run.inv.npv),
        money(crate::invest::npv_net_of_compliance(&d.run.inv, fd))
    );
    let c = &d.curve;
    let _ = writeln!(o, "\nSHEDDING CAPABILITY CURVE (certified MW that keeps every SLA)");
    for p in &c.points {
        let _ = writeln!(
            o,
            "  {:>4.0} min  {:>8.2} MW certified   shadow price ${:>8.2}/MWh   displaces {:>7.2} MWh of BESS",
            p.duration_min, p.certified_shed_mw, p.marginal_shadow_price_per_mwh, p.bess_mwh_displaced
        );
    }
    let _ = writeln!(
        o,
        "  BESS for {:.1} MW of required shed: {:.1} MW / {:.1} MWh without shedding vs {:.1} MW / {:.1} MWh with it -> capex avoided {}",
        c.required_shed_mw,
        c.bess_mw_without_shed,
        c.bess_mwh_without_shed,
        c.bess_mw_with_shed,
        c.bess_mwh_with_shed,
        money(c.capex_avoided_usd)
    );
    let _ = writeln!(o, "\nDOSSIER\n  dossier_sha256 {}", d.sha256);
    o
}

/// Markdown audit report (for `$GITHUB_STEP_SUMMARY`, PR comments and filings).
pub fn audit_markdown(d: &Dossier, scenario_path: &str, expected: Option<ComplianceVerdict>) -> String {
    let rep = &d.report;
    let mut o = String::new();
    let icon = match rep.verdict {
        ComplianceVerdict::BankablePass => "PASS",
        ComplianceVerdict::ConditionalWarning => "WARNING",
        ComplianceVerdict::StatutoryBlocker => "BLOCKER",
    };
    let _ = writeln!(o, "## firmgap audit: `{}`\n", crate::scenario::sanitize(scenario_path));
    let _ = writeln!(o, "**Verdict: {} ({})** under {} (`{}`)", rep.verdict.as_str(), icon, rep.rulepack.name(), rep.rulepack_id);
    if let Some(e) = expected {
        let _ = writeln!(
            o,
            "\nExpected verdict declared in the scenario: `{}` ({})",
            e.as_str(),
            if e == rep.verdict { "matches" } else { "MISMATCH" }
        );
    }
    let _ = writeln!(o, "\n| Rule | Severity | Status | Observed | Limit | Summary |\n|---|---|---|---:|---:|---|");
    for c in &rep.checks {
        let f = |v: Option<f64>| v.map_or(String::new(), |x| crate::compliance::fx(x, 2));
        let _ = writeln!(
            o,
            "| `{}` | {} | {} | {} | {} | {} |",
            c.rule_id.code(),
            c.severity.as_str(),
            if c.passed { "pass" } else { "**FAIL**" },
            f(c.observed),
            f(c.limit),
            c.summary.replace('|', "/")
        );
    }
    if !rep.remediations.is_empty() {
        let _ = writeln!(o, "\n### Path to compliance\n");
        for r in &rep.remediations {
            let _ =
                writeln!(o, "- {}**{}** (`{}`): {}", if r.recommended { "Recommended: " } else { "" }, r.title, r.rule_id.code(), r.action);
        }
    }
    let fd = &rep.financial_drag;
    let _ = writeln!(
        o,
        "\nSecurity posted {}; carry {}; expected forfeiture {}; SLA damages {}/yr",
        money(fd.security_locked_usd),
        money(fd.working_capital_carry_usd),
        money(fd.expected_forfeiture_usd),
        money(fd.sla_ld_exposure_usd_per_year)
    );
    let _ = writeln!(
        o,
        "\n`dossier_sha256` {}  \n`scenario_sha256` {}  \nReproduce: `{}`",
        d.sha256,
        d.scenario_sha256,
        d.value.get("repro_command").and_then(crate::canon::Value::as_str).unwrap_or("")
    );
    o
}

/// Plain-text bankability matrix.
pub fn compare_text(c: &crate::portfolio::Compare, scenario_path: &str) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} - global bankability matrix for {}", crate::VERSION, crate::scenario::sanitize(scenario_path));
    let _ = writeln!(
        o,
        "\n{:>4}  {:<12} {:<19} {:>8} {:>8}  {:>10}  failed rules",
        "rank", "rulepack", "verdict", "blockers", "warnings", "drag"
    );
    for r in &c.rows {
        let _ = writeln!(
            o,
            "{:>4}  {:<12} {:<19} {:>8} {:>8}  {:>10}  {}",
            r.rank,
            r.rulepack.cli_id(),
            r.verdict.as_str(),
            r.blockers,
            r.warnings,
            money(r.total_drag_usd),
            if r.failed_rules.is_empty() { "-".to_string() } else { r.failed_rules.join(", ") }
        );
    }
    let _ = writeln!(o, "\nscenario_sha256 {}\nmatrix_sha256   {}", c.scenario_sha256, c.sha256);
    o
}

/// Plain-text batch summary.
pub fn batch_text(b: &crate::portfolio::Batch) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} - batch audit of {} scenario files", crate::VERSION, b.sites.len());
    for st in &b.sites {
        match &st.error {
            Some(e) => {
                let _ = writeln!(o, "  {:<48} ERROR {}", st.file, crate::scenario::sanitize(e));
            }
            None => {
                for r in &st.results {
                    let _ = writeln!(
                        o,
                        "  {:<48} {:<12} {:<19}{} {}",
                        st.file,
                        r.rulepack.cli_id(),
                        r.verdict.as_str(),
                        if r.unexpected { " UNEXPECTED" } else { "" },
                        &r.dossier_sha256[..16.min(r.dossier_sha256.len())]
                    );
                }
            }
        }
    }
    let _ = writeln!(o, "\n{} unexpected, {} errors, batch_sha256 {}", b.unexpected(), b.errors(), b.sha256);
    o
}

/// Rulepack registry listing.
pub fn rulepacks_text() -> String {
    let mut o = String::new();
    let _ = writeln!(o, "firmgap {} rulepacks (screening tool, not legal advice; see docs/COMPLIANCE.md)\n", crate::VERSION);
    for rp in crate::compliance::JurisdictionRulepack::ALL {
        let _ = writeln!(o, "{:<12} {:<30} {:<26} {}", rp.cli_id(), rp.id(), rp.status().as_str(), rp.name());
        for r in rp.rules() {
            let _ = writeln!(o, "    {:<26} {}", r.code(), r.citation());
        }
    }
    o
}

/// Rulepack registry as canonical JSON (conforms to `schemas/compliance_rulepack.schema.json` per entry).
pub fn rulepacks_json() -> String {
    use crate::canon::{canonical, obj, s, Value};
    let list = crate::compliance::JurisdictionRulepack::ALL
        .iter()
        .map(|rp| {
            obj(vec![
                ("id", s(rp.id())),
                ("jurisdiction", s(rp.cli_id())),
                ("name", s(rp.name())),
                ("status", s(rp.status().as_str())),
                (
                    "rules",
                    Value::Arr(rp.rules().iter().map(|r| obj(vec![("rule_id", s(r.code())), ("citation", s(r.citation()))])).collect()),
                ),
            ])
        })
        .collect();
    canonical(&obj(vec![("engine_version", s(crate::VERSION)), ("rulepacks", Value::Arr(list))])) + "\n"
}
