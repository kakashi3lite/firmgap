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

// ---------------- model 0.3: shedding curve, SB 6 security, SHA-256, dossier integrity ----------------

fn site_with(pairs: &[(&str, &str)]) -> firmgap::SiteInputs {
    let mut s = firmgap::SiteInputs::default();
    for (k, v) in pairs {
        s.set(k, v).unwrap();
    }
    s
}

#[test]
fn shedding_curve_is_monotonically_non_increasing_in_duration() {
    let p = Params::default();
    for u_min in ["0", "0.2", "0.4", "0.7", "1"] {
        for slack in ["0", "0.1", "0.5", "1", "2", "4", "12"] {
            for share in ["0", "0.2", "0.4", "0.8"] {
                for beta in ["0.3", "0.75", "1"] {
                    let st =
                        site_with(&[("u_min", u_min), ("sla_slack_hours", slack), ("flexible_share", share), ("beta", beta)]).resolve(&p);
                    let c = firmgap::shed_curve(&st, &p);
                    for w in c.points.windows(2) {
                        assert!(
                            w[1].certified_shed_mw <= w[0].certified_shed_mw + 1e-12,
                            "u_min {u_min} slack {slack} share {share} beta {beta}: {:?}",
                            c.points
                        );
                    }
                    for pt in &c.points {
                        assert!(pt.certified_shed_mw >= 0.0 && pt.certified_shed_mw <= st.gross_peak_mw + 1e-9);
                        assert!(pt.target_shed_mw <= pt.certified_shed_mw + 1e-12);
                    }
                    assert!(c.capex_avoided_usd >= -1e-6, "shedding can only reduce BESS capex");
                }
            }
        }
    }
}

#[test]
fn sb6_security_posting_is_invariant_to_bess_size() {
    use firmgap::compliance::{compliance_report, JurisdictionRulepack};
    let site = site_with(&[("gross_peak_mw", "100")]);
    let mut seen = None;
    for mw in [0.0, 5.0, 23.0, 60.0, 200.0] {
        let p = Params { target_mw: 100.0, interim_mw: 60.0, ceiling_mw: 60.0, bess_mw: mw, ..Params::default() };
        let r = run(&p, false).unwrap();
        let rep = compliance_report(JurisdictionRulepack::ErcotSb6_2026, &site.resolve(&p), &p, &r);
        let sec = rep.financial_drag.security_locked_usd;
        assert_eq!(sec, 5_000_000.0, "security uses gross requested peak, not net of storage");
        assert_eq!(*seen.get_or_insert(sec), sec);
    }
}

#[test]
fn sha256_vectors_and_dossier_determinism() {
    use firmgap::audit_bundle::build_dossier;
    use firmgap::compliance::JurisdictionRulepack;
    assert_eq!(firmgap::sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(firmgap::sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    let p = Params::default();
    let site = site_with(&[("gross_peak_mw", "80")]);
    let a = build_dossier(JurisdictionRulepack::ErcotSb6_2026, &p, &site, "x.toml").unwrap();
    let b = build_dossier(JurisdictionRulepack::ErcotSb6_2026, &p, &site, "x.toml").unwrap();
    assert_eq!(a.text, b.text);
    assert_eq!(a.sha256, b.sha256);
    assert!(firmgap::verify_text(&a.text).unwrap().ok);
}

#[test]
fn tampering_any_single_byte_fails_verification() {
    use firmgap::audit_bundle::build_dossier;
    use firmgap::compliance::JurisdictionRulepack;
    let p = Params::default();
    let d = build_dossier(JurisdictionRulepack::ErcotSb6_2026, &p, &site_with(&[("gross_peak_mw", "90")]), "t.toml").unwrap();
    let bytes = d.text.as_bytes();
    // the final newline is outside the JSON value and is not covered by the hash
    for i in 0..bytes.len() - 1 {
        let mut t = bytes.to_vec();
        t[i] = if t[i] == b'x' { b'y' } else { b'x' };
        let text = String::from_utf8(t).unwrap();
        let intact = firmgap::verify_text(&text).map(|v| v.ok).unwrap_or(false);
        assert!(!intact, "tampering byte {i} ({:?}) was not detected", bytes[i] as char);
    }
}

// ---------------- model 0.4: registry, portfolio, hostile input ----------------

#[test]
fn every_rulepack_evaluates_every_listed_rule_and_nothing_else() {
    use firmgap::compliance::{compliance_report, JurisdictionRulepack};
    let p = Params::default();
    let r = run(&p, true).unwrap();
    for gross in ["1", "8", "45", "100"] {
        let st = site_with(&[("gross_peak_mw", gross), ("backup_gen_mw", "5")]).resolve(&p);
        for rp in JurisdictionRulepack::ALL {
            let rep = compliance_report(rp, &st, &p, &r);
            assert_eq!(rep.rulepack_id, rp.id());
            for c in &rep.checks {
                assert!(rp.rules().contains(&c.rule_id), "{} emitted {} which is not in its rule list", rp.cli_id(), c.rule_id.code());
                assert!(c.summary.is_ascii() && !c.summary.is_empty());
                if let Some(v) = c.observed {
                    assert!(v.is_finite());
                }
            }
            let fd = rep.financial_drag;
            for v in [fd.security_locked_usd, fd.working_capital_carry_usd, fd.forfeiture_exposure_usd, fd.total_drag_usd] {
                assert!(v.is_finite() && v >= 0.0);
            }
        }
    }
}

#[test]
fn applying_a_resolving_remediation_turns_its_rule_green() {
    use firmgap::compliance::{compliance_report, ChangeValue, JurisdictionRulepack};
    let base = [
        ("gross_peak_mw", "100"),
        ("backup_gen_mw", "40"),
        ("pue", "1.3"),
        ("green_energy_share", "0.3"),
        ("hourly_renewable_match", "0.5"),
    ];
    let p = Params { target_mw: 100.0, interim_mw: 60.0, ceiling_mw: 60.0, ..Params::default() };
    let r = run(&p, true).unwrap();
    let mut applied = 0;
    for rp in JurisdictionRulepack::ALL {
        let site = site_with(&base);
        let rep = compliance_report(rp, &site.resolve(&p), &p, &r);
        for o in rep.remediations.iter().filter(|o| o.resolves) {
            let mut s2 = site.clone();
            let mut p2 = p.clone();
            for (k, v) in &o.changes {
                match v {
                    ChangeValue::Num(x) if firmgap::compliance::SiteInputs::is_site_key(k) => s2.set_value(k, *x).unwrap(),
                    ChangeValue::Num(x) => p2.set_f64(k, *x).unwrap(),
                    ChangeValue::Bool(b) => s2.set_value(k, if *b { 1.0 } else { 0.0 }).unwrap(),
                }
            }
            let r2 = run(&p2, true).unwrap();
            let rep2 = compliance_report(rp, &s2.resolve(&p2), &p2, &r2);
            let after = rep2.checks.iter().find(|c| c.rule_id == o.rule_id).unwrap();
            assert!(after.passed, "{}: {} did not resolve {}", rp.cli_id(), o.id, o.rule_id.code());
            applied += 1;
        }
    }
    assert!(applied >= 3, "expected several resolving remediations, got {applied}");
}

#[test]
fn batch_audit_is_deterministic_and_bounded() {
    use firmgap::{batch_audit, BatchSelection};
    let a = batch_audit("scenarios", BatchSelection::All).unwrap();
    let b = batch_audit("scenarios/", BatchSelection::All).unwrap();
    assert_eq!(a.text, b.text, "trailing slash must not change the batch");
    assert_eq!(a.errors(), 0);
    assert_eq!(a.sites.len(), 7);
    assert!(a.sites.iter().all(|s| s.results.len() == firmgap::compliance::JurisdictionRulepack::ALL.len()));
    // the declared run matches single-file audits byte for byte
    let d = batch_audit("scenarios", BatchSelection::Declared).unwrap();
    let declared: Vec<_> = d.sites.iter().filter(|s| s.error.is_none()).collect();
    assert_eq!(declared.len(), 3, "three bundled scenarios declare a jurisdiction");
    assert_eq!(d.unexpected(), 0, "declared verdicts must match expected_verdict");
    assert!(batch_audit("definitely-not-a-dir", BatchSelection::All).is_err());
}

#[test]
fn malformed_and_extreme_inputs_fail_without_panicking() {
    use firmgap::canon::parse;
    for txt in ["{", "[[[[", "\u{0}", "{\"a\":1e400}", "\"\\ud800\"", "-", "1e", "{\"a\":[1,2,]}"] {
        assert!(parse(txt).is_err());
        assert!(!matches!(firmgap::verify_text(txt), Ok(v) if v.ok));
    }
    for tok in ["", "#s=", "#s=****", "#s=e30", "s=AAAA", "https://x/#s=eyJ2IjoxfQ"] {
        assert!(firmgap::share_decode(tok).is_err(), "accepted {tok:?}");
    }
    let mut s = firmgap::SiteInputs::default();
    for (k, v) in [
        ("gross_peak_mw", "NaN"),
        ("gross_peak_mw", "inf"),
        ("gross_peak_mw", "1e309"),
        ("pue", "0.5"),
        ("backup_tier4", "yes"),
        ("nope", "1"),
    ] {
        assert!(s.set(k, v).is_err(), "{k}={v} accepted");
    }
}
