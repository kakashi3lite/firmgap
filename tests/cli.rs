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
