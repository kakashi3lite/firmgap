// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! `firmgap` command-line interface.
//!
//! Inputs are layered: case defaults -> `--preset` -> `--config` file ->
//! repeated `--set key=value`. The binary reads only the files it is given (a
//! scenario, a dossier, or the `*.toml` files directly inside a `--dir`), writes
//! only to stdout/stderr and the `--out` file, opens no network sockets, spawns no
//! processes and reads no environment variables.
//!
//! Exit codes: 0 ok, 1 dossier integrity failure, 2 usage or input error,
//! 3 compliance gate failure (`--fail-on-blocker`).

#![forbid(unsafe_code)]

use firmgap::compliance::{ComplianceVerdict, JurisdictionRulepack, SiteInputs};
use firmgap::portfolio::{self, BatchSelection};
use firmgap::{audit_bundle, guard, optimise, presets, report, robust, run, scenario, stress, sweep, worlds, Error, Params};
use std::io::Write;
use std::process::ExitCode;

const HELP: &str = "\
firmgap - interim non-firm grid energization & curtailment economics for AI data centers

USAGE:
    firmgap <COMMAND> [OPTIONS]

COMMANDS:
    diligence   Investment-committee report: KPIs, investor NPV, weather-year Monte Carlo, guardrails, verdict
    run         Simulate one configuration and report KPIs, investor view, pathways and dispatch
    robust      Robust optimiser: best NPV design compliant in >= 90% of weather years
    stress      Stress suite: design x world matrix, NPV tornado, break-evens, compliance cliffs
    montecarlo  Weather-year Monte Carlo for the current design
    audit       Compliance rulepack + shedding curve + tamper-evident SHA-256 dossier (or --verify / --share)
    compare-jurisdictions  Rank one scenario under every rulepack (global bankability matrix)
    batch-audit Audit every *.toml in --dir under one, all or the declared rulepack (portfolio sweep)
    rulepacks   List rulepacks, their legal status and the rules each one evaluates
    optimise    v0.1 least-cost search on the base year (case all-in cost objective)
    sweep       BESS power frontier (0-30 MW) at the current duration and hold
    worlds      List world scenarios and bridge designs
    params      List every parameter with default, range and unit
    presets     List built-in v0.1 scenarios
    help        Show this message

OPTIONS:
    -w, --world <ID>        Apply a world scenario (stated, base, harsh, fasttrack, crunch, tariff, workload)
    -d, --design <ID>       Apply a bridge design (robust, edge, eng, sw, diesel)
    -p, --preset <NAME>     Start from a built-in v0.1 scenario (default: case)
    -c, --config <FILE>     Load a key = value scenario file (TOML subset); alias --scenario
    -j, --jurisdiction <ID> Rulepack: ercot-sb6, ferc-rm26-4, va-sb253, uk-neso, ie-cru, es-rd, sg-gdc, eu-eed
                            (batch-audit also: all, declared). Optional when the scenario declares one
        --format <F>        text (default), json or markdown
        --dir <DIR>         Directory of scenario files (batch-audit; not recursive)
        --share <TOKEN|URL> Audit a web proof link (#s=...); fails if its embedded hash does not match
        --fail-on-blocker   Exit 3 on a StatutoryBlocker not declared via expected_verdict, or a verdict mismatch
        --timings           Print elapsed wall time to stderr
    -o, --out <FILE>        Write the audit dossier JSON to FILE
        --verify <FILE>     Verify a dossier's SHA-256 (exit 0 = intact, 1 = tampered)
        --expect <SHA256>   With --verify: also require this out-of-band hash
    -s, --set <KEY=VALUE>   Override one parameter (repeatable)
    -n, --years <N>         Monte Carlo weather years (default: mc_seeds = 24)
        --json              Machine-readable output
        --events            Include the per-event ledger (run)
        --fixed-hold        Do not search throttle-hold policies (optimise)
    -V, --version           Print version

Inputs are layered: case defaults -> preset -> world -> design -> config file -> --set.

EXAMPLES:
    firmgap diligence --world base --design robust
    firmgap robust --world base
    firmgap stress --world base --design robust --json
    firmgap montecarlo --world harsh --design eng -n 40
    firmgap run --preset right-sized --events
    firmgap audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --jurisdiction ercot-sb6 --out dossier.json
    firmgap audit --verify dossier.json
    firmgap compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml --format markdown
    firmgap batch-audit --dir scenarios --jurisdiction all --format markdown
";

#[derive(Debug, Default)]
struct Cli {
    command: String,
    preset: Option<String>,
    world: Option<String>,
    design: Option<String>,
    years: Option<usize>,
    config: Option<String>,
    jurisdiction: Option<String>,
    out: Option<String>,
    verify: Option<String>,
    expect: Option<String>,
    format: Option<String>,
    dir: Option<String>,
    share: Option<String>,
    fail_on_blocker: bool,
    timings: bool,
    sets: Vec<(String, String)>,
    json: bool,
    events: bool,
    fixed_hold: bool,
}

const MAX_ARGS: usize = 256;

fn parse_args(args: &[String]) -> Result<Cli, Error> {
    if args.len() > MAX_ARGS {
        return Err(Error::Usage(format!("too many arguments (max {MAX_ARGS})")));
    }
    let mut cli = Cli::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut value =
            |flag: &str| -> Result<String, Error> { it.next().cloned().ok_or_else(|| Error::Usage(format!("{flag} needs a value"))) };
        match a.as_str() {
            "-h" | "--help" | "help" => cli.command = "help".into(),
            "-V" | "--version" => cli.command = "version".into(),
            "--json" => cli.json = true,
            "--events" => cli.events = true,
            "--fixed-hold" => cli.fixed_hold = true,
            "-p" | "--preset" => cli.preset = Some(value("--preset")?),
            "-w" | "--world" => cli.world = Some(value("--world")?),
            "-d" | "--design" => cli.design = Some(value("--design")?),
            "-n" | "--years" => {
                let v = value("--years")?;
                let n: usize =
                    v.parse().map_err(|_| Error::Usage(format!("--years expects an integer, got '{}'", scenario::sanitize(&v))))?;
                if !(2..=1000).contains(&n) {
                    return Err(Error::Usage("--years must be between 2 and 1000".into()));
                }
                cli.years = Some(n);
            }
            "-c" | "--config" | "--scenario" => cli.config = Some(value("--scenario")?),
            "-j" | "--jurisdiction" => cli.jurisdiction = Some(value("--jurisdiction")?),
            "-o" | "--out" => cli.out = Some(value("--out")?),
            "--verify" => cli.verify = Some(value("--verify")?),
            "--expect" => cli.expect = Some(value("--expect")?),
            "--format" => {
                let f = value("--format")?;
                if !["text", "json", "markdown", "md"].contains(&f.as_str()) {
                    return Err(Error::Usage(format!("--format must be text, json or markdown, got '{}'", scenario::sanitize(&f))));
                }
                cli.format = Some(if f == "md" { "markdown".into() } else { f });
            }
            "--dir" => cli.dir = Some(value("--dir")?),
            "--share" => cli.share = Some(value("--share")?),
            "--fail-on-blocker" => cli.fail_on_blocker = true,
            "--timings" => cli.timings = true,
            "-s" | "--set" => {
                let kv = value("--set")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| Error::Usage(format!("--set expects KEY=VALUE, got '{}'", scenario::sanitize(&kv))))?;
                cli.sets.push((k.trim().to_string(), v.trim().to_string()));
            }
            cmd @ ("run"
            | "optimise"
            | "optimize"
            | "sweep"
            | "params"
            | "presets"
            | "diligence"
            | "robust"
            | "stress"
            | "montecarlo"
            | "worlds"
            | "audit"
            | "compare-jurisdictions"
            | "batch-audit"
            | "rulepacks")
                if cli.command.is_empty() =>
            {
                cli.command = if cmd == "optimize" { "optimise".into() } else { cmd.to_string() };
            }
            other => {
                return Err(Error::Usage(format!("unrecognised argument '{}' (try `firmgap help`)", scenario::sanitize(other))));
            }
        }
    }
    if cli.command.is_empty() {
        cli.command = "help".into();
    }
    Ok(cli)
}

fn build_params(cli: &Cli) -> Result<(Params, String), Error> {
    build_params_site(cli).map(|(p, _, label)| (p, label))
}

fn build_params_site(cli: &Cli) -> Result<(Params, SiteInputs, String), Error> {
    let mut p = Params::default();
    let mut site = SiteInputs::default();
    let mut label = String::from("case");
    if let Some(name) = &cli.preset {
        presets::apply(&mut p, name)?;
        label = name.clone();
    }
    if let Some(id) = &cli.world {
        p = worlds::apply_world(&p, id)?;
        label = format!("{label} / world {id}");
    }
    if let Some(id) = &cli.design {
        p = worlds::apply_design(&p, id)?;
        label = format!("{label} / design {id}");
    }
    if let Some(path) = &cli.config {
        let text = scenario::read_file(path)?;
        let pairs = scenario::parse(&text)?;
        scenario::apply_with_site(&mut p, &mut site, &pairs)?;
        label = format!("{label} + config");
    }
    for (k, v) in &cli.sets {
        if SiteInputs::is_site_key(k) {
            site.set(k, v)?;
        } else {
            p.set(k, v)?;
        }
    }
    if !cli.sets.is_empty() {
        label = format!("{label} + {} override(s)", cli.sets.len());
    }
    p.validate()?;
    Ok((p, site, label))
}

/// Output of a command plus its exit code (non-zero codes still print their report).
struct Outcome {
    text: String,
    code: u8,
}

impl From<String> for Outcome {
    fn from(text: String) -> Self {
        Outcome { text, code: 0 }
    }
}

fn format_of(cli: &Cli) -> &str {
    if cli.json {
        "json"
    } else {
        cli.format.as_deref().unwrap_or("text")
    }
}

fn write_out(cli: &Cli, text: &str) -> Result<(), Error> {
    if let Some(out) = &cli.out {
        std::fs::write(out, text.as_bytes()).map_err(|e| Error::Io(format!("cannot write output file: {}", e.kind())))?;
    }
    Ok(())
}

fn scenario_meta(cli: &Cli) -> Result<scenario::ScenarioMeta, Error> {
    match &cli.config {
        Some(path) => scenario::metadata(&scenario::parse(&scenario::read_file(path)?)?),
        None => Ok(scenario::ScenarioMeta::default()),
    }
}

fn verify_cmd(cli: &Cli, path: &str) -> Result<Outcome, Error> {
    let text = audit_bundle::read_dossier(path)?;
    let v = audit_bundle::verify_text(&text)?;
    if !v.ok {
        return Err(Error::Integrity(format!(
            "hash mismatch: file claims {} but the canonical body hashes to {} (the dossier was modified after it was generated)",
            v.claimed, v.actual
        )));
    }
    // The embedded hash only proves the file is self-consistent. Comparing against a hash recorded
    // out of band (IC minutes, a signed commit, an email) is what makes it tamper-evident.
    if let Some(want) = &cli.expect {
        if !want.trim().eq_ignore_ascii_case(&v.actual) {
            return Err(Error::Integrity(format!(
                "dossier is self-consistent but its hash {} does not match the expected {}",
                v.actual,
                scenario::sanitize(want.trim())
            )));
        }
    }
    Ok(Outcome::from(format!(
        "OK: dossier is tamper-free{}\n  dossier_sha256 {}\n  rulepack {}  engine {}\n",
        if cli.expect.is_some() { " and matches the expected hash" } else { " (self-consistent; pass --expect <sha256> to pin it)" },
        v.actual,
        scenario::sanitize(&v.rulepack_id),
        scenario::sanitize(&v.engine_version)
    )))
}

fn render_dossier(cli: &Cli, d: &audit_bundle::Dossier, path: &str, meta: scenario::ScenarioMeta) -> Result<Outcome, Error> {
    write_out(cli, &d.text)?;
    let expected_mismatch = meta.jurisdiction == Some(d.report.rulepack) && meta.expected_verdict.is_some_and(|e| e != d.report.verdict);
    let declared_blocker =
        meta.jurisdiction == Some(d.report.rulepack) && meta.expected_verdict == Some(ComplianceVerdict::StatutoryBlocker);
    let gate_fail =
        cli.fail_on_blocker && (expected_mismatch || (d.report.verdict == ComplianceVerdict::StatutoryBlocker && !declared_blocker));
    let mut text = match format_of(cli) {
        "json" => d.text.clone(),
        "markdown" => report::audit_markdown(d, path, meta.expected_verdict.filter(|_| meta.jurisdiction == Some(d.report.rulepack))),
        _ => {
            let mut t = report::audit_text(d);
            if let Some(out) = &cli.out {
                t.push_str(&format!(
                    "\nwrote {}\nverify with: firmgap audit --verify {}\n",
                    scenario::sanitize(out),
                    scenario::sanitize(out)
                ));
            }
            t
        }
    };
    if gate_fail && format_of(cli) != "json" {
        text.push_str(if expected_mismatch {
            "\nCOMPLIANCE GATE: FAIL (verdict differs from the scenario's expected_verdict)\n"
        } else {
            "\nCOMPLIANCE GATE: FAIL (StatutoryBlocker; declare expected_verdict = \"StatutoryBlocker\" for a deliberate fixture)\n"
        });
    }
    Ok(Outcome { text, code: if gate_fail { 3 } else { 0 } })
}

fn resolve_rulepack(cli: &Cli, meta: scenario::ScenarioMeta) -> Result<JurisdictionRulepack, Error> {
    match (&cli.jurisdiction, meta.jurisdiction) {
        (Some(j), _) => JurisdictionRulepack::parse(j).ok_or_else(|| {
            Error::Usage(format!("unknown jurisdiction '{}' (run `firmgap rulepacks` for the list)", scenario::sanitize(j)))
        }),
        (None, Some(rp)) => Ok(rp),
        (None, None) => Err(Error::Usage(
            "audit needs --jurisdiction <id> (or a `jurisdiction` key in the scenario, or --verify <dossier.json>)".into(),
        )),
    }
}

fn audit(cli: &Cli) -> Result<Outcome, Error> {
    if let Some(path) = &cli.verify {
        return verify_cmd(cli, path);
    }
    if let Some(tok) = &cli.share {
        if cli.config.is_some() {
            return Err(Error::Usage("use either --share or --scenario, not both".into()));
        }
        let sc = portfolio::share_decode(tok)?;
        if let Some(j) = &cli.jurisdiction {
            if JurisdictionRulepack::parse(j) != Some(sc.rulepack) {
                return Err(Error::Usage("--jurisdiction differs from the jurisdiction inside the share token".into()));
            }
        }
        let d = audit_bundle::build_dossier(sc.rulepack, &sc.params, &sc.site, &sc.path)?;
        if let Some(h) = &sc.expected_sha256 {
            if *h != d.sha256 {
                return Err(Error::Integrity(format!("share token claims dossier {h} but its inputs reproduce {}", d.sha256)));
            }
        }
        let path = sc.path.clone();
        return render_dossier(cli, &d, &path, scenario::ScenarioMeta::default());
    }
    let path = cli.config.as_deref().ok_or_else(|| Error::Usage("audit needs --scenario <file.toml> (or --share / --verify)".into()))?;
    let meta = scenario_meta(cli)?;
    let rp = resolve_rulepack(cli, meta)?;
    let (p, site, _) = build_params_site(cli)?;
    let d = audit_bundle::build_dossier(rp, &p, &site, path)?;
    render_dossier(cli, &d, path, meta)
}

fn compare_cmd(cli: &Cli) -> Result<Outcome, Error> {
    let path = cli.config.as_deref().ok_or_else(|| Error::Usage("compare-jurisdictions needs --scenario <file.toml>".into()))?;
    let (p, site, _) = build_params_site(cli)?;
    let c = portfolio::compare_jurisdictions(&p, &site, path)?;
    write_out(cli, &c.text)?;
    Ok(Outcome::from(match format_of(cli) {
        "json" => c.text,
        "markdown" => portfolio::compare_markdown(&c, path),
        _ => report::compare_text(&c, path),
    }))
}

fn batch_cmd(cli: &Cli) -> Result<Outcome, Error> {
    let dir = cli.dir.as_deref().ok_or_else(|| Error::Usage("batch-audit needs --dir <scenarios_dir>".into()))?;
    let sel_txt = cli.jurisdiction.as_deref().unwrap_or("all");
    let sel = BatchSelection::parse(sel_txt).ok_or_else(|| {
        Error::Usage(format!("unknown jurisdiction '{}' (use all, declared or a rulepack id)", scenario::sanitize(sel_txt)))
    })?;
    let b = portfolio::batch_audit(dir, sel)?;
    write_out(cli, &b.text)?;
    let gate_fail = cli.fail_on_blocker && (b.unexpected() > 0 || b.errors() > 0);
    let mut text = match format_of(cli) {
        "json" => b.text.clone(),
        "markdown" => portfolio::batch_markdown(&b),
        _ => report::batch_text(&b),
    };
    if gate_fail && format_of(cli) != "json" {
        text.push_str(&format!("\nCOMPLIANCE GATE: FAIL ({} unexpected results, {} files with errors)\n", b.unexpected(), b.errors()));
    }
    let code = if gate_fail {
        3
    } else if b.errors() > 0 {
        2
    } else {
        0
    };
    Ok(Outcome { text, code })
}

fn execute(cli: &Cli) -> Result<Outcome, Error> {
    match cli.command.as_str() {
        "audit" => audit(cli),
        "compare-jurisdictions" => compare_cmd(cli),
        "batch-audit" => batch_cmd(cli),
        "rulepacks" => Ok(Outcome::from(if cli.json || cli.format.as_deref() == Some("json") {
            report::rulepacks_json()
        } else {
            report::rulepacks_text()
        })),
        _ => execute_classic(cli).map(Outcome::from),
    }
}

fn execute_classic(cli: &Cli) -> Result<String, Error> {
    match cli.command.as_str() {
        "help" => Ok(HELP.to_string()),
        "version" => Ok(format!("firmgap {}\n", firmgap::VERSION)),
        "params" => Ok(report::params_text()),
        "presets" => Ok(report::presets_text()),
        "worlds" => Ok(report::worlds_text()),
        "diligence" => {
            let (p, label) = build_params(cli)?;
            let r = run(&p, false)?;
            let mc = robust::monte_carlo(&p, cli.years.unwrap_or_else(|| p.mc_years()))?;
            let g = guard::guardrails(&p, &r, Some(&mc));
            Ok(if cli.json { report::diligence_json(&r, &mc, &g, &label) + "\n" } else { report::diligence_text(&r, &mc, &g, &label) })
        }
        "montecarlo" => {
            let (p, _) = build_params(cli)?;
            let mc = robust::monte_carlo(&p, cli.years.unwrap_or_else(|| p.mc_years()))?;
            Ok(if cli.json { report::mc_json_doc(&mc) + "\n" } else { report::mc_text(&mc) })
        }
        "robust" => {
            let (mut p, _) = build_params(cli)?;
            if let Some(n) = cli.years {
                p.mc_seeds = n as f64;
            }
            let reference = run(&p, false)?;
            let o = robust::robust_optimise(&p)?;
            Ok(if cli.json { report::robust_json(&o) + "\n" } else { report::robust_text(&o, &reference) })
        }
        "stress" => {
            let (p, _) = build_params(cli)?;
            let s = stress::stress_suite(&p)?;
            Ok(if cli.json { report::stress_json(&s) + "\n" } else { report::stress_text(&s) })
        }
        "run" => {
            let (p, label) = build_params(cli)?;
            let r = run(&p, false)?;
            Ok(if cli.json { report::run_json(&r, &label, cli.events) + "\n" } else { report::run_text(&r, &label, cli.events) })
        }
        "optimise" => {
            let (p, _) = build_params(cli)?;
            let reference = run(&p, false)?;
            let o = optimise(&p, !cli.fixed_hold)?;
            Ok(if cli.json { report::optimise_json(&o) + "\n" } else { report::optimise_text(&o, &reference) })
        }
        "sweep" => {
            let (p, _) = build_params(cli)?;
            let rows = sweep(&p)?;
            Ok(if cli.json { report::sweep_json(&rows) + "\n" } else { report::sweep_text(&rows) })
        }
        other => Err(Error::Usage(format!("unknown command '{}'", scenario::sanitize(other)))),
    }
}

fn main() -> ExitCode {
    // `args_os` + explicit conversion: non-UTF-8 arguments are rejected
    // cleanly instead of panicking as `std::env::args` would.
    let args: Result<Vec<String>, _> = std::env::args_os().skip(1).map(|a| a.into_string()).collect();
    let started = std::time::Instant::now();
    let mut timings = false;
    let result = match args {
        Ok(args) => parse_args(&args).and_then(|cli| {
            timings = cli.timings;
            execute(&cli)
        }),
        Err(_) => Err(Error::Usage("arguments must be valid UTF-8".into())),
    };
    if timings {
        eprintln!("firmgap: elapsed {:.3} ms", started.elapsed().as_secs_f64() * 1000.0);
    }
    match result {
        Ok(out) => {
            let mut stdout = std::io::stdout().lock();
            // A closed pipe (e.g. `| head`) is not an error worth a panic.
            let _ = stdout.write_all(out.text.as_bytes());
            let _ = stdout.flush();
            ExitCode::from(out.code)
        }
        Err(e) => {
            eprintln!("error: {e}");
            // 1 = integrity failure (tampered dossier or proof link), 2 = usage / input error.
            // Exit 3 (compliance gate) is returned with a report, through `Outcome::code`.
            ExitCode::from(if matches!(e, Error::Integrity(_)) { 1 } else { 2 })
        }
    }
}
