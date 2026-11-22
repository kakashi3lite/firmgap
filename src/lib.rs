// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! # firmgap
//!
//! Hour-by-hour simulator that prices the **"firm gap"**: the months between an
//! *interim, non-firm* grid energization and the eventual *firm* interconnection
//! of a large flexible load such as an AI-inference data center.
//!
//! It answers one question for finance and engineering together:
//!
//! > Can a battery (BESS), on-site gensets and deferrable batch compute keep the
//! > campus under the utility's curtailment ceiling, pass the sub-5-minute
//! > response test, respect compute SLAs, and still beat the cost of waiting in
//! > the interconnection queue?
//!
//! ## Pipeline
//!
//! ```text
//! Params ─► worlds (stress overlays) ─► year::build_year ─► sim::simulate x4 (derated)
//!                                                              │
//!                         economics::economics (case KPIs) ◄───┤
//!                         invest::investor (NPV, downside) ◄───┘
//!                                   │
//!     guard::guardrails ─► verdict  │  robust::monte_carlo (weather years)
//!     robust::robust_optimise       │  stress::stress_suite (matrix, tornado, break-evens)
//!                                   │
//!     compliance::compliance_report ◄── shed_curve::shed_curve ─► audit_bundle::build_dossier (SHA-256)
//! ```
//!
//! * [`year`] synthesises an 8,760-hour load and curtailment-signal year.
//! * [`sim`] dispatches throttle / BESS / genset against the ceiling and tracks
//!   the deferred-work SLA queue.
//! * [`economics`] phases capacity month by month for three pathways and
//!   computes all-in $/MWh, carry, and the breakeven carry rate.
//! * [`optimise`] finds the least-cost compliant bridge configuration (v0.1 objective).
//! * [`invest`] prices the bridge as an investment against interim-only (model v0.2).
//! * [`robust`] runs weather-year Monte Carlo and the robust (>= 90% of years) optimiser.
//! * [`stress`] and [`worlds`] provide the investor stress suite and scenario overlays.
//! * [`guard`] turns results into guardrail findings and an investment verdict.
//! * [`compliance`] evaluates versioned statutory rulepacks (ERCOT / Texas SB 6, FERC RM26-4, EU EED
//!   Art. 12) and solves counterfactual paths to compliance (model v0.3).
//! * [`shed_curve`] certifies sheddable MW by event duration (KKT GPU power-capping allocator).
//! * [`audit_bundle`] and [`canon`] build a tamper-evident SHA-256 dossier, byte-identical to the
//!   browser's.
//! * [`rulepacks_intl`] adds Virginia, GB, Ireland, Spain and Singapore (model v0.4).
//! * [`portfolio`] ranks all rulepacks (`compare-jurisdictions`), sweeps directories (`batch-audit`)
//!   and encodes / decodes the shareable proof token.
//!
//! The numerical kernel is a faithful port of the reference JavaScript model in
//! `web/index.html`; `tests/golden.rs` pins both implementations together.
//!
//! ## Example
//!
//! ```
//! use firmgap::{Params, run};
//!
//! let mut p = Params::default();
//! p.set("bess_mw", "13.5").unwrap();
//! p.set("bess_h", "2").unwrap();
//! p.set("max_throttle_h", "12").unwrap();
//! let r = run(&p, false).unwrap();
//! assert!(r.econ.certified);
//! assert_eq!(r.econ.pull_forward_months, 28.0);
//!
//! // Investor view under realistic derates (Base Case world, robust design)
//! let q = firmgap::apply_design(&firmgap::apply_world(&Params::default(), "base").unwrap(), "robust").unwrap();
//! let r = run(&q, false).unwrap();
//! assert!(r.compliant() && r.inv.npv > 0.0);
//! ```

#![forbid(unsafe_code)]
// Hourly loops index several parallel series with the same hour index; this is
// intentional and mirrors the reference implementation line for line.
#![allow(clippy::needless_range_loop)]

pub mod audit_bundle;
pub mod calendar;
pub mod canon;
pub mod compliance;
pub mod economics;
pub mod error;
pub mod guard;
pub mod invest;
pub mod json;
pub mod optimise;
pub mod params;
pub mod portfolio;
pub mod presets;
pub mod report;
pub mod rng;
pub mod robust;
pub mod rulepacks_intl;
pub mod scenario;
pub mod shed_curve;
pub mod sim;
pub mod stress;
pub mod worlds;
pub mod year;

pub use audit_bundle::{build_dossier, sha256_hex, verify_text, ComplianceDossier, Dossier};
pub use compliance::{compliance_report, ComplianceReport, ComplianceVerdict, JurisdictionRulepack, RulepackStatus, SiteInputs};
pub use economics::{Economics, Pathway};
pub use error::Error;
pub use guard::{guardrails, verdict, Finding, Severity, Verdict};
pub use invest::{Disposition, Investor};
pub use optimise::{optimise, sweep, Candidate, Optimisation};
pub use params::{Params, Strategy};
pub use portfolio::{batch_audit, compare_jurisdictions, share_decode, share_encode, Batch, BatchSelection, Compare};
pub use robust::{monte_carlo, robust_optimise, seed_list, MonteCarlo, Robust};
pub use shed_curve::{shed_curve, SheddingCapabilityCurve};
pub use sim::{is_compliant, reprice, run, run_cached, run_on_year, Run, RunOptions, SimResult};
pub use stress::{stress_suite, StressReport};
pub use worlds::{apply_design, apply_world, DESIGNS, WORLDS};
pub use year::{build_year, Year};

/// Hours in the modelled (non-leap) year.
pub const HOURS_PER_YEAR: usize = 8760;

/// Crate version, exposed for reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
