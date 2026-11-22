// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Golden parity for models 0.3 and 0.4 (eight compliance rulepacks, shedding curve, audit dossier,
//! global bankability matrix and the shareable proof token).
//!
//! `tests/golden_audit.json` is written from the web reference by `node scripts/golden.mjs --write`.
//! Each case carries its scenario as TOML and the exact canonical dossier text the browser produces.
//! The Rust dossier must match it byte for byte, so its SHA-256 matches too. On a mismatch the test
//! prints the first differing position with context.

use firmgap::audit_bundle::{build_dossier, verify_text};
use firmgap::canon::{canonical, parse, quantize, Value};
use firmgap::compliance::{JurisdictionRulepack, SiteInputs};
use firmgap::{scenario, Params};

const GOLDEN: &str = include_str!("golden_audit.json");

fn field<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or_else(|| panic!("golden case missing '{k}'"))
}

fn first_diff(a: &str, b: &str) -> String {
    let pos = a.bytes().zip(b.bytes()).position(|(x, y)| x != y).unwrap_or(a.len().min(b.len()));
    let lo = pos.saturating_sub(120);
    format!(
        "first difference at byte {pos}\n  rust: ...{}\n  web:  ...{}",
        &a[lo..(pos + 120).min(a.len())],
        &b[lo..(pos + 120).min(b.len())]
    )
}

#[test]
fn rust_dossiers_are_byte_identical_to_the_web_reference() {
    let golden = parse(GOLDEN).expect("golden file parses");
    assert_eq!(field(&golden, "engine_version"), firmgap::VERSION, "golden file was generated for another engine version");
    let Some(Value::Arr(cases)) = golden.get("cases") else { panic!("no cases") };
    assert!(cases.len() >= 19, "every rulepack needs passing and failing golden cases");
    for c in cases {
        let id = field(c, "id");
        let pairs = scenario::parse(field(c, "toml")).unwrap_or_else(|e| panic!("{id}: {e}"));
        let mut p = Params::default();
        let mut site = SiteInputs::default();
        scenario::apply_with_site(&mut p, &mut site, &pairs).unwrap_or_else(|e| panic!("{id}: {e}"));
        p.validate().unwrap_or_else(|e| panic!("{id}: {e}"));
        let rp = JurisdictionRulepack::parse(field(c, "jurisdiction")).expect("known jurisdiction");
        let d = build_dossier(rp, &p, &site, field(c, "path")).unwrap_or_else(|e| panic!("{id}: {e}"));
        let want = field(c, "text");
        assert!(d.text == want, "{id}: dossier differs from the web reference\n{}", first_diff(&d.text, want));
        assert_eq!(d.sha256, field(c, "dossier_sha256"), "{id}");
        assert_eq!(d.scenario_sha256, field(c, "scenario_sha256"), "{id}");
        let curve_sha = firmgap::sha256_hex(canonical(&quantize(&d.curve.to_value())).as_bytes());
        assert_eq!(curve_sha, field(c, "shedding_curve_sha256"), "{id}: shedding curve hash");
        assert_eq!(d.report.verdict.as_str(), field(c, "verdict"), "{id}");
        let v = verify_text(&d.text).expect("verifies");
        assert!(v.ok, "{id}: own dossier fails verification");
    }
}

#[test]
fn expected_verdicts_for_the_b2b_scenarios() {
    let golden = parse(GOLDEN).expect("golden file parses");
    let Some(Value::Arr(cases)) = golden.get("cases") else { panic!("no cases") };
    let verdict = |id: &str| cases.iter().find(|c| field(c, "id") == id).map(|c| field(c, "verdict").to_string());
    assert_eq!(verdict("ercot-sb6-100mw-compliant").as_deref(), Some("BankablePass"));
    assert_eq!(verdict("ercot-sb6-illegal-stack").as_deref(), Some("StatutoryBlocker"));
    assert_eq!(verdict("ferc-rm26-4-colocated").as_deref(), Some("BankablePass"));
}

fn golden_section<'a>(golden: &'a Value, key: &str) -> &'a Vec<Value> {
    match golden.get(key) {
        Some(Value::Arr(a)) => a,
        _ => panic!("golden file has no '{key}' section"),
    }
}

#[test]
fn every_rulepack_has_golden_coverage() {
    let golden = parse(GOLDEN).expect("golden file parses");
    let cases = golden_section(&golden, "cases");
    for rp in JurisdictionRulepack::ALL {
        let n = cases.iter().filter(|c| field(c, "jurisdiction") == rp.cli_id()).count();
        assert!(n >= 1, "{} has no golden case", rp.cli_id());
    }
}

#[test]
fn bankability_matrix_is_byte_identical_to_the_web_reference() {
    let golden = parse(GOLDEN).expect("golden file parses");
    for c in golden_section(&golden, "compare") {
        let id = field(c, "id");
        let pairs = scenario::parse(field(c, "toml")).unwrap_or_else(|e| panic!("{id}: {e}"));
        let mut p = Params::default();
        let mut site = SiteInputs::default();
        scenario::apply_with_site(&mut p, &mut site, &pairs).unwrap_or_else(|e| panic!("{id}: {e}"));
        let m = firmgap::compare_jurisdictions(&p, &site, field(c, "path")).unwrap_or_else(|e| panic!("{id}: {e}"));
        let want = field(c, "text");
        assert!(m.text == want, "{id}: matrix differs from the web reference\n{}", first_diff(&m.text, want));
        assert_eq!(m.sha256, field(c, "matrix_sha256"), "{id}");
    }
}

#[test]
fn share_tokens_roundtrip_bit_exact() {
    let golden = parse(GOLDEN).expect("golden file parses");
    for c in golden_section(&golden, "share") {
        let id = field(c, "id");
        let token = field(c, "token");
        let url = format!("https://kakashi3lite.github.io/firmgap/#s={token}");
        let sc = firmgap::share_decode(&url).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(sc.path, field(c, "path"), "{id}");
        assert_eq!(sc.expected_sha256.as_deref(), Some(field(c, "dossier_sha256")), "{id}");
        let d = build_dossier(sc.rulepack, &sc.params, &sc.site, &sc.path).unwrap_or_else(|e| panic!("{id}: {e}"));
        let want = field(c, "text");
        assert!(d.text == want, "{id}: shared dossier differs\n{}", first_diff(&d.text, want));
        let (again, sha) = firmgap::share_encode(sc.rulepack, &sc.params, &sc.site, &sc.path).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(sha, field(c, "dossier_sha256"), "{id}");
        assert_eq!(again, token, "{id}: Rust re-encodes a different token");
    }
}
