// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! International rulepacks (model 0.4): Virginia SB 253, GB NESO / Ofgem, Ireland CRU/2025/236,
//! Spain RD 1032/2026 and Singapore GDCR / DC-CFA2.
//!
//! Every message, threshold and order of operations is identical to `INTL_REPORTS` in the web
//! reference (`web/index.html`), so both produce byte-identical dossiers (`tests/golden_audit.rs`).
//! Sources and their status (adopted, proposed, reported) are listed in `docs/JURISDICTIONS_2026.md`.

use crate::canon::qz;
use crate::compliance::{
    check, financials_with, fx, round_up, shed_at_duration, usd_txt, verdict_for, ChangeValue, ComplianceReport, FinancialTerms,
    JurisdictionRulepack, RemediationOption, RuleCheck, RuleId, Severity, Site, REM_MAX_BESS_MW,
};
use crate::params::Params;
use crate::shed_curve::capex_power_energy;
use crate::sim::Run;

#[allow(clippy::too_many_arguments)]
fn rem(
    id: &'static str,
    rule_id: RuleId,
    title: &'static str,
    action: String,
    changes: Vec<(&'static str, ChangeValue)>,
    capex_usd: f64,
    recommended: bool,
    resolves: bool,
) -> RemediationOption {
    RemediationOption {
        id,
        rule_id,
        title,
        action,
        changes,
        value_usd_per_year: 0.0,
        pv_5y_usd: 0.0,
        delta_vs_other_usd_per_year: 0.0,
        capex_usd,
        recommended,
        resolves,
    }
}

fn finish(
    rp: JurisdictionRulepack,
    checks: Vec<RuleCheck>,
    fin: crate::compliance::ComplianceFinancials,
    remediations: Vec<RemediationOption>,
) -> ComplianceReport {
    ComplianceReport { rulepack: rp, rulepack_id: rp.id(), verdict: verdict_for(&checks), checks, financial_drag: fin, remediations }
}

fn yes(v: bool) -> f64 {
    if v {
        1.0
    } else {
        0.0
    }
}

/// Virginia SB 253 (+ HB 507, HB 284).
pub fn virginia(s: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let f = &r.flex;
    let g = s.gross_peak_mw;
    let mut checks = Vec::new();
    let mut rems = Vec::new();
    let app = g >= 25.0 && s.va_dominion_territory;
    checks.push(check(
        VaApplicability,
        Severity::Info,
        true,
        Some(g),
        Some(25.0),
        "MW",
        if app {
            format!(
                "Peak demand {} MW in Dominion territory: SB 253 high-load cost allocation can apply once the SCC approves it.",
                fx(g, 1)
            )
        } else {
            "Below 25 MW or outside Dominion territory: SB 253 cost allocation does not apply.".into()
        },
    ));
    let exposure = if app { f.energy_cost * s.va_rate_uplift_share } else { 0.0 };
    checks.push(check(
        VaCostAllocation,
        Severity::Info,
        true,
        Some(exposure),
        None,
        "$/yr",
        if app {
            format!(
                "If the SCC approves the shift, a {}% uplift on {}/yr of energy cost adds about {}/yr.",
                fx(s.va_rate_uplift_share * 100.0, 1),
                usd_txt(f.energy_cost),
                usd_txt(exposure)
            )
        } else {
            "No SB 253 cost-allocation exposure.".into()
        },
    ));
    let has_gen = s.backup_gen_mw > 0.0 || p.gen_mw > 0.0;
    checks.push(check(
        VaTier4Backup,
        Severity::Blocker,
        !has_gen || s.backup_tier4,
        Some(yes(s.backup_tier4)),
        Some(yes(has_gen)),
        "flag",
        if !has_gen {
            "No backup generators: no Tier IV air-permit requirement.".into()
        } else if s.backup_tier4 {
            "Backup generators are Tier IV or equivalent.".into()
        } else {
            "Backup generators are not Tier IV: DEQ must deny new data-centre generator air permits submitted after July 2026.".into()
        },
    ));
    if has_gen && !s.backup_tier4 {
        rems.push(rem(
            "VA_SPECIFY_TIER4",
            VaTier4Backup,
            "Specify Tier IV (or equivalent) backup generators",
            "Procure Tier IV units or equivalent after-treatment before filing the air-permit application.".into(),
            vec![("backup_tier4", ChangeValue::Bool(true))],
            0.0,
            true,
            true,
        ));
    }
    let shed1 = shed_at_duration(s, 1.0);
    checks.push(check(
        VaDemandFlex,
        Severity::Info,
        true,
        Some(shed1),
        None,
        "MW",
        format!(
            "Certified 1-hour shed of {} MW is the volume to offer into the voluntary demand-flexibility programs Dominion and APCo must develop.",
            fx(shed1, 2)
        ),
    ));
    finish(JurisdictionRulepack::VirginiaSb253_2026, checks, financials_with(s, p, r, &FinancialTerms::none()), rems)
}

/// GB NESO Gate 2 + Ofgem proposed data-centre reforms.
pub fn uk(s: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let g = s.gross_peak_mw;
    let mut checks = Vec::new();
    let mut rems = Vec::new();
    checks.push(check(
        UkGate2,
        Severity::Blocker,
        s.uk_gate2_ready,
        Some(yes(s.uk_gate2_ready)),
        Some(1.0),
        "flag",
        if s.uk_gate2_ready {
            "Meets NESO Gate 2 readiness and strategic-alignment criteria: eligible for a Gate 2 offer.".into()
        } else {
            "Does not meet Gate 2 readiness: no firm connection date until land-rights and planning evidence is in place.".into()
        },
    ));
    if !s.uk_gate2_ready {
        rems.push(rem(
            "UK_SECURE_GATE2",
            UkGate2,
            "Secure Gate 2 readiness evidence",
            "Secure land rights (or an option) and the planning evidence NESO requires, then re-apply for a Gate 2 offer.".into(),
            vec![("uk_gate2_ready", ChangeValue::Bool(true))],
            0.0,
            true,
            true,
        ));
    }
    let milestones: [(RuleId, &'static str, bool, &'static str, &'static str); 3] = [
        (
            UkEndUser,
            "uk_end_user_evidence",
            s.uk_end_user_evidence,
            "Credible end-user evidence is on file (proposed queue milestone).",
            "No credible end-user evidence: under the proposed milestones the network operator could penalise or remove the project.",
        ),
        (
            UkLongLead,
            "uk_long_lead_procured",
            s.uk_long_lead_procured,
            "Long-lead electrical equipment is procured (proposed milestone).",
            "Long-lead equipment (e.g. switchgear) is not procured: proposed milestone open.",
        ),
        (
            UkCapability,
            "uk_capability_evidence",
            s.uk_capability_evidence,
            "Financial and technical capability evidence is on file (proposed milestone).",
            "Financial and technical capability evidence is missing (proposed milestone).",
        ),
    ];
    let mut missing: Vec<&'static str> = Vec::new();
    let mut first_missing: Option<RuleId> = None;
    for (id, key, ok, good, bad) in milestones {
        checks.push(check(id, Severity::Warning, ok, Some(yes(ok)), Some(1.0), "flag", if ok { good.into() } else { bad.into() }));
        if !ok {
            missing.push(key);
            if first_missing.is_none() {
                first_missing = Some(id);
            }
        }
    }
    if let Some(rule) = first_missing {
        rems.push(rem(
            "UK_FILE_MILESTONES",
            rule,
            "File the missing queue-milestone evidence",
            format!(
                "Prepare evidence for: {}. Each item is a proposed Ofgem milestone the network operator can check.",
                missing.join(", ")
            ),
            missing.iter().map(|k| (*k, ChangeValue::Bool(true))).collect(),
            0.0,
            true,
            true,
        ));
    }
    let fee = s.uk_commitment_fee_usd_per_mw * g;
    checks.push(check(
        UkCommitmentFee,
        Severity::Info,
        true,
        Some(fee),
        None,
        "$",
        format!(
            "Proposed commitment fee {} ({}/MW x {} MW, assumed rate) is secured from offer acceptance to connection and is not returned if milestones are missed.",
            usd_txt(fee),
            usd_txt(s.uk_commitment_fee_usd_per_mw),
            fx(g, 1)
        ),
    ));
    let terms = FinancialTerms { security: fee, study_fee: 0.0, forfeit_share: 1.0, tranches: false, show: true };
    finish(JurisdictionRulepack::UkNeso2026, checks, financials_with(s, p, r, &terms), rems)
}

/// Ireland CRU/2025/236.
pub fn ireland(s: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let g = s.gross_peak_mw;
    let mut checks = Vec::new();
    let mut rems = Vec::new();
    let mic = g / s.power_factor;
    let tier = if mic < 1.0 {
        0
    } else if mic < 10.0 {
        1
    } else {
        2
    };
    checks.push(check(
        IeApplicability,
        Severity::Info,
        true,
        Some(mic),
        Some(1.0),
        "MVA",
        match tier {
            0 => format!("MIC {} MVA is below 1 MVA: the LEU connection policy does not apply.", fx(mic, 2)),
            1 => format!("MIC {} MVA is between 1 and 10 MVA: an autoproducer unit must cover 100% of MIC (de-rated) and trade in the SEM.", fx(mic, 2)),
            _ => format!(
                "MIC {} MVA is at least 10 MVA: dispatchable on-site or proximate generation and/or storage must match MIC (de-rated) and trade in the SEM.",
                fx(mic, 2)
            ),
        },
    ));
    if tier > 0 {
        let k = p.effective();
        let credit = s.ie_renewable_derated_mw.min(0.8 * mic);
        let required = mic - credit;
        let gen_part = s.backup_gen_mw * s.ie_gen_derate;
        let avail = gen_part + k.bess_mw * s.ie_storage_derate;
        let ok = avail >= required - 1e-9;
        checks.push(check(
            IeDispatchableMatch,
            Severity::Blocker,
            ok,
            Some(avail),
            Some(required),
            "MW",
            format!(
                "{}de-rated dispatchable capacity {} MW ({} MW generation x {} + {} MW storage x {}) vs requirement {} MW (MIC {} MVA less {} MW renewable credit).",
                if ok { "Covered: " } else { "Short: " },
                fx(avail, 2),
                fx(s.backup_gen_mw, 1),
                fx(s.ie_gen_derate, 2),
                fx(k.bess_mw, 2),
                fx(s.ie_storage_derate, 2),
                fx(required, 2),
                fx(mic, 2),
                fx(credit, 2)
            ),
        ));
        if !ok && s.ie_storage_derate > 0.0 {
            let need = (required - gen_part) / (s.ie_storage_derate * p.bess_derate);
            let mw_apply = round_up(need, 0.1).min(REM_MAX_BESS_MW);
            let d_mw = (mw_apply - p.bess_mw).max(0.0);
            rems.push(rem(
                "IE_ADD_STORAGE",
                IeDispatchableMatch,
                "Add de-rated storage to match MIC",
                format!(
                    "Raise BESS to {} MW (+{} MW / +{} MWh) so de-rated capacity covers the MIC requirement.",
                    fx(mw_apply, 1),
                    fx(d_mw, 1),
                    fx(d_mw * p.bess_h, 1)
                ),
                vec![("bess_mw", ChangeValue::Num(qz(mw_apply).unwrap_or(mw_apply)))],
                capex_power_energy(p, d_mw, d_mw * p.bess_h),
                true,
                need <= REM_MAX_BESS_MW,
            ));
        }
        let has_gen = s.backup_gen_mw > 0.0;
        let run_ok = !has_gen || p.gen_cap_h >= 8000.0;
        checks.push(check(
            IeRunHours,
            Severity::Warning,
            run_ok,
            Some(p.gen_cap_h),
            Some(8000.0),
            "h/yr",
            if !has_gen {
                "No generation counted toward MIC: no run-hour restriction.".into()
            } else if run_ok {
                format!("Generation may run {} h/yr: not bound by limited run hours.", fx(p.gen_cap_h, 0))
            } else {
                format!(
                    "Generation is capped at {} h/yr: units must not be bound by limited run hours (firmgap threshold 8,000 h/yr), so it may not count toward MIC.",
                    fx(p.gen_cap_h, 0)
                )
            },
        ));
        checks.push(check(
            IeRenewable80,
            Severity::Warning,
            s.renewable_share >= 0.8 - 1e-12,
            Some(s.renewable_share),
            Some(0.8),
            "fraction",
            format!(
                "Annual additional Irish renewables {}% vs 80% (six-year glide path from energisation).",
                fx(s.renewable_share * 100.0, 1)
            ),
        ));
    }
    finish(JurisdictionRulepack::IrelandCru2026, checks, financials_with(s, p, r, &FinancialTerms::none()), rems)
}

/// Spain Real Decreto 1032/2026.
pub fn spain(s: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let g = s.gross_peak_mw;
    let mut checks = Vec::new();
    let mut rems = Vec::new();
    let app = g >= 10.0;
    let sev = if app { Severity::Blocker } else { Severity::Info };
    checks.push(check(
        EsApplicability,
        Severity::Info,
        true,
        Some(g),
        Some(10.0),
        "MW",
        if app {
            format!("Access capacity {} MW is at least 10 MW: RD 1032/2026 connection conditions apply (in force 2026-11-22).", fx(g, 1))
        } else {
            "Below 10 MW: RD 1032/2026 efficiency and hourly-renewable conditions do not apply.".into()
        },
    ));
    checks.push(check(
        EsPue,
        sev,
        !app || s.pue <= 1.25 + 1e-12,
        Some(s.pue),
        Some(1.25),
        "-",
        format!("PUE {} vs the 1.25 cap (enforced two years after start of operation, until the EU rating label applies).", fx(s.pue, 2)),
    ));
    checks.push(check(
        EsWue,
        sev,
        !app || s.wue_m3_per_mwh <= 0.1 + 1e-12,
        Some(s.wue_m3_per_mwh),
        Some(0.1),
        "m3/MWh",
        format!(
            "WUE {} m3/MWh vs the 0.1 cap (about 100 litres per MWh of IT energy; enforced one year after start).",
            fx(s.wue_m3_per_mwh, 3)
        ),
    ));
    let m = s.hourly_renewable_match;
    let t = s.es_hourly_target;
    let ok = !app || m >= t - 1e-12;
    let hourly_sev = if !app {
        Severity::Info
    } else if s.es_interruptible_opt_in {
        Severity::Warning
    } else {
        Severity::Blocker
    };
    checks.push(check(
        EsHourlyRenewable,
        hourly_sev,
        ok,
        Some(m),
        Some(t),
        "fraction",
        format!(
            "Hourly new-renewable match {}% vs {}% target{}",
            fx(m * 100.0, 1),
            fx(t * 100.0, 1),
            if ok {
                "."
            } else if s.es_interruptible_opt_in {
                ": covered for up to five years by the interruptible path (first to disconnect when the grid needs it)."
            } else {
                ": the shortfall triggers automatic compensation charges; sanctions can suspend connection rights for up to three years."
            }
        ),
    ));
    if !ok {
        rems.push(rem(
            "ES_CONTRACT_HOURLY",
            EsHourlyRenewable,
            "Contract new renewables and storage for hourly matching",
            format!(
                "Raise hourly matching from {}% to {}% with new plants (connected at most 36 months before start), storage or PPAs longer than six years.",
                fx(m * 100.0, 1),
                fx(t * 100.0, 1)
            ),
            vec![("hourly_renewable_match", ChangeValue::Num(qz(t).unwrap_or(t)))],
            0.0,
            true,
            true,
        ));
        if !s.es_interruptible_opt_in {
            rems.push(rem(
                "ES_INTERRUPTIBLE_PATH",
                EsHourlyRenewable,
                "Take the five-year interruptible path",
                format!(
                    "Accept being first to disconnect when the grid needs it; certified 30-minute shed of {} MW backs the commitment. Downgrades the blocker to a warning for up to five years.",
                    fx(shed_at_duration(s, 0.5), 2)
                ),
                vec![("es_interruptible_opt_in", ChangeValue::Bool(true))],
                0.0,
                false,
                false,
            ));
        }
    }
    let it = s.it_power_kw;
    checks.push(check(
        EsReporting,
        Severity::Info,
        true,
        Some(it),
        Some(500.0),
        "kW",
        if it > 500.0 {
            format!(
                "IT power {} kW: annual efficiency report to MITECO{}",
                fx(it, 0),
                if it > 1000.0 { " and EU Code of Conduct declaration." } else { "." }
            )
        } else {
            "IT power at or below 500 kW: no MITECO efficiency report.".into()
        },
    ));
    finish(JurisdictionRulepack::SpainRd2026, checks, financials_with(s, p, r, &FinancialTerms::none()), rems)
}

/// Singapore Green Data Centre Roadmap / DC-CFA2.
pub fn singapore(s: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let mut checks = Vec::new();
    let mut rems = Vec::new();
    checks.push(check(
        SgPueGdcr,
        Severity::Blocker,
        s.pue <= 1.3 + 1e-12,
        Some(s.pue),
        Some(1.3),
        "-",
        format!("PUE {} vs the Green Data Centre Roadmap target of 1.3 for new builds.", fx(s.pue, 2)),
    ));
    checks.push(check(
        SgPueCfa,
        Severity::Warning,
        s.pue <= 1.25 + 1e-12,
        Some(s.pue),
        Some(1.25),
        "-",
        format!("PUE {} vs 1.25 at full load reported for DC-CFA allocations (competitive threshold).", fx(s.pue, 2)),
    ));
    let gs = s.green_energy_share;
    let gok = gs >= 0.5 - 1e-12;
    checks.push(check(
        SgGreen50,
        Severity::Blocker,
        gok,
        Some(gs),
        Some(0.5),
        "fraction",
        format!("Green-energy pathways supply {}% of power vs at least 50% required for DC-CFA2.", fx(gs * 100.0, 1)),
    ));
    if !gok {
        rems.push(rem(
            "SG_GREEN_PATHWAY",
            SgGreen50,
            "Contract approved green-energy pathways to 50%",
            format!(
                "Raise approved green-energy supply (imports, biomethane, low-carbon ammonia, hydrogen, on-site solar) from {}% to 50%.",
                fx(gs * 100.0, 1)
            ),
            vec![("green_energy_share", ChangeValue::Num(0.5))],
            0.0,
            true,
            true,
        ));
    }
    checks.push(check(
        SgCfaWindow,
        Severity::Info,
        true,
        None,
        None,
        "",
        "DC-CFA2 (at least 200 MW) closed 2026-03-31; new capacity needs a future allocation call.".into(),
    ));
    finish(JurisdictionRulepack::SingaporeGreenDc2026, checks, financials_with(s, p, r, &FinancialTerms::none()), rems)
}
