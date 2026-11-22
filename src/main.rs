// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! `firmgap` command-line interface.
//!
//! Inputs are layered: case defaults -> `--preset` -> `--config` file ->
//! repeated `--set key=value`. The binary reads at most one file (the
//! scenario), writes only to stdout/stderr, opens no network sockets, spawns
//! no processes and reads no environment variables.

#![forbid(unsafe_code)]

use firmgap::{guard, optimise, presets, report, robust, run, scenario, stress, sweep, worlds, Error, Params};
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
    -c, --config <FILE>     Load a key = value scenario file (TOML subset)
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
";

#[derive(Debug, Default)]
struct Cli {
    command: String,
    preset: Option<String>,
    world: Option<String>,
    design: Option<String>,
    years: Option<usize>,
    config: Option<String>,
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
            "-c" | "--config" => cli.config = Some(value("--config")?),
            "-s" | "--set" => {
                let kv = value("--set")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| Error::Usage(format!("--set expects KEY=VALUE, got '{}'", scenario::sanitize(&kv))))?;
                cli.sets.push((k.trim().to_string(), v.trim().to_string()));
            }
            cmd @ ("run" | "optimise" | "optimize" | "sweep" | "params" | "presets" | "diligence" | "robust" | "stress" | "montecarlo"
            | "worlds")
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
    let mut p = Params::default();
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
        scenario::apply(&mut p, &pairs)?;
        label = format!("{label} + config");
    }
    for (k, v) in &cli.sets {
        p.set(k, v)?;
    }
    if !cli.sets.is_empty() {
        label = format!("{label} + {} override(s)", cli.sets.len());
    }
    p.validate()?;
    Ok((p, label))
}

fn execute(cli: &Cli) -> Result<String, Error> {
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
    let result = match args {
        Ok(args) => parse_args(&args).and_then(|cli| execute(&cli)),
        Err(_) => Err(Error::Usage("arguments must be valid UTF-8".into())),
    };
    match result {
        Ok(out) => {
            let mut stdout = std::io::stdout().lock();
            // A closed pipe (e.g. `| head`) is not an error worth a panic.
            let _ = stdout.write_all(out.as_bytes());
            let _ = stdout.flush();
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}
