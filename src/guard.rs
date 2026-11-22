// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Guardrails and the investment verdict (port of `guardrails` / `verdictOf` in the web workbench).
//!
//! * **Violation**: blocks investment (not certifiable, ceiling breached, SLA exceeded, no bridge
//!   window, negative NPV, not robust across weather years, inconsistent cost inputs).
//! * **Warning**: an open diligence item (assumption beyond field evidence, no deadband, ...).
//! * **Info**: context for the reader.

use crate::params::Params;
use crate::report::money;
use crate::robust::MonteCarlo;
use crate::sim::Run;
use std::fmt;

/// Finding severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Blocks investment.
    Violation,
    /// Open diligence item.
    Warning,
    /// Context.
    Info,
}

impl Severity {
    /// One-letter code used in golden files (`v`, `w`, `i`).
    pub fn letter(self) -> char {
        match self {
            Severity::Violation => 'v',
            Severity::Warning => 'w',
            Severity::Info => 'i',
        }
    }
}

/// One guardrail finding.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Severity.
    pub sev: Severity,
    /// Stable machine code (e.g. `CERT`).
    pub code: &'static str,
    /// Short title.
    pub title: String,
    /// Explanation with the numbers behind it.
    pub detail: String,
}

/// Overall verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// At least one violation.
    NotInvestable,
    /// Weather-year robustness not yet evaluated.
    Pending,
    /// No violations, but open warnings.
    Conditional,
    /// No violations and no warnings.
    Investable,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Verdict::NotInvestable => "NOT INVESTABLE",
            Verdict::Pending => "PENDING",
            Verdict::Conditional => "CONDITIONAL",
            Verdict::Investable => "INVESTABLE",
        })
    }
}

/// Evaluate every guardrail for a run, optionally with weather-year robustness.
pub fn guardrails(p: &Params, r: &Run, mc: Option<&MonteCarlo>) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut add = |sev, code, title: String, detail: String| out.push(Finding { sev, code, title, detail });
    let f = &r.flex;
    let e = &r.econ;
    let i = &r.inv;
    let gapped = f.events.iter().filter(|x| x.peak_gap > 1e-9).count();
    use Severity::{Info, Violation, Warning};

    if !e.certified {
        add(
            Violation,
            "CERT",
            "Fails the first-minutes response test".into(),
            format!(
                "{} of {} events leave load above the effective {:.1} MW ceiling before preemption lands (worst coverage {:.0}%). The utility will not certify the expansion; BESS capital at risk (downside NPV {}).",
                f.failed5,
                gapped,
                p.effective().ceiling_mw,
                f.worst_ramp_cover * 100.0,
                money(i.npv_fail)
            ),
        );
    }
    if e.certified && f.short_e >= 0.5 {
        add(
            Violation,
            "SHORT",
            "Ceiling breached during long events".into(),
            format!("{:.1} MWh/yr above the ceiling: {}/yr penalties plus termination risk on the interim agreement. Extend the hold limit or add BESS energy.", f.short_e, money(f.penalty_cost)),
        );
    }
    if f.breach_pct >= p.sla_target {
        add(
            Violation,
            "SLA",
            "SLA-late compute above target".into(),
            format!(
                "{:.2}% of compute misses its {:.0} h window (target < {:.1}%).",
                f.breach_pct * 100.0,
                p.sla_window,
                p.sla_target * 100.0
            ),
        );
    }
    if e.certified && i.bridge_months <= 0.0 {
        add(
            Violation,
            "WINDOW",
            "No bridge window".into(),
            "Firm service arrives before the bridge is commissioned; the BESS earns only post-firm value.".into(),
        );
    }
    if i.npv < 0.0 {
        add(
            Violation,
            "NPV",
            "Negative net value vs. interim-only".into(),
            format!(
                "NPV {}. Break-even gross margin {}.",
                money(i.npv),
                i.break_even_margin.map_or_else(|| "n/a".to_string(), |v| format!("${:.0}/MWh", v))
            ),
        );
    }
    if let Some(mc) = mc {
        if mc.p_compliant < 0.9 {
            add(
                Violation,
                "MC",
                "Not robust across weather years".into(),
                format!(
                    "Compliant in {:.0}% of {} synthetic congestion years (investment threshold >= 90%).",
                    mc.p_compliant * 100.0,
                    mc.n
                ),
            );
        }
        if mc.npv_p10 < 0.0 {
            add(Violation, "P10", "P10 NPV is negative".into(), format!("P10 NPV {} across {} weather years.", money(mc.npv_p10), mc.n));
        }
    }
    if p.bess_power_cost / 4.0 > p.bess_cost {
        add(
            Violation,
            "COST",
            "Inconsistent BESS cost inputs".into(),
            "Power block exceeds the 4 h reference cost, so the energy block is priced at $0/kWh.".into(),
        );
    }
    if p.bess_mw > 0.0 && p.bess_h < 1.0 {
        add(
            Warning,
            "CRATE",
            format!("C-rate {:.1}C exceeds 1C", 1.0 / p.bess_h),
            "Verify chemistry, warranty cycling limits and thermal design for high-rate discharge.".into(),
        );
    }
    if p.max_throttle_h > 3.0 && f.thr_e > 0.0 {
        add(
            Warning,
            "HOLD",
            format!("Throttle hold of {:.0} h exceeds field evidence", p.max_throttle_h),
            "Public field data show a 25% cut sustained for 3 h on a 256-GPU cluster; longer holds need workload-owner contracts and a pilot.".into(),
        );
    }
    if p.defer_share > 0.30 {
        add(
            Warning,
            "DEFER",
            format!("{:.0}% of load assumed deferrable", p.defer_share * 100.0),
            "Needs contractual deferral rights by workload class; inference SLAs must stay untouched.".into(),
        );
    }
    if p.defer_cost == 0.0 && f.thr_e > 0.0 {
        add(
            Warning,
            "FREE",
            "Deferral is free in this run".into(),
            format!(
                "Workload owners usually require compensation; NPV stays positive up to {}.",
                i.break_even_defer_cost.map_or_else(|| "n/a".to_string(), |v| format!("${:.0}/MWh deferred", v))
            ),
        );
    }
    if p.safety_margin < 0.25 {
        add(
            Warning,
            "DEADBAND",
            "No metering / control deadband".into(),
            "Real controllers need margin below the ceiling for meter error and telemetry latency.".into(),
        );
    }
    if p.bess_mw > 0.0 && p.soc_window > 0.95 {
        add(Warning, "SOC", "SoC window above 95%".into(), "Likely outside the warranty operating window.".into());
    }
    if p.bess_mw > 0.0 && p.bess_derate >= 1.0 {
        add(Warning, "AVAIL", "100% BESS availability assumed".into(), "No allowance for an inverter outage (N-1) during an event.".into());
    }
    if p.gen_mw > 0.0 && f64::from(f.gen_hrs) >= p.gen_cap_h {
        add(
            Warning,
            "PERMIT",
            "Genset permit hours exhausted".into(),
            format!("Genset ran {} h, the permit cap; later events are uncovered.", f.gen_hrs),
        );
    }
    if (r.year.placed_hours() as f64) < p.stress_hours.round() {
        add(
            Warning,
            "SAT",
            "Signal calendar saturated".into(),
            "Not all requested signal hours could be placed in the seasonal windows.".into(),
        );
    }
    if p.salvage > 0.4 {
        add(
            Warning,
            "SALV",
            "Optimistic BESS resale value".into(),
            "The secondary market for used grid batteries is thin; treat resale above 40% as speculative.".into(),
        );
    }
    if let Some(pb) = i.payback {
        if i.bridge_months > 0.0 && f64::from(pb) > p.bess_lead + i.bridge_months {
            add(
                Warning,
                "PAYB",
                "Payback outlasts the bridge window".into(),
                "The capital is recovered only through post-firm value.".into(),
            );
        }
    }
    if e.reduction < 0.25 {
        add(
            Info,
            "KPI",
            "Case KPI (> 25% lower all-in $/MWh) not met".into(),
            format!(
                "All-in {:.1}% vs. waiting; 25% needs carry >= {}. The KPI is diluted by energy cost, which is equal across pathways.",
                e.reduction * 100.0,
                if e.breakeven_carry.is_finite() && e.breakeven_carry > 0.0 {
                    format!("{}/mo", money(e.breakeven_carry))
                } else {
                    "n/a".into()
                }
            ),
        );
    }
    add(
        Info,
        "MARGIN",
        "NPV scales with the assumed gross margin".into(),
        format!(
            "Assumed ${:.0}/MWh before energy; break-even {}. Replace with contracted revenue.",
            p.margin_pre_energy,
            i.break_even_margin.map_or_else(|| "n/a".to_string(), |v| format!("${:.0}/MWh", v))
        ),
    );
    out
}

/// Verdict rule: any violation is NOT INVESTABLE; without weather-year robustness PENDING; any
/// warning CONDITIONAL; otherwise INVESTABLE.
pub fn verdict(findings: &[Finding], mc_evaluated: bool) -> Verdict {
    if findings.iter().any(|f| f.sev == Severity::Violation) {
        Verdict::NotInvestable
    } else if !mc_evaluated {
        Verdict::Pending
    } else if findings.iter().any(|f| f.sev == Severity::Warning) {
        Verdict::Conditional
    } else {
        Verdict::Investable
    }
}

/// Codes as `sev:CODE` strings in evaluation order (golden-file format).
pub fn codes(findings: &[Finding]) -> Vec<String> {
    findings.iter().map(|f| format!("{}:{}", f.sev.letter(), f.code)).collect()
}

/// Map a compliance report onto guardrail findings (failed blockers become violations, failed
/// warnings become warnings). Separate from [`guardrails`] so existing verdicts are unchanged.
pub fn compliance_findings(rep: &crate::compliance::ComplianceReport) -> Vec<Finding> {
    use crate::compliance::Severity as C;
    rep.checks
        .iter()
        .filter(|c| !c.passed)
        .map(|c| Finding {
            sev: match c.severity {
                C::Blocker => Severity::Violation,
                C::Warning => Severity::Warning,
                C::Info => Severity::Info,
            },
            code: c.rule_id.code(),
            title: format!("{} failed", c.rule_id.code()),
            detail: c.summary.clone(),
        })
        .collect()
}
