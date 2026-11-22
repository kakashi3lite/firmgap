// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! End-to-end tests of the compiled `firmgap` binary, including hostile input.

use std::process::Command;

fn firmgap(args: &[&str]) -> (i32, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_firmgap")).args(args).output().expect("binary runs");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn help_and_version() {
    let (code, out, _) = firmgap(&["--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("USAGE"));
    let (code, out, _) = firmgap(&["--version"]);
    assert_eq!(code, 0);
    assert!(out.starts_with("firmgap "));
}

#[test]
fn run_case_text_report() {
    let (code, out, err) = firmgap(&["run"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("TARGET KPIs"));
    assert!(out.contains("PATHWAYS"));
}

#[test]
fn run_right_sized_json() {
    let (code, out, err) = firmgap(&["run", "--preset", "right-sized", "--json", "--events"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.trim_start().starts_with('{') && out.trim_end().ends_with('}'));
    assert!(out.contains("\"certified\":true"));
    assert!(out.contains("\"pull_forward_months\":28"));
    assert!(out.contains("\"events\":["));
}

#[test]
fn params_and_presets_list() {
    let (code, out, _) = firmgap(&["params"]);
    assert_eq!(code, 0);
    assert!(out.contains("bess_mw") && out.contains("carry"));
    let (code, out, _) = firmgap(&["presets"]);
    assert_eq!(code, 0);
    assert!(out.contains("right-sized"));
}

#[test]
fn rejects_bad_input_with_exit_code_2() {
    let cases: &[&[&str]] = &[
        &["run", "--set", "bess_mw=NaN"],
        &["run", "--set", "bess_mw=-5"],
        &["run", "--set", "unknown_key=1"],
        &["run", "--set", "no_equals_sign"],
        &["run", "--preset", "../../etc/passwd"],
        &["run", "--config", "/definitely/not/here.toml"],
        &["run", "--set"],
        &["frobnicate"],
        &["run", "--set", "interim_mw=40", "--set", "target_mw=30"],
    ];
    for args in cases {
        let (code, out, err) = firmgap(args);
        assert_eq!(code, 2, "args {args:?} should fail; stdout={out} stderr={err}");
        assert!(err.starts_with("error: "), "{err}");
        assert!(out.is_empty());
    }
}

#[test]
fn error_messages_do_not_echo_control_characters() {
    let (code, _, err) = firmgap(&["run", "--preset", "evil\u{1b}[2Jname"]);
    assert_eq!(code, 2);
    assert!(!err.contains('\u{1b}'), "terminal escape echoed: {err:?}");
}

#[test]
fn worlds_list() {
    let (code, out, _) = firmgap(&["worlds"]);
    assert_eq!(code, 0);
    assert!(out.contains("harsh") && out.contains("robust"));
}

#[test]
fn diligence_json_has_verdict_and_monte_carlo() {
    let (code, out, err) = firmgap(&["diligence", "--world", "base", "--design", "robust", "-n", "4", "--json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.trim_start().starts_with('{') && out.trim_end().ends_with('}'));
    assert!(out.contains("\"verdict\":\"CONDITIONAL\""), "{out}");
    assert!(out.contains("\"monte_carlo\":{"));
    assert!(out.contains("\"investor\":{"));
}

#[test]
fn diligence_text_flags_knife_edge_design() {
    let (code, out, err) = firmgap(&["diligence", "--world", "base", "--design", "edge", "-n", "4"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("VERDICT: NOT INVESTABLE"));
    assert!(out.contains("(CERT)"));
}

#[test]
fn montecarlo_and_bad_world() {
    let (code, out, _) = firmgap(&["montecarlo", "--world", "harsh", "--design", "robust", "-n", "3"]);
    assert_eq!(code, 0);
    assert!(out.contains("compliant"));
    let (code, _, err) = firmgap(&["run", "--world", "mars"]);
    assert_eq!(code, 2);
    assert!(err.contains("unknown world"));
    let (code, _, _) = firmgap(&["diligence", "-n", "1"]);
    assert_eq!(code, 2);
}

fn temp_path(name: &str) -> String {
    let mut p = std::env::temp_dir();
    p.push(format!("firmgap-test-{}-{name}", std::process::id()));
    p.to_string_lossy().into_owned()
}

#[test]
fn audit_writes_and_verifies_a_dossier() {
    let out = temp_path("dossier.json");
    let (code, stdout, err) =
        firmgap(&["audit", "--scenario", "scenarios/ercot-sb6-100mw-compliant.toml", "--jurisdiction", "ercot-sb6", "--out", &out]);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.contains("VERDICT: BankablePass"), "{stdout}");
    assert!(stdout.contains("dossier_sha256"));
    let (code, stdout, err) = firmgap(&["audit", "--verify", &out]);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.starts_with("OK"), "{stdout}");
    let sha = stdout.split_whitespace().skip_while(|w| *w != "dossier_sha256").nth(1).unwrap_or("").to_string();
    assert_eq!(sha.len(), 64, "{stdout}");
    let (code, _, err) = firmgap(&["audit", "--verify", &out, "--expect", &sha]);
    assert_eq!(code, 0, "{err}");
    let (code, _, err) = firmgap(&["audit", "--verify", &out, "--expect", &"0".repeat(64)]);
    assert_eq!(code, 1, "wrong expected hash must fail: {err}");
    // tamper one byte of a number
    let text = std::fs::read_to_string(&out).unwrap();
    let pos = text.find("\"gross_peak_mw\":100").expect("gross peak present") + "\"gross_peak_mw\":".len();
    let mut t = text.into_bytes();
    t[pos] = b'2';
    std::fs::write(&out, t).unwrap();
    let (code, _, err) = firmgap(&["audit", "--verify", &out]);
    assert_eq!(code, 1, "tampered dossier must exit 1: {err}");
    assert!(err.contains("hash mismatch"), "{err}");
    let _ = std::fs::remove_file(&out);
}

#[test]
fn audit_illegal_stack_reports_blocker_and_both_paths() {
    let (code, stdout, err) = firmgap(&["audit", "--scenario", "scenarios/ercot-sb6-illegal-stack.toml", "--jurisdiction", "ercot-sb6"]);
    assert_eq!(code, 0, "{err}");
    assert!(stdout.contains("VERDICT: StatutoryBlocker"), "{stdout}");
    assert!(stdout.contains("Path A: price-responsive (LMP) curtailment only"));
    assert!(stdout.contains("Path B: ERCOT reliability service only"));
    assert!(stdout.contains("[FAIL] SB6_ILLEGAL_REVENUE_STACK"));
}

#[test]
fn audit_usage_errors() {
    let (code, _, err) = firmgap(&["audit", "--scenario", "scenarios/case-study.toml"]);
    assert_eq!(code, 2, "a scenario without jurisdiction metadata needs --jurisdiction");
    assert!(err.contains("--jurisdiction"));
    let (code, _, _) = firmgap(&["audit", "--scenario", "scenarios/ercot-sb6-100mw-compliant.toml", "--jurisdiction", "mars"]);
    assert_eq!(code, 2);
    let (code, _, _) = firmgap(&["audit", "--verify", "/definitely/not/here.json"]);
    assert_eq!(code, 2);
}

#[test]
fn compare_jurisdictions_ranks_all_rulepacks() {
    let (code, out, err) =
        firmgap(&["compare-jurisdictions", "--scenario", "scenarios/ercot-sb6-100mw-compliant.toml", "--format", "markdown"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Global bankability matrix"));
    for id in ["TX-SB6", "US-FERC", "US-VA-SB253", "GB-NESO", "IE-CRU", "ES-RD-1032", "SG-IMDA", "EU-EED"] {
        assert!(out.contains(id), "missing {id}");
    }
    let (code, json, _) = firmgap(&["compare-jurisdictions", "--scenario", "scenarios/ercot-sb6-100mw-compliant.toml", "--format", "json"]);
    assert_eq!(code, 0);
    assert!(json.contains("\"matrix_sha256\":\""));
}

#[test]
fn batch_audit_all_and_gate() {
    let (code, out, err) = firmgap(&["batch-audit", "--dir", "scenarios", "--jurisdiction", "all", "--format", "markdown"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("firmgap batch audit (7 sites"), "{out}");
    let (code, out, err) = firmgap(&["batch-audit", "--dir", "scenarios", "--jurisdiction", "declared", "--fail-on-blocker"]);
    assert_eq!(code, 3, "the gate fails when files cannot be audited under 'declared' (no metadata): {err}{out}");
    assert!(out.contains("COMPLIANCE GATE: FAIL"));
    let (code, _, _) = firmgap(&["batch-audit", "--dir", "scenarios", "--jurisdiction", "mars"]);
    assert_eq!(code, 2);
}

#[test]
fn fail_on_blocker_respects_expected_verdict() {
    // the illegal-stack fixture declares expected_verdict = StatutoryBlocker: the gate passes
    let (code, out, err) =
        firmgap(&["audit", "--scenario", "scenarios/ercot-sb6-illegal-stack.toml", "--format", "markdown", "--fail-on-blocker"]);
    assert_eq!(code, 0, "{err}{out}");
    assert!(out.contains("Verdict: StatutoryBlocker"));
    // an undeclared blocker fails with exit 3 and still prints the report
    let (code, out, _) =
        firmgap(&["audit", "--scenario", "scenarios/ercot-sb6-100mw-compliant.toml", "--jurisdiction", "es-rd", "--fail-on-blocker"]);
    assert_eq!(code, 3, "{out}");
    assert!(out.contains("COMPLIANCE GATE: FAIL"));
}

#[test]
fn share_token_roundtrip_through_the_cli() {
    let golden = std::fs::read_to_string("tests/golden_audit.json").unwrap();
    let start = golden.find("\"token\": \"").unwrap() + "\"token\": \"".len();
    let token = &golden[start..start + golden[start..].find('"').unwrap()];
    let (code, out, err) = firmgap(&["audit", "--share", &format!("https://x.invalid/#s={token}"), "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with('{') && out.contains("\"dossier_sha256\""));
    // a token whose payload was altered no longer decodes or no longer matches its hash
    let mut chars: Vec<char> = token.chars().collect();
    let mid = chars.len() / 2;
    chars[mid] = if chars[mid] == 'A' { 'B' } else { 'A' };
    let bad: String = chars.into_iter().collect();
    let (code, _, _) = firmgap(&["audit", "--share", &bad]);
    assert!(code == 1 || code == 2, "tampered token must fail");
}

#[test]
fn rulepacks_listing() {
    let (code, out, _) = firmgap(&["rulepacks"]);
    assert_eq!(code, 0);
    assert!(out.contains("es-rd") && out.contains("ES_HOURLY_RENEWABLE"));
    let (code, out, _) = firmgap(&["rulepacks", "--json"]);
    assert_eq!(code, 0);
    assert!(out.contains("\"status\":\"statute-and-rule\""));
}

#[test]
fn verify_rejects_one_byte_tampering_from_tmp() {
    let out = temp_path("one-byte.json");
    let (code, _, err) = firmgap(&["audit", "--scenario", "scenarios/ferc-rm26-4-colocated.toml", "--out", &out, "--format", "json"]);
    assert_eq!(code, 0, "{err}");
    let mut bytes = std::fs::read(&out).unwrap();
    let i = bytes.len() / 2;
    bytes[i] = if bytes[i] == b'1' { b'2' } else { b'1' };
    std::fs::write(&out, &bytes).unwrap();
    let (code, _, _) = firmgap(&["audit", "--verify", &out]);
    assert!(code == 1 || code == 2, "tampered dossier accepted");
    let _ = std::fs::remove_file(&out);
}
