// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Declarative, versioned compliance rulepacks with counterfactual remediation.
//!
//! **Read the citations.** Each check states the provision it implements and whether it is a
//! statute or rule (blocking), a regulator-approved protocol, or a planning assumption. The model
//! is a screening tool for diligence, not legal advice. Notable limits (verified 2026-11-22):
//!
//! * The SB 6 30-minute disconnect test is a *planning stress test*. The PUCT declined to codify a
//!   30-minute curtailment window in the 16 TAC Sec. 25.194 adoption order.
//! * FERC RM26-4 has no final rule. Those checks model anticipated interconnection-agreement terms.
//! * The EU EED Art. 12 imposes reporting, not a PUE ceiling. The PUE target is national or contractual.
//!
//! The JavaScript reference implementation lives in `web/index.html` (FIRMGAP-CORE block,
//! `complianceReport`, `INTL_REPORTS`). `tests/golden_audit.rs` pins both to identical dossiers.
//! The Virginia, GB, Ireland, Spain and Singapore rulepacks live in [`crate::rulepacks_intl`].

use crate::canon::{num, obj, opt, qz, round_half_up, s, Value};
use crate::error::Error;
use crate::params::Params;
use crate::shed_curve::{capex_power_energy, certified_shed, shed_classes};
use crate::sim::{simulate, Run, SimConfig};
use crate::year::Year;
use crate::HOURS_PER_YEAR as HRS;
use std::collections::BTreeMap;

/// Kind of a site input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteKind {
    /// Number with a range.
    Num,
    /// Boolean flag.
    Bool,
}

/// Metadata of one site input.
#[derive(Debug, Clone, Copy)]
pub struct SiteSpec {
    /// Scenario-file key.
    pub key: &'static str,
    /// Inclusive minimum (numbers).
    pub min: f64,
    /// Inclusive maximum (numbers).
    pub max: f64,
    /// Default (`None` = derived from the simulation parameters).
    pub default: Option<f64>,
    /// Kind.
    pub kind: SiteKind,
    /// Unit.
    pub unit: &'static str,
    /// Description.
    pub help: &'static str,
}

const fn n(key: &'static str, min: f64, max: f64, default: Option<f64>, unit: &'static str, help: &'static str) -> SiteSpec {
    SiteSpec { key, min, max, default, kind: SiteKind::Num, unit, help }
}
const fn b(key: &'static str, default: bool, help: &'static str) -> SiteSpec {
    SiteSpec { key, min: 0.0, max: 1.0, default: Some(if default { 1.0 } else { 0.0 }), kind: SiteKind::Bool, unit: "", help }
}

/// Every compliance site input (identical to `SITE_SPEC` in the web reference).
pub const SITE_SPECS: &[SiteSpec] = &[
    n("gross_peak_mw", 1.0, 5000.0, None, "MW", "Requested (non-coincident) peak demand at the site, NOT netted for on-site BESS or generation. Default: target_mw."),
    n("study_fee_usd", 0.0, 1e8, Some(100000.0), "$", "Initial transmission screening study fee (statutory minimum $100,000)."),
    n("security_usd_per_mw", 0.0, 1e7, Some(50000.0), "$/MW", "Financial security per MW of requested peak demand (16 TAC 25.194)."),
    n("forfeiture_share", 0.0, 1.0, Some(0.2), "fraction", "Share of security tied to allocated capacity retained if the request is withdrawn after allocation."),
    n("release_tranche_share", 0.01, 1.0, Some(0.2), "fraction", "Security returned at energization and at each later SLLIA milestone."),
    n("months_to_energization", 0.0, 120.0, Some(24.0), "months", "ASSUMPTION: months from posting security to energization."),
    n("milestone_interval_months", 1.0, 120.0, Some(12.0), "months", "ASSUMPTION: months between SLLIA release milestones."),
    n("collateral_yield", 0.0, 0.5, Some(0.04), "fraction", "ASSUMPTION: interest earned on cash collateral (rule floor: 1-month T-bill)."),
    n("withdraw_probability", 0.0, 1.0, Some(0.1), "fraction", "ASSUMPTION: probability of withdrawing after capacity allocation."),
    n("sla_ld_usd_per_mwh", 0.0, 1e6, Some(500.0), "$/MWh", "ASSUMPTION: liquidated damages per MWh of SLA-late compute."),
    n("reliability_service_mw", 0.0, 5000.0, Some(0.0), "MW", "MW committed to the ERCOT large-load reliability service (PURA 39.170(b))."),
    n("reliability_service_usd_per_mw_year", 0.0, 1e7, Some(40000.0), "$/MW-yr", "ASSUMPTION: no market price exists yet; ERCOT is still designing the service."),
    n("computational_share", 0.0, 1.0, Some(0.9), "fraction", "Share of site demand that is power-electronic computational load."),
    n("backup_gen_mw", 0.0, 5000.0, Some(0.0), "MW", "On-site backup generation (non-exporting)."),
    n("backup_deploy_hours_per_year", 0.0, 8760.0, Some(20.0), "h/yr", "ASSUMPTION: hours/yr ERCOT may direct backup deployment in emergencies."),
    n("disconnect_test_minutes", 5.0, 240.0, Some(30.0), "min", "Planning stress test: duration of a full grid disconnect the campus must survive."),
    n("u_min", 0.0, 1.0, Some(0.4), "fraction", "GPU power-cap floor for flexible workloads (fraction of nominal power)."),
    n("beta", 0.1, 1.0, Some(0.75), "-", "Speed-power sensitivity exponent: throughput ~ power^beta."),
    n("sla_slack_hours", 0.0, 48.0, Some(2.0), "h", "SLA slack of the batch class (training gets slack x multiplier)."),
    n("flexible_share", 0.0, 0.95, None, "fraction", "Share of site load that is flexible batch / training. Default: defer_share."),
    n("training_share_of_flexible", 0.0, 1.0, Some(0.5), "fraction", "Share of the flexible load that is training (rest is batch)."),
    n("training_slack_multiplier", 1.0, 24.0, Some(3.0), "x", "Training SLA slack = sla_slack_hours x this multiplier."),
    n("batch_value_usd_per_mwh", 0.0, 1e5, Some(250.0), "$/MWh", "ASSUMPTION: value of batch throughput per MWh of compute."),
    n("training_value_usd_per_mwh", 0.0, 1e5, Some(150.0), "$/MWh", "ASSUMPTION: value of training throughput per MWh of compute."),
    n("critical_value_usd_per_mwh", 0.0, 1e6, Some(2000.0), "$/MWh", "ASSUMPTION: value of inference (critical, not sheddable)."),
    n("required_shed_mw", 0.0, 5000.0, None, "MW", "Curtailment the site must deliver. Default: gross_peak_mw - effective ceiling."),
    n("bess_degradation_usd_per_mwh", 0.0, 1000.0, Some(20.0), "$/MWh", "ASSUMPTION: BESS cycling (degradation) cost per MWh of throughput."),
    n("poi_import_limit_mw", 0.0, 5000.0, None, "MW", "Point-of-interconnection import limit. Default: gross_peak_mw."),
    n("max_ramp_mw_per_min", 0.1, 1000.0, Some(10.0), "MW/min", "ASSUMPTION: ramp-rate limit in the interconnection agreement."),
    n("power_factor", 0.5, 1.0, Some(0.98), "-", "Site power factor at the POI."),
    n("pf_floor", 0.5, 1.0, Some(0.95), "-", "Minimum power factor in the interconnection agreement."),
    n("milestone_energize_month", 0.0, 240.0, None, "month", "Staged-energization milestone month. Default: interim_month + bess_lead."),
    n("it_power_kw", 0.0, 5e6, None, "kW", "Installed IT power demand (EED threshold 500 kW). Default: gross_peak_mw x 1000."),
    n("pue", 1.0, 3.0, Some(1.3), "-", "Power usage effectiveness (reported under EED Annex VII)."),
    n("pue_target", 1.0, 3.0, Some(1.3), "-", "National or contractual PUE target (NOT an EED Art. 12 requirement)."),
    n("renewable_share", 0.0, 1.0, Some(0.5), "fraction", "Renewable energy share (reported under EED Annex VII)."),
    n("flex_readiness_target", 0.0, 1.0, Some(0.1), "fraction", "ASSUMPTION: target 1-hour certified shed as a share of gross peak."),
    n("va_rate_uplift_share", 0.0, 1.0, Some(0.158), "fraction", "ASSUMPTION: rate increase for the high-load class if the SCC approves the SB 253 cost shift (SCC staff estimate reported in press)."),
    n("uk_commitment_fee_usd_per_mw", 0.0, 10000000.0, Some(20000.0), "$/MW", "ASSUMPTION: proposed GB data-centre commitment fee; Ofgem has not set the amount."),
    n("ie_renewable_derated_mw", 0.0, 5000.0, Some(0.0), "MW", "De-rated MW of additional Irish renewables contracted (up to 80% of MIC can count toward the dispatchable requirement)."),
    n("ie_storage_derate", 0.0, 1.0, Some(0.5), "fraction", "ASSUMPTION: CRM T-4 de-rating factor for the BESS duration (check the latest auction pack)."),
    n("ie_gen_derate", 0.0, 1.0, Some(0.9), "fraction", "ASSUMPTION: CRM de-rating factor for on-site generation."),
    n("wue_m3_per_mwh", 0.0, 5.0, Some(0.1), "m3/MWh", "Water usage effectiveness (m3 per MWh of IT energy)."),
    n("hourly_renewable_match", 0.0, 1.0, Some(0.6), "fraction", "Share of consumption matched hour by hour with new renewables (Spain RD 1032/2026)."),
    n("es_hourly_target", 0.0, 1.0, Some(0.8), "fraction", "Hourly matching target in force for the site (80% end state; transitional steps before 2032)."),
    n("green_energy_share", 0.0, 1.0, Some(0.5), "fraction", "Share of power from IMDA-approved green-energy pathways (Singapore DC-CFA2)."),
    b("participates_in_reliability_service", false, "Enrolled in the ERCOT large-load reliability service (PURA 39.170(b))."),
    b("price_responsive_lmp_curtailment", true, "Curtails in response to wholesale prices (LMP arbitrage)."),
    b("firm_shed_telemetry", true, "Remote curtailment equipment for firm load shed is installed (PURA 39.170(a))."),
    b("interconnected_after_2025", true, "Transmission-voltage load interconnected after 2025-12-31."),
    b("ride_through_certified", false, "NOGRR 282 voltage / frequency ride-through demonstrated."),
    b("backup_disclosed", true, "On-site backup generation disclosed to the utility and ERCOT."),
    b("eed_metering_ready", true, "Metering and data collection for EED Annex VII KPIs in place."),
    b("renewable_disclosed", true, "Renewable energy share disclosure prepared."),
    b("va_dominion_territory", true, "Served by Dominion Energy Virginia (SB 253 cost-allocation scope)."),
    b("backup_tier4", false, "Backup generators are EPA Tier IV or equivalent (Virginia HB 507)."),
    b("uk_gate2_ready", true, "Meets NESO Gate 2 readiness and strategic-alignment criteria."),
    b("uk_end_user_evidence", false, "Evidence of a credible end user (proposed Ofgem queue milestone)."),
    b("uk_long_lead_procured", false, "Long-lead electrical equipment (e.g. switchgear) procured (proposed milestone)."),
    b("uk_capability_evidence", true, "Financial and technical capability evidence (proposed milestone)."),
    b("es_interruptible_opt_in", false, "Accepts the transitional path: first to be disconnected when the grid needs it (five years)."),
];

/// Look up a site spec.
pub fn site_spec(key: &str) -> Option<&'static SiteSpec> {
    SITE_SPECS.iter().find(|sp| sp.key == key)
}

/// Raw, validated site inputs (derived fields may be unset). Every field is optional so existing
/// scenario files remain valid.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SiteInputs {
    values: BTreeMap<&'static str, f64>,
}

impl SiteInputs {
    /// Set a site input from scenario text, enforcing type and range.
    pub fn set(&mut self, key: &str, raw: &str) -> Result<(), Error> {
        let sp = site_spec(key.trim()).ok_or_else(|| Error::UnknownKey(key.to_string()))?;
        let value = raw.trim().trim_matches('"').trim_matches('\'');
        let v = match sp.kind {
            SiteKind::Bool => match value.to_ascii_lowercase().as_str() {
                "true" => 1.0,
                "false" => 0.0,
                _ => {
                    return Err(Error::InvalidValue { key: sp.key.to_string(), value: value.to_string(), reason: "expected true or false" })
                }
            },
            SiteKind::Num => {
                let v: f64 = value.replace('_', "").parse().map_err(|_| Error::InvalidValue {
                    key: sp.key.to_string(),
                    value: value.to_string(),
                    reason: "expected a number",
                })?;
                if !v.is_finite() {
                    return Err(Error::InvalidValue {
                        key: sp.key.to_string(),
                        value: value.to_string(),
                        reason: "must be a finite number",
                    });
                }
                if !(sp.min..=sp.max).contains(&v) {
                    return Err(Error::OutOfRange { key: sp.key.to_string(), value: v, min: sp.min, max: sp.max });
                }
                v
            }
        };
        self.values.insert(sp.key, v);
        Ok(())
    }

    /// Set a number or flag programmatically (validated).
    pub fn set_value(&mut self, key: &str, v: f64) -> Result<(), Error> {
        let sp = site_spec(key).ok_or_else(|| Error::UnknownKey(key.to_string()))?;
        if !v.is_finite() || !(sp.min..=sp.max).contains(&v) {
            return Err(Error::OutOfRange { key: key.to_string(), value: v, min: sp.min, max: sp.max });
        }
        self.values.insert(sp.key, v);
        Ok(())
    }

    /// True if the key is a site input.
    pub fn is_site_key(key: &str) -> bool {
        site_spec(key).is_some()
    }

    fn get_or(&self, key: &str, fallback: f64) -> f64 {
        match self.values.get(key) {
            Some(v) => *v,
            None => site_spec(key).and_then(|sp| sp.default).unwrap_or(fallback),
        }
    }

    fn flag(&self, key: &str) -> bool {
        self.get_or(key, 0.0) != 0.0
    }

    /// Resolve derived defaults against the simulation parameters.
    pub fn resolve(&self, p: &Params) -> Site {
        let e = p.effective();
        let gross = self.values.get("gross_peak_mw").copied().unwrap_or(p.target_mw);
        let pick = |key: &str, derived: f64| self.values.get(key).copied().unwrap_or(derived);
        Site {
            gross_peak_mw: gross,
            study_fee_usd: self.get_or("study_fee_usd", 0.0),
            security_usd_per_mw: self.get_or("security_usd_per_mw", 0.0),
            forfeiture_share: self.get_or("forfeiture_share", 0.0),
            release_tranche_share: self.get_or("release_tranche_share", 0.2),
            months_to_energization: self.get_or("months_to_energization", 0.0),
            milestone_interval_months: self.get_or("milestone_interval_months", 12.0),
            collateral_yield: self.get_or("collateral_yield", 0.0),
            withdraw_probability: self.get_or("withdraw_probability", 0.0),
            sla_ld_usd_per_mwh: self.get_or("sla_ld_usd_per_mwh", 0.0),
            reliability_service_mw: self.get_or("reliability_service_mw", 0.0),
            reliability_service_usd_per_mw_year: self.get_or("reliability_service_usd_per_mw_year", 0.0),
            computational_share: self.get_or("computational_share", 0.0),
            backup_gen_mw: self.get_or("backup_gen_mw", 0.0),
            backup_deploy_hours_per_year: self.get_or("backup_deploy_hours_per_year", 0.0),
            disconnect_test_minutes: self.get_or("disconnect_test_minutes", 30.0),
            u_min: self.get_or("u_min", 0.4),
            beta: self.get_or("beta", 0.75),
            sla_slack_hours: self.get_or("sla_slack_hours", 0.0),
            flexible_share: pick("flexible_share", p.defer_share.min(0.95)),
            training_share_of_flexible: self.get_or("training_share_of_flexible", 0.5),
            training_slack_multiplier: self.get_or("training_slack_multiplier", 1.0),
            batch_value_usd_per_mwh: self.get_or("batch_value_usd_per_mwh", 0.0),
            training_value_usd_per_mwh: self.get_or("training_value_usd_per_mwh", 0.0),
            critical_value_usd_per_mwh: self.get_or("critical_value_usd_per_mwh", 0.0),
            required_shed_mw: pick("required_shed_mw", (gross - e.ceiling_mw).clamp(0.0, 5000.0)),
            bess_degradation_usd_per_mwh: self.get_or("bess_degradation_usd_per_mwh", 0.0),
            poi_import_limit_mw: pick("poi_import_limit_mw", gross.min(5000.0)),
            max_ramp_mw_per_min: self.get_or("max_ramp_mw_per_min", 10.0),
            power_factor: self.get_or("power_factor", 1.0),
            pf_floor: self.get_or("pf_floor", 0.95),
            milestone_energize_month: pick("milestone_energize_month", (p.interim_month + p.bess_lead).min(240.0)),
            it_power_kw: pick("it_power_kw", (gross * 1000.0).min(5e6)),
            pue: self.get_or("pue", 1.3),
            pue_target: self.get_or("pue_target", 1.3),
            renewable_share: self.get_or("renewable_share", 0.0),
            flex_readiness_target: self.get_or("flex_readiness_target", 0.1),
            participates_in_reliability_service: self.flag("participates_in_reliability_service"),
            price_responsive_lmp_curtailment: self.flag("price_responsive_lmp_curtailment"),
            firm_shed_telemetry: self.flag("firm_shed_telemetry"),
            interconnected_after_2025: self.flag("interconnected_after_2025"),
            ride_through_certified: self.flag("ride_through_certified"),
            backup_disclosed: self.flag("backup_disclosed"),
            eed_metering_ready: self.flag("eed_metering_ready"),
            renewable_disclosed: self.flag("renewable_disclosed"),
            va_rate_uplift_share: self.get_or("va_rate_uplift_share", 0.0),
            uk_commitment_fee_usd_per_mw: self.get_or("uk_commitment_fee_usd_per_mw", 0.0),
            ie_renewable_derated_mw: self.get_or("ie_renewable_derated_mw", 0.0),
            ie_storage_derate: self.get_or("ie_storage_derate", 0.0),
            ie_gen_derate: self.get_or("ie_gen_derate", 0.0),
            wue_m3_per_mwh: self.get_or("wue_m3_per_mwh", 0.0),
            hourly_renewable_match: self.get_or("hourly_renewable_match", 0.0),
            es_hourly_target: self.get_or("es_hourly_target", 0.0),
            green_energy_share: self.get_or("green_energy_share", 0.0),
            va_dominion_territory: self.flag("va_dominion_territory"),
            backup_tier4: self.flag("backup_tier4"),
            uk_gate2_ready: self.flag("uk_gate2_ready"),
            uk_end_user_evidence: self.flag("uk_end_user_evidence"),
            uk_long_lead_procured: self.flag("uk_long_lead_procured"),
            uk_capability_evidence: self.flag("uk_capability_evidence"),
            es_interruptible_opt_in: self.flag("es_interruptible_opt_in"),
        }
    }
}

/// Fully resolved site (every derived default filled in).
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub struct Site {
    pub gross_peak_mw: f64,
    pub study_fee_usd: f64,
    pub security_usd_per_mw: f64,
    pub forfeiture_share: f64,
    pub release_tranche_share: f64,
    pub months_to_energization: f64,
    pub milestone_interval_months: f64,
    pub collateral_yield: f64,
    pub withdraw_probability: f64,
    pub sla_ld_usd_per_mwh: f64,
    pub reliability_service_mw: f64,
    pub reliability_service_usd_per_mw_year: f64,
    pub computational_share: f64,
    pub backup_gen_mw: f64,
    pub backup_deploy_hours_per_year: f64,
    pub disconnect_test_minutes: f64,
    pub u_min: f64,
    pub beta: f64,
    pub sla_slack_hours: f64,
    pub flexible_share: f64,
    pub training_share_of_flexible: f64,
    pub training_slack_multiplier: f64,
    pub batch_value_usd_per_mwh: f64,
    pub training_value_usd_per_mwh: f64,
    pub critical_value_usd_per_mwh: f64,
    pub required_shed_mw: f64,
    pub bess_degradation_usd_per_mwh: f64,
    pub poi_import_limit_mw: f64,
    pub max_ramp_mw_per_min: f64,
    pub power_factor: f64,
    pub pf_floor: f64,
    pub milestone_energize_month: f64,
    pub it_power_kw: f64,
    pub pue: f64,
    pub pue_target: f64,
    pub renewable_share: f64,
    pub flex_readiness_target: f64,
    pub participates_in_reliability_service: bool,
    pub price_responsive_lmp_curtailment: bool,
    pub firm_shed_telemetry: bool,
    pub interconnected_after_2025: bool,
    pub ride_through_certified: bool,
    pub backup_disclosed: bool,
    pub eed_metering_ready: bool,
    pub renewable_disclosed: bool,
    pub va_rate_uplift_share: f64,
    pub uk_commitment_fee_usd_per_mw: f64,
    pub ie_renewable_derated_mw: f64,
    pub ie_storage_derate: f64,
    pub ie_gen_derate: f64,
    pub wue_m3_per_mwh: f64,
    pub hourly_renewable_match: f64,
    pub es_hourly_target: f64,
    pub green_energy_share: f64,
    pub va_dominion_territory: bool,
    pub backup_tier4: bool,
    pub uk_gate2_ready: bool,
    pub uk_end_user_evidence: bool,
    pub uk_long_lead_procured: bool,
    pub uk_capability_evidence: bool,
    pub es_interruptible_opt_in: bool,
}

impl Site {
    /// Canonical value (every key of [`SITE_SPECS`]).
    pub fn to_value(&self) -> Value {
        let f = num;
        obj(vec![
            ("gross_peak_mw", f(self.gross_peak_mw)),
            ("study_fee_usd", f(self.study_fee_usd)),
            ("security_usd_per_mw", f(self.security_usd_per_mw)),
            ("forfeiture_share", f(self.forfeiture_share)),
            ("release_tranche_share", f(self.release_tranche_share)),
            ("months_to_energization", f(self.months_to_energization)),
            ("milestone_interval_months", f(self.milestone_interval_months)),
            ("collateral_yield", f(self.collateral_yield)),
            ("withdraw_probability", f(self.withdraw_probability)),
            ("sla_ld_usd_per_mwh", f(self.sla_ld_usd_per_mwh)),
            ("reliability_service_mw", f(self.reliability_service_mw)),
            ("reliability_service_usd_per_mw_year", f(self.reliability_service_usd_per_mw_year)),
            ("computational_share", f(self.computational_share)),
            ("backup_gen_mw", f(self.backup_gen_mw)),
            ("backup_deploy_hours_per_year", f(self.backup_deploy_hours_per_year)),
            ("disconnect_test_minutes", f(self.disconnect_test_minutes)),
            ("u_min", f(self.u_min)),
            ("beta", f(self.beta)),
            ("sla_slack_hours", f(self.sla_slack_hours)),
            ("flexible_share", f(self.flexible_share)),
            ("training_share_of_flexible", f(self.training_share_of_flexible)),
            ("training_slack_multiplier", f(self.training_slack_multiplier)),
            ("batch_value_usd_per_mwh", f(self.batch_value_usd_per_mwh)),
            ("training_value_usd_per_mwh", f(self.training_value_usd_per_mwh)),
            ("critical_value_usd_per_mwh", f(self.critical_value_usd_per_mwh)),
            ("required_shed_mw", f(self.required_shed_mw)),
            ("bess_degradation_usd_per_mwh", f(self.bess_degradation_usd_per_mwh)),
            ("poi_import_limit_mw", f(self.poi_import_limit_mw)),
            ("max_ramp_mw_per_min", f(self.max_ramp_mw_per_min)),
            ("power_factor", f(self.power_factor)),
            ("pf_floor", f(self.pf_floor)),
            ("milestone_energize_month", f(self.milestone_energize_month)),
            ("it_power_kw", f(self.it_power_kw)),
            ("pue", f(self.pue)),
            ("pue_target", f(self.pue_target)),
            ("renewable_share", f(self.renewable_share)),
            ("flex_readiness_target", f(self.flex_readiness_target)),
            ("participates_in_reliability_service", Value::Bool(self.participates_in_reliability_service)),
            ("price_responsive_lmp_curtailment", Value::Bool(self.price_responsive_lmp_curtailment)),
            ("firm_shed_telemetry", Value::Bool(self.firm_shed_telemetry)),
            ("interconnected_after_2025", Value::Bool(self.interconnected_after_2025)),
            ("ride_through_certified", Value::Bool(self.ride_through_certified)),
            ("backup_disclosed", Value::Bool(self.backup_disclosed)),
            ("eed_metering_ready", Value::Bool(self.eed_metering_ready)),
            ("renewable_disclosed", Value::Bool(self.renewable_disclosed)),
            ("va_rate_uplift_share", f(self.va_rate_uplift_share)),
            ("uk_commitment_fee_usd_per_mw", f(self.uk_commitment_fee_usd_per_mw)),
            ("ie_renewable_derated_mw", f(self.ie_renewable_derated_mw)),
            ("ie_storage_derate", f(self.ie_storage_derate)),
            ("ie_gen_derate", f(self.ie_gen_derate)),
            ("wue_m3_per_mwh", f(self.wue_m3_per_mwh)),
            ("hourly_renewable_match", f(self.hourly_renewable_match)),
            ("es_hourly_target", f(self.es_hourly_target)),
            ("green_energy_share", f(self.green_energy_share)),
            ("va_dominion_territory", Value::Bool(self.va_dominion_territory)),
            ("backup_tier4", Value::Bool(self.backup_tier4)),
            ("uk_gate2_ready", Value::Bool(self.uk_gate2_ready)),
            ("uk_end_user_evidence", Value::Bool(self.uk_end_user_evidence)),
            ("uk_long_lead_procured", Value::Bool(self.uk_long_lead_procured)),
            ("uk_capability_evidence", Value::Bool(self.uk_capability_evidence)),
            ("es_interruptible_opt_in", Value::Bool(self.es_interruptible_opt_in)),
        ])
    }
}

/// Jurisdiction rulepacks. Every variant is listed in [`JurisdictionRulepack::ALL`] and evaluated by
/// [`compliance_report`]; adding a corridor touches only this module (and its own rule module), never
/// the simulator or the dossier code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JurisdictionRulepack {
    /// Texas SB 6 / 16 TAC Sec. 25.194 / ERCOT protocols.
    ErcotSb6_2026,
    /// FERC RM26-4 (anticipated terms; no final rule).
    FercRm26_4,
    /// Virginia SB 253 high-load cost allocation (+ HB 507 Tier IV backup, HB 284 demand flexibility).
    VirginiaSb253_2026,
    /// GB NESO Gate 2 + Ofgem proposed data-centre connection reforms.
    UkNeso2026,
    /// Ireland CRU/2025/236 Large Energy Users connection policy.
    IrelandCru2026,
    /// Spain Real Decreto 1032/2026 data-centre connection conditions.
    SpainRd2026,
    /// Singapore Green Data Centre Roadmap / DC-CFA2.
    SingaporeGreenDc2026,
    /// EU Energy Efficiency Directive Art. 12 reporting.
    EuEed2026,
}

/// Legal status of a rulepack as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulepackStatus {
    /// Statute and adopted rule in force.
    StatuteAndRule,
    /// Regulator-approved protocol or decision in force.
    AdoptedPolicy,
    /// Proposed or anticipated terms (no final rule).
    Anticipated,
    /// Reporting obligation only.
    ReportingObligation,
}

impl RulepackStatus {
    /// Machine string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StatuteAndRule => "statute-and-rule",
            Self::AdoptedPolicy => "adopted-policy",
            Self::Anticipated => "anticipated-no-final-rule",
            Self::ReportingObligation => "reporting-obligation",
        }
    }
}

impl JurisdictionRulepack {
    /// Every rulepack, in the fixed evaluation and tie-break order (identical to `JUR_ORDER` in the
    /// web reference).
    pub const ALL: [JurisdictionRulepack; 8] = [
        Self::ErcotSb6_2026,
        Self::FercRm26_4,
        Self::VirginiaSb253_2026,
        Self::UkNeso2026,
        Self::IrelandCru2026,
        Self::SpainRd2026,
        Self::SingaporeGreenDc2026,
        Self::EuEed2026,
    ];

    /// Parse a CLI id (see [`JurisdictionRulepack::cli_id`]) or a short alias.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "ercot-sb6" | "ercot" | "tx-sb6" => Some(Self::ErcotSb6_2026),
            "ferc-rm26-4" | "ferc" | "pjm" => Some(Self::FercRm26_4),
            "va-sb253" | "virginia" | "va" => Some(Self::VirginiaSb253_2026),
            "uk-neso" | "uk" | "gb" | "neso" => Some(Self::UkNeso2026),
            "ie-cru" | "ireland" | "ie" | "cru" => Some(Self::IrelandCru2026),
            "es-rd" | "spain" | "es" => Some(Self::SpainRd2026),
            "sg-gdc" | "singapore" | "sg" => Some(Self::SingaporeGreenDc2026),
            "eu-eed" | "eed" | "eu" => Some(Self::EuEed2026),
            _ => None,
        }
    }

    /// CLI id.
    pub fn cli_id(self) -> &'static str {
        match self {
            Self::ErcotSb6_2026 => "ercot-sb6",
            Self::FercRm26_4 => "ferc-rm26-4",
            Self::VirginiaSb253_2026 => "va-sb253",
            Self::UkNeso2026 => "uk-neso",
            Self::IrelandCru2026 => "ie-cru",
            Self::SpainRd2026 => "es-rd",
            Self::SingaporeGreenDc2026 => "sg-gdc",
            Self::EuEed2026 => "eu-eed",
        }
    }

    /// Versioned rulepack id (`YYYY.MM` = month of the source text the rules were checked against).
    pub fn id(self) -> &'static str {
        match self {
            Self::ErcotSb6_2026 => "TX-SB6-16TAC25.194-2026.10",
            Self::FercRm26_4 => "US-FERC-RM26-4-2026.01",
            Self::VirginiaSb253_2026 => "US-VA-SB253-2026.06",
            Self::UkNeso2026 => "GB-NESO-OFGEM-DC-2026.09",
            Self::IrelandCru2026 => "IE-CRU-2025-236-2026.05",
            Self::SpainRd2026 => "ES-RD-1032-2026.11",
            Self::SingaporeGreenDc2026 => "SG-IMDA-GDCR-DCCFA2-2026.03",
            Self::EuEed2026 => "EU-EED-ART12-2026.01",
        }
    }

    /// Display name.
    pub fn name(self) -> &'static str {
        match self {
            Self::ErcotSb6_2026 => "ERCOT / Texas SB 6 large-load rules",
            Self::FercRm26_4 => "FERC RM26-4 large-load interconnection (anticipated terms)",
            Self::VirginiaSb253_2026 => "Virginia SB 253 high-load cost allocation (+ HB 507, HB 284)",
            Self::UkNeso2026 => "GB NESO Gate 2 + Ofgem data-centre connection reforms (proposed)",
            Self::IrelandCru2026 => "Ireland CRU Large Energy Users connection policy",
            Self::SpainRd2026 => "Spain Real Decreto 1032/2026 data-centre connection conditions",
            Self::SingaporeGreenDc2026 => "Singapore Green Data Centre Roadmap / DC-CFA2",
            Self::EuEed2026 => "EU Energy Efficiency Directive Art. 12 reporting",
        }
    }

    /// Legal status of the rulepack as a whole.
    pub fn status(self) -> RulepackStatus {
        match self {
            Self::ErcotSb6_2026 | Self::VirginiaSb253_2026 | Self::SpainRd2026 => RulepackStatus::StatuteAndRule,
            Self::IrelandCru2026 | Self::SingaporeGreenDc2026 => RulepackStatus::AdoptedPolicy,
            Self::FercRm26_4 | Self::UkNeso2026 => RulepackStatus::Anticipated,
            Self::EuEed2026 => RulepackStatus::ReportingObligation,
        }
    }

    /// Rules this rulepack can emit, in evaluation order.
    pub fn rules(self) -> &'static [RuleId] {
        use RuleId::*;
        match self {
            Self::ErcotSb6_2026 => &[
                Sb6Applicability,
                Sb6FinancialSecurity,
                Sb6IllegalRevenueStack,
                Sb6FirmShedTelemetry,
                Sb6Disconnect30m,
                ErcotNogrr282,
                Sb6BackupDisclosure,
                TceqAirPermitCap,
            ],
            Self::FercRm26_4 => &[FercDocketStatus, FercPoiImport, FercRampRate, FercPowerFactor, FercStagedEnergization],
            Self::VirginiaSb253_2026 => &[VaApplicability, VaCostAllocation, VaTier4Backup, VaDemandFlex],
            Self::UkNeso2026 => &[UkGate2, UkEndUser, UkLongLead, UkCapability, UkCommitmentFee],
            Self::IrelandCru2026 => &[IeApplicability, IeDispatchableMatch, IeRunHours, IeRenewable80],
            Self::SpainRd2026 => &[EsApplicability, EsPue, EsWue, EsHourlyRenewable, EsReporting],
            Self::SingaporeGreenDc2026 => &[SgPueGdcr, SgPueCfa, SgGreen50, SgCfaWindow],
            Self::EuEed2026 => &[EedApplicability, EedReportingReadiness, EedPue, EedRenewableDisclosure, EedFlexReadiness],
        }
    }
}

/// Stable rule identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum RuleId {
    Sb6Applicability,
    Sb6FinancialSecurity,
    Sb6IllegalRevenueStack,
    Sb6FirmShedTelemetry,
    Sb6Disconnect30m,
    ErcotNogrr282,
    Sb6BackupDisclosure,
    TceqAirPermitCap,
    FercDocketStatus,
    FercPoiImport,
    FercRampRate,
    FercPowerFactor,
    FercStagedEnergization,
    EedApplicability,
    EedReportingReadiness,
    EedPue,
    EedRenewableDisclosure,
    EedFlexReadiness,
    VaApplicability,
    VaCostAllocation,
    VaTier4Backup,
    VaDemandFlex,
    UkGate2,
    UkEndUser,
    UkLongLead,
    UkCapability,
    UkCommitmentFee,
    IeApplicability,
    IeDispatchableMatch,
    IeRunHours,
    IeRenewable80,
    EsApplicability,
    EsPue,
    EsWue,
    EsHourlyRenewable,
    EsReporting,
    SgPueGdcr,
    SgPueCfa,
    SgGreen50,
    SgCfaWindow,
}

impl RuleId {
    /// Machine code (dossier `rule_id`).
    pub fn code(self) -> &'static str {
        match self {
            Self::Sb6Applicability => "SB6_APPLICABILITY",
            Self::Sb6FinancialSecurity => "SB6_FINANCIAL_SECURITY",
            Self::Sb6IllegalRevenueStack => "SB6_ILLEGAL_REVENUE_STACK",
            Self::Sb6FirmShedTelemetry => "SB6_FIRM_SHED_TELEMETRY",
            Self::Sb6Disconnect30m => "SB6_DISCONNECT_30M",
            Self::ErcotNogrr282 => "ERCOT_NOGRR282",
            Self::Sb6BackupDisclosure => "SB6_BACKUP_DISCLOSURE",
            Self::TceqAirPermitCap => "TCEQ_AIR_PERMIT_CAP",
            Self::FercDocketStatus => "FERC_DOCKET_STATUS",
            Self::FercPoiImport => "FERC_POI_IMPORT",
            Self::FercRampRate => "FERC_RAMP_RATE",
            Self::FercPowerFactor => "FERC_POWER_FACTOR",
            Self::FercStagedEnergization => "FERC_STAGED_ENERGIZATION",
            Self::EedApplicability => "EED_APPLICABILITY",
            Self::EedReportingReadiness => "EED_REPORTING_READINESS",
            Self::EedPue => "EED_PUE",
            Self::EedRenewableDisclosure => "EED_RENEWABLE_DISCLOSURE",
            Self::EedFlexReadiness => "EED_FLEX_READINESS",
            Self::VaApplicability => "VA_APPLICABILITY",
            Self::VaCostAllocation => "VA_COST_ALLOCATION",
            Self::VaTier4Backup => "VA_TIER4_BACKUP",
            Self::VaDemandFlex => "VA_DEMAND_FLEX",
            Self::UkGate2 => "UK_GATE2",
            Self::UkEndUser => "UK_END_USER",
            Self::UkLongLead => "UK_LONG_LEAD",
            Self::UkCapability => "UK_CAPABILITY",
            Self::UkCommitmentFee => "UK_COMMITMENT_FEE",
            Self::IeApplicability => "IE_APPLICABILITY",
            Self::IeDispatchableMatch => "IE_DISPATCHABLE_MATCH",
            Self::IeRunHours => "IE_RUN_HOURS",
            Self::IeRenewable80 => "IE_RENEWABLE_80",
            Self::EsApplicability => "ES_APPLICABILITY",
            Self::EsPue => "ES_PUE",
            Self::EsWue => "ES_WUE",
            Self::EsHourlyRenewable => "ES_HOURLY_RENEWABLE",
            Self::EsReporting => "ES_REPORTING",
            Self::SgPueGdcr => "SG_PUE_GDCR",
            Self::SgPueCfa => "SG_PUE_CFA",
            Self::SgGreen50 => "SG_GREEN_50",
            Self::SgCfaWindow => "SG_CFA_WINDOW",
        }
    }

    /// Citation text.
    pub fn citation(self) -> &'static str {
        match self {
            Self::Sb6Applicability => "PURA Sec. 37.0561(c) (Tex. SB 6, 2025): 75 MW demand threshold; 16 TAC Sec. 25.194 (PUCT Project 58481, adoption order filed 2026-09-18): requested peak demand is not netted for on-site generation",
            Self::Sb6FinancialSecurity => "PURA Sec. 37.0561(f): flat study fee of at least $100,000; 16 TAC Sec. 25.194: security of $50,000/MW, 20% of security tied to allocated capacity retained on withdrawal after allocation, 20% returned at energization and the rest in 20% increments per SLLIA milestones, cash collateral earns at least the 1-month T-bill rate",
            Self::Sb6IllegalRevenueStack => "PURA Sec. 39.170(b)(3) (Tex. SB 6): the ERCOT large-load reliability service must exclude any large load that curtails in response to the wholesale price of electricity or participates in a different reliability or ancillary service",
            Self::Sb6FirmShedTelemetry => "PURA Sec. 39.170(a) (Tex. SB 6): equipment allowing transmission-voltage load interconnected after 2025-12-31 to be curtailed during firm load shed; ERCOT PGRR 145 Sec. 9.2.5 (breakers under remote transmission-operator control)",
            Self::Sb6Disconnect30m => "Planning stress test, not a codified time limit: survival of a firm-load-shed curtailment under PURA Sec. 39.170(a). The PUCT declined to codify a 30-minute curtailment window in the 16 TAC Sec. 25.194 adoption order",
            Self::ErcotNogrr282 => "ERCOT NOGRR 282, Large Computational Load ride-through (PUCT approved 2026-07-09, effective 2026-08-01); Large Electronic Load = at least 75 MW with at least 50% computational load (NPRR 1308 definition)",
            Self::Sb6BackupDisclosure => "PURA Sec. 37.0561 (Tex. SB 6): disclosure of on-site backup generating facilities that can serve at least 50% of on-site demand; ERCOT may direct their deployment or load curtailment in an energy emergency",
            Self::TceqAirPermitCap => "Site air permit (TCEQ) run-hour limit; SB 6 does not authorize violating emissions limits; RICE NESHAP 40 CFR 63.6640(f) limits emergency engines to 50 h/yr of non-emergency use",
            Self::FercDocketStatus => "FERC Docket RM26-4-000 (DOE large-load ANOPR); 2026-06-18 show-cause orders to six RTOs/ISOs incl. new transmission services for flexible large loads. No final rule: these checks model anticipated interconnection-agreement terms",
            Self::FercPoiImport => "Interconnection-agreement POI import limit (anticipated flexible large-load service, FERC RM26-4 show-cause orders)",
            Self::FercRampRate => "Interconnection-agreement ramp-rate limit (anticipated; NERC large-load reliability guidance)",
            Self::FercPowerFactor => "Interconnection-agreement power-factor requirement at the POI (anticipated)",
            Self::FercStagedEnergization => "Interconnection-agreement staged-energization milestone (anticipated)",
            Self::EedApplicability => "Directive (EU) 2023/1791 Art. 12 and Annex VII; Delegated Regulation (EU) 2024/1364: annual KPI reporting for data centres with installed IT power demand of at least 500 kW (due 15 May each year)",
            Self::EedReportingReadiness => "Directive (EU) 2023/1791 Art. 12(1) and Annex VII; Delegated Regulation (EU) 2024/1364 Annexes I-II (KPI data collection)",
            Self::EedPue => "PUE is reported under Delegated Regulation (EU) 2024/1364; the target used here is national or contractual, NOT an EED Art. 12 ceiling",
            Self::EedRenewableDisclosure => "Directive (EU) 2023/1791 Annex VII (renewable energy share and guarantees of origin)",
            Self::EedFlexReadiness => "Grid-flexibility readiness score (firmgap metric; 1-hour certified shed / gross peak). Not an EED requirement",
            Self::VaApplicability => "Va. SB 253 (2026 Session, Lucas), passed with the Governor amendments on 2026-04-22: the SCC may assign PJM capacity-auction and distribution/substation financing costs to high-load customers; 25 MW threshold as reported in bill summaries",
            Self::VaCostAllocation => "Va. SB 253 (2026): any cost shift requires State Corporation Commission approval; the uplift rate is an assumption based on the SCC staff estimate reported in press",
            Self::VaTier4Backup => "Va. HB 507 (2026, McAuliff): DEQ must deny data-centre generator air permits submitted after July 2026 unless the units are Tier IV or equivalent",
            Self::VaDemandFlex => "Va. HB 284 / SB 371 (2026, signed 2026-06-30): Dominion and Appalachian Power must develop voluntary demand-flexibility programs for high-demand customers",
            Self::UkGate2 => "NESO Connections Reform (Gate 2 to Whole Queue; methodologies approved by Ofgem in 2025): a Gate 2 offer requires the readiness and strategic-alignment criteria",
            Self::UkEndUser => "Ofgem consultation Proposed data centre connection reforms (Curate pillar, 2026-07-29 to 2026-09-17, awaiting decision): queue-management milestone - credible end user",
            Self::UkLongLead => "Ofgem consultation Proposed data centre connection reforms (2026, awaiting decision): queue-management milestone - procurement of long-lead electrical equipment",
            Self::UkCapability => "Ofgem consultation Proposed data centre connection reforms (2026, awaiting decision): queue-management milestone - financial and technical capability",
            Self::UkCommitmentFee => "Ofgem consultation Proposed data centre connection reforms (2026, awaiting decision): commitment fee secured from offer acceptance to readiness and not returned if the project fails the rules; amount not yet set",
            Self::IeApplicability => "CRU/2025/236 Large Energy Users Connection Policy decision (2025-12-12); EirGrid Data Centre Connection Offer Process and Policy v3 (May 2026): tiers at 1 MVA and 10 MVA of Maximum Import Capacity",
            Self::IeDispatchableMatch => "CRU/2025/236: dispatchable on-site or proximate generation and/or storage (MIC of 10 MVA or more) or an autoproducer (1 to 10 MVA) matching MIC on a de-rated CRM T-4 basis and trading in the SEM; up to 80% of the de-rated renewable match may count",
            Self::IeRunHours => "CRU/2025/236: the units must not be bound by limited run hours outside routine maintenance; the System Operators reduce MIC if availability falls short",
            Self::IeRenewable80 => "CRU/2025/236: at least 80% of annual demand from additional renewable generation in Ireland, six-year glide path from energisation, annual reporting",
            Self::EsApplicability => "Real Decreto 1032/2026 (BOE 2026-11-21, in force 2026-11-22): data centres with grid access capacity of 10 MW or more, including same-owner groups on one site",
            Self::EsPue => "RD 1032/2026: PUE of 1.25 or less until the EU data-centre rating label (expected August 2027), enforced two years after start of operation",
            Self::EsWue => "RD 1032/2026: WUE of 0.1 m3/MWh or less until the EU label, enforced one year after start of operation",
            Self::EsHourlyRenewable => "RD 1032/2026: 80% of consumption matched hour by hour with new renewables (plants connected at most 36 months before start) until renewables exceed 90% of the mix; transitional interruptible path for five years; automatic compensation charges and suspension of connection rights for up to three years",
            Self::EsReporting => "RD 1032/2026: sites above 500 kW of IT power report efficiency and sustainability data to MITECO each year; above 1 MW they declare EU Code of Conduct adherence",
            Self::SgPueGdcr => "IMDA Green Data Centre Roadmap (2024-05-30): PUE target of 1.3 or lower for new data centres",
            Self::SgPueCfa => "EDB/IMDA DC-CFA2 (opened 2025-12-01, closed 2026-03-31): best-in-class efficiency; 1.25 PUE at full load as reported for capacity allocations",
            Self::SgGreen50 => "EDB/IMDA DC-CFA2: at least 50% of power from approved green-energy pathways",
            Self::SgCfaWindow => "EDB/IMDA DC-CFA2 allocated at least 200 MW; its application window closed 2026-03-31",
        }
    }
}

/// Check severity (a failed `Blocker` blocks bankability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Statutory, regulatory or contractual blocker.
    Blocker,
    /// Diligence warning.
    Warning,
    /// Informational.
    Info,
}

impl Severity {
    /// Dossier string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Blocker => "Blocker",
            Self::Warning => "Warning",
            Self::Info => "Info",
        }
    }
}

/// Overall compliance verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceVerdict {
    /// No failed blockers or warnings.
    BankablePass,
    /// Only warnings failed.
    ConditionalWarning,
    /// At least one blocker failed.
    StatutoryBlocker,
}

impl ComplianceVerdict {
    /// Parse the dossier string (case-insensitive).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
            "bankablepass" => Some(Self::BankablePass),
            "conditionalwarning" => Some(Self::ConditionalWarning),
            "statutoryblocker" => Some(Self::StatutoryBlocker),
            _ => None,
        }
    }

    /// Dossier string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BankablePass => "BankablePass",
            Self::ConditionalWarning => "ConditionalWarning",
            Self::StatutoryBlocker => "StatutoryBlocker",
        }
    }
}

/// One evaluated rule.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleCheck {
    /// Rule.
    pub rule_id: RuleId,
    /// Severity if failed.
    pub severity: Severity,
    /// Passed.
    pub passed: bool,
    /// Observed value.
    pub observed: Option<f64>,
    /// Limit.
    pub limit: Option<f64>,
    /// Unit.
    pub unit: &'static str,
    /// Human summary with the numbers.
    pub summary: String,
}

/// Compliance-related financial drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplianceFinancials {
    /// Study fee ($).
    pub study_fee_usd: f64,
    /// Financial security posted ($), on gross requested peak demand.
    pub security_locked_usd: f64,
    /// Full-balance-equivalent months the security is locked.
    pub security_months_equivalent: f64,
    /// Working-capital carry at (WACC − collateral yield) ($).
    pub working_capital_carry_usd: f64,
    /// Security retained if withdrawn after allocation ($).
    pub forfeiture_exposure_usd: f64,
    /// `withdraw_probability x exposure` ($).
    pub expected_forfeiture_usd: f64,
    /// Liquidated damages on SLA-late compute ($/yr).
    pub sla_ld_exposure_usd_per_year: f64,
    /// carry + expected forfeiture + one year of SLA LDs ($).
    pub total_drag_usd: f64,
}

/// A parameter change proposed by a remediation (`bool` changes use 0/1).
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeValue {
    /// Number.
    Num(f64),
    /// Flag.
    Bool(bool),
}

/// Counterfactual path to compliance.
#[derive(Debug, Clone, PartialEq)]
pub struct RemediationOption {
    /// Stable id.
    pub id: &'static str,
    /// Rule it fixes.
    pub rule_id: RuleId,
    /// Title.
    pub title: &'static str,
    /// What to do, with the numbers.
    pub action: String,
    /// Parameter changes (`key`, value) that implement it.
    pub changes: Vec<(&'static str, ChangeValue)>,
    /// Annual value of the path ($/yr).
    pub value_usd_per_year: f64,
    /// 5-year PV of that value at WACC ($).
    pub pv_5y_usd: f64,
    /// Value minus the alternative path ($/yr).
    pub delta_vs_other_usd_per_year: f64,
    /// Additional CapEx ($).
    pub capex_usd: f64,
    /// Recommended option for its rule.
    pub recommended: bool,
    /// Whether applying it turns the rule green.
    pub resolves: bool,
}

/// Full compliance report.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceReport {
    /// Rulepack.
    pub rulepack: JurisdictionRulepack,
    /// Versioned rulepack id.
    pub rulepack_id: &'static str,
    /// Verdict.
    pub verdict: ComplianceVerdict,
    /// Checks in evaluation order.
    pub checks: Vec<RuleCheck>,
    /// Financial drag.
    pub financial_drag: ComplianceFinancials,
    /// Remediations.
    pub remediations: Vec<RemediationOption>,
}

/// Fixed-decimal formatting identical to the JavaScript reference `fx` (round half up).
pub fn fx(x: f64, d: u32) -> String {
    if !x.is_finite() {
        return "n/a".into();
    }
    let scale = match d {
        0 => 1.0,
        1 => 10.0,
        2 => 100.0,
        3 => 1000.0,
        _ => 10000.0,
    };
    let k = round_half_up(x * scale);
    let k = if k == 0.0 { 0.0 } else { k };
    let neg = k < 0.0;
    let mut digits = format!("{:.0}", k.abs());
    if d > 0 {
        let d = d as usize;
        while digits.len() < d + 1 {
            digits.insert(0, '0');
        }
        let cut = digits.len() - d;
        digits = format!("{}.{}", &digits[..cut], &digits[cut..]);
    }
    if neg {
        format!("-{digits}")
    } else {
        digits
    }
}

/// Compact dollars identical to the JavaScript reference `usdTxt`.
pub fn usd_txt(x: f64) -> String {
    if !x.is_finite() {
        return "n/a".into();
    }
    let a = x.abs();
    let sg = if x < 0.0 { "-" } else { "" };
    if a >= 1e6 {
        format!("{sg}${}M", fx(a / 1e6, 2))
    } else if a >= 1e3 {
        format!("{sg}${}k", fx(a / 1e3, 1))
    } else {
        format!("{sg}${}", fx(a, 0))
    }
}

/// Remediations never propose more BESS than the web workbench input range (`bessMW` max 60 MW).
pub const REM_MAX_BESS_MW: f64 = 60.0;

pub(crate) fn round_up(x: f64, step: f64) -> f64 {
    (x / step - 1e-9).ceil() * step
}

pub(crate) fn annuity(rate: f64, years: f64) -> f64 {
    if rate == 0.0 {
        years
    } else {
        (1.0 - (1.0 + rate).powf(-years)) / rate
    }
}

/// Certified shed at a duration (hours) for a site.
pub fn shed_at_duration(st: &Site, dt: f64) -> f64 {
    certified_shed(&shed_classes(st), dt)
}

/// Result of the disconnect stress test.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisconnectNeed {
    /// Test duration (h).
    pub dt: f64,
    /// Load left after certified shed (MW).
    pub residual: f64,
    /// Power the BESS must supply after backup generation (MW).
    pub need_p: f64,
    /// Energy the BESS must supply (MWh).
    pub need_e: f64,
    /// Survives.
    pub ok: bool,
    /// Available BESS power after derate (MW).
    pub bess_mw: f64,
    /// Available BESS energy after SoC window (MWh).
    pub bess_mwh: f64,
}

/// Evaluate the full-disconnect stress test.
pub fn disconnect_need(st: &Site, p: &Params) -> DisconnectNeed {
    let e = p.effective();
    let dt = st.disconnect_test_minutes / 60.0;
    let residual = (st.gross_peak_mw - shed_at_duration(st, dt)).max(0.0);
    let need_p = (residual - st.backup_gen_mw).max(0.0);
    let ok = need_p <= e.bess_mw + 1e-9 && need_p * dt <= e.bess_mwh + 1e-9;
    DisconnectNeed { dt, residual, need_p, need_e: need_p * dt, ok, bess_mw: e.bess_mw, bess_mwh: e.bess_mwh }
}

fn gen_hours_for(p: &Params, year: &Year, mw: f64) -> u32 {
    let mut q = p.clone();
    q.bess_mw = mw;
    simulate(&q, year, &SimConfig::flex(&q, false)).gen_hrs
}

/// How a rulepack ties up money: security posted, study fee, the share forfeited on withdrawal, and
/// whether the security is released in SLLIA tranches.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FinancialTerms {
    /// Security posted ($).
    pub security: f64,
    /// Study fee ($).
    pub study_fee: f64,
    /// Share of the security forfeited on withdrawal.
    pub forfeit_share: f64,
    /// Released in tranches at energization and SLLIA milestones (SB 6).
    pub tranches: bool,
    /// Report the locked months.
    pub show: bool,
}

impl FinancialTerms {
    /// No financial security.
    pub fn none() -> Self {
        Self { security: 0.0, study_fee: 0.0, forfeit_share: 0.0, tranches: false, show: false }
    }

    /// Texas SB 6 / 16 TAC Sec. 25.194 terms.
    pub fn sb6(st: &Site, applicable: bool) -> Self {
        Self {
            security: if applicable { st.security_usd_per_mw * st.gross_peak_mw } else { 0.0 },
            study_fee: if applicable { st.study_fee_usd } else { 0.0 },
            forfeit_share: st.forfeiture_share,
            tranches: true,
            show: applicable,
        }
    }
}

/// Compliance financial drag for a site under SB 6 terms (`applicable`) or no terms.
pub fn compliance_financials(st: &Site, p: &Params, r: &Run, applicable: bool) -> ComplianceFinancials {
    financials_with(st, p, r, &FinancialTerms::sb6(st, applicable))
}

/// Compliance financial drag for explicit terms.
pub fn financials_with(st: &Site, p: &Params, r: &Run, t: &FinancialTerms) -> ComplianceFinancials {
    let security = t.security;
    let rm = (1.0 + p.wacc).powf(1.0 / 12.0) - 1.0;
    let ry = (1.0 + st.collateral_yield).powf(1.0 / 12.0) - 1.0;
    let net = (rm - ry).max(0.0);
    let mut locked = st.months_to_energization;
    if t.tranches {
        let n_tr = (1.0 / st.release_tranche_share - 1e-9).ceil().max(0.0) as u32;
        for j in 1..=n_tr {
            locked += st.milestone_interval_months * (1.0 - f64::from(j) * st.release_tranche_share).max(0.0);
        }
    }
    let carry = security * net * locked;
    let exposure = t.forfeit_share * security;
    let expected = st.withdraw_probability * exposure;
    let ld = st.sla_ld_usd_per_mwh * r.flex.breach;
    ComplianceFinancials {
        study_fee_usd: t.study_fee,
        security_locked_usd: security,
        security_months_equivalent: if t.show { locked } else { 0.0 },
        working_capital_carry_usd: carry,
        forfeiture_exposure_usd: exposure,
        expected_forfeiture_usd: expected,
        sla_ld_exposure_usd_per_year: ld,
        total_drag_usd: carry + expected + ld,
    }
}

pub(crate) fn check(
    rule_id: RuleId,
    severity: Severity,
    passed: bool,
    observed: Option<f64>,
    limit: Option<f64>,
    unit: &'static str,
    summary: String,
) -> RuleCheck {
    RuleCheck { rule_id, severity, passed, observed, limit, unit, summary }
}

pub(crate) fn verdict_for(checks: &[RuleCheck]) -> ComplianceVerdict {
    if checks.iter().any(|c| !c.passed && c.severity == Severity::Blocker) {
        ComplianceVerdict::StatutoryBlocker
    } else if checks.iter().any(|c| !c.passed && c.severity == Severity::Warning) {
        ComplianceVerdict::ConditionalWarning
    } else {
        ComplianceVerdict::BankablePass
    }
}

pub(crate) fn flag(v: bool) -> f64 {
    if v {
        1.0
    } else {
        0.0
    }
}

/// Evaluate a rulepack for a resolved site and a completed run (`r` must include the flex trace
/// for the FERC rulepack; without it the POI and ramp checks use the annual peak gap only).
pub fn compliance_report(rp: JurisdictionRulepack, st: &Site, p: &Params, r: &Run) -> ComplianceReport {
    match rp {
        JurisdictionRulepack::ErcotSb6_2026 => ercot(st, p, r),
        JurisdictionRulepack::FercRm26_4 => ferc(st, p, r),
        JurisdictionRulepack::VirginiaSb253_2026 => crate::rulepacks_intl::virginia(st, p, r),
        JurisdictionRulepack::UkNeso2026 => crate::rulepacks_intl::uk(st, p, r),
        JurisdictionRulepack::IrelandCru2026 => crate::rulepacks_intl::ireland(st, p, r),
        JurisdictionRulepack::SpainRd2026 => crate::rulepacks_intl::spain(st, p, r),
        JurisdictionRulepack::SingaporeGreenDc2026 => crate::rulepacks_intl::singapore(st, p, r),
        JurisdictionRulepack::EuEed2026 => eed(st, p, r),
    }
}

fn ercot(st: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let f = &r.flex;
    let mut checks = Vec::new();
    let mut rem: Vec<RemediationOption> = Vec::new();
    let applicable = st.gross_peak_mw >= 75.0;
    let g = st.gross_peak_mw;
    checks.push(check(
        Sb6Applicability,
        Severity::Info,
        true,
        Some(g),
        Some(75.0),
        "MW",
        if applicable {
            format!("Requested peak demand {} MW is at or above 75 MW: large-load standards apply (on-site BESS and generation do not reduce it).", fx(g, 1))
        } else {
            format!("Requested peak demand {} MW is below 75 MW: SB 6 large-load standards do not apply; remaining SB 6 checks are informational.", fx(g, 1))
        },
    ));
    let fin = compliance_financials(st, p, r, applicable);
    checks.push(check(
        Sb6FinancialSecurity,
        Severity::Info,
        true,
        Some(fin.security_locked_usd),
        None,
        "$",
        if applicable {
            format!(
                "Security {} ({}/MW x {} MW) plus study fee {}; carry {}, forfeiture exposure {}.",
                usd_txt(fin.security_locked_usd),
                usd_txt(st.security_usd_per_mw),
                fx(g, 1),
                usd_txt(fin.study_fee_usd),
                usd_txt(fin.working_capital_carry_usd),
                usd_txt(fin.forfeiture_exposure_usd)
            )
        } else {
            "Not applicable below 75 MW.".into()
        },
    ));
    let both = st.participates_in_reliability_service && st.price_responsive_lmp_curtailment;
    checks.push(check(
        Sb6IllegalRevenueStack,
        if applicable { Severity::Blocker } else { Severity::Info },
        !(applicable && both),
        Some(flag(both)),
        Some(0.0),
        "flag",
        if !applicable {
            "Not applicable below 75 MW.".into()
        } else if both {
            "Ineligible stack: the site enrolls in the ERCOT large-load reliability service AND curtails on wholesale prices. Choose one path.".into()
        } else {
            "Revenue stack is eligible (reliability service and price-responsive curtailment are not combined).".into()
        },
    ));
    if applicable && both {
        let lmp_value = (f.thr_e + f.bes_e) * (p.stress_price - p.base_price).max(0.0);
        let rs_value = st.reliability_service_mw * st.reliability_service_usd_per_mw_year;
        let af = annuity(p.wacc, 5.0);
        let a_best = lmp_value >= rs_value;
        rem.push(RemediationOption {
            id: "PATH_A_LMP_ONLY",
            rule_id: Sb6IllegalRevenueStack,
            title: "Path A: price-responsive (LMP) curtailment only",
            action: "Leave the ERCOT large-load reliability service; keep curtailing on wholesale prices.".into(),
            changes: vec![("participates_in_reliability_service", ChangeValue::Bool(false))],
            value_usd_per_year: lmp_value,
            pv_5y_usd: lmp_value * af,
            delta_vs_other_usd_per_year: lmp_value - rs_value,
            capex_usd: 0.0,
            recommended: a_best,
            resolves: true,
        });
        rem.push(RemediationOption {
            id: "PATH_B_RELIABILITY_ONLY",
            rule_id: Sb6IllegalRevenueStack,
            title: "Path B: ERCOT reliability service only",
            action: format!(
                "Stop price-responsive curtailment; commit {} MW to the reliability service (24-hour notice, curtailed for the full emergency).",
                fx(st.reliability_service_mw, 1)
            ),
            changes: vec![("price_responsive_lmp_curtailment", ChangeValue::Bool(false))],
            value_usd_per_year: rs_value,
            pv_5y_usd: rs_value * af,
            delta_vs_other_usd_per_year: rs_value - lmp_value,
            capex_usd: 0.0,
            recommended: !a_best,
            resolves: true,
        });
    }
    let need_tel = applicable && st.interconnected_after_2025;
    checks.push(check(
        Sb6FirmShedTelemetry,
        if need_tel { Severity::Blocker } else { Severity::Info },
        !need_tel || st.firm_shed_telemetry,
        Some(flag(st.firm_shed_telemetry)),
        Some(flag(need_tel)),
        "flag",
        if !need_tel {
            "Not required (below 75 MW or interconnected before 2026).".into()
        } else if st.firm_shed_telemetry {
            "Remote curtailment equipment for firm load shed is in place.".into()
        } else {
            "Missing remote curtailment equipment required before interconnection.".into()
        },
    ));
    let d = disconnect_need(st, p);
    checks.push(check(
        Sb6Disconnect30m,
        if applicable { Severity::Warning } else { Severity::Info },
        !applicable || d.ok,
        Some(d.need_p),
        Some(d.bess_mw),
        "MW",
        if !applicable {
            "Not applicable below 75 MW.".into()
        } else {
            format!(
                "{}{}-minute disconnect: residual load after certified shed {} MW, BESS must supply {} MW / {} MWh (available {} MW / {} MWh).",
                if d.ok { "Survives a " } else { "Does NOT survive a " },
                fx(st.disconnect_test_minutes, 0),
                fx(d.residual, 2),
                fx(d.need_p, 2),
                fx(d.need_e, 2),
                fx(d.bess_mw, 2),
                fx(d.bess_mwh, 2)
            )
        },
    ));
    if applicable && !d.ok {
        let h = p.bess_h;
        let mw_req = (d.need_p / p.bess_derate).max(d.need_e / (h * p.soc_window));
        let mw_apply = round_up(mw_req, 0.1).min(REM_MAX_BESS_MW);
        let d_mw = (mw_apply - p.bess_mw).max(0.0);
        rem.push(RemediationOption {
            id: "DISCONNECT_ADD_BESS",
            rule_id: Sb6Disconnect30m,
            title: "Add BESS capacity",
            action: format!(
                "Raise BESS to {} MW / {} MWh (+{} MW / +{} MWh).",
                fx(mw_apply, 1),
                fx(mw_apply * h, 1),
                fx(d_mw, 1),
                fx(d_mw * h, 1)
            ),
            changes: vec![("bess_mw", ChangeValue::Num(qz(mw_apply).unwrap_or(mw_apply)))],
            value_usd_per_year: 0.0,
            pv_5y_usd: 0.0,
            delta_vs_other_usd_per_year: 0.0,
            capex_usd: capex_power_energy(p, d_mw, d_mw * h),
            recommended: false,
            resolves: mw_req <= REM_MAX_BESS_MW,
        });
        let ok_at = |sl: f64| {
            let mut t = st.clone();
            t.sla_slack_hours = sl;
            disconnect_need(&t, p).ok
        };
        if ok_at(48.0) {
            let (mut lo, mut hi) = (st.sla_slack_hours, 48.0f64);
            for _ in 0..64 {
                let mid = (lo + hi) / 2.0;
                if ok_at(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            let sl_apply = round_up(hi, 0.01).min(48.0);
            rem.push(RemediationOption {
                id: "DISCONNECT_MORE_SLA_SLACK",
                rule_id: Sb6Disconnect30m,
                title: "Negotiate more SLA slack for flexible batch",
                action: format!(
                    "Raise batch SLA slack from {} h to {} h (training x{}), so more load is certified sheddable.",
                    fx(st.sla_slack_hours, 2),
                    fx(sl_apply, 2),
                    fx(st.training_slack_multiplier, 1)
                ),
                changes: vec![("sla_slack_hours", ChangeValue::Num(qz(sl_apply).unwrap_or(sl_apply)))],
                value_usd_per_year: 0.0,
                pv_5y_usd: 0.0,
                delta_vs_other_usd_per_year: 0.0,
                capex_usd: 0.0,
                recommended: true,
                resolves: true,
            });
        }
        // stable sort by capex (same as Array.prototype.sort in the reference)
        rem.sort_by(|a, b| a.capex_usd.partial_cmp(&b.capex_usd).unwrap_or(std::cmp::Ordering::Equal));
        let mut first = true;
        for o in rem.iter_mut().filter(|o| o.rule_id == Sb6Disconnect30m) {
            o.recommended = first && o.resolves;
            first = false;
        }
    }
    let lel = applicable && st.computational_share >= 0.5;
    checks.push(check(
        ErcotNogrr282,
        if lel { Severity::Warning } else { Severity::Info },
        !lel || st.ride_through_certified,
        Some(st.computational_share),
        Some(0.5),
        "fraction",
        if !lel {
            "Not a Large Electronic Load (below 75 MW or below 50% computational load).".into()
        } else if st.ride_through_certified {
            "Large Electronic Load: ride-through capability demonstrated.".into()
        } else {
            "Large Electronic Load: ride-through performance must be demonstrated (NOGRR 282).".into()
        },
    ));
    let need_disc = applicable && st.backup_gen_mw >= 0.5 * g;
    checks.push(check(
        Sb6BackupDisclosure,
        if need_disc { Severity::Blocker } else { Severity::Info },
        !need_disc || st.backup_disclosed,
        Some(st.backup_gen_mw),
        Some(0.5 * g),
        "MW",
        if !applicable {
            "Not applicable below 75 MW.".into()
        } else if !need_disc {
            "Backup generation below 50% of on-site demand: no SB 6 backup disclosure trigger.".into()
        } else if st.backup_disclosed {
            "Backup generation covers at least 50% of demand and is disclosed.".into()
        } else {
            "Backup generation covers at least 50% of demand but is NOT disclosed.".into()
        },
    ));
    let has_gen = p.gen_mw > 0.0 || st.backup_gen_mw > 0.0;
    let deploy = if st.backup_gen_mw > 0.0 { st.backup_deploy_hours_per_year } else { 0.0 };
    let gen_hrs = f64::from(f.gen_hrs);
    let need = gen_hrs + deploy;
    let permit_ok = !has_gen || need <= p.gen_cap_h;
    checks.push(check(
        TceqAirPermitCap,
        Severity::Blocker,
        permit_ok,
        Some(if has_gen { need } else { 0.0 }),
        Some(p.gen_cap_h),
        "h/yr",
        if !has_gen {
            "No on-site generation: no air-permit exposure.".into()
        } else {
            format!(
                "Generators need {} h/yr ({} h curtailment + {} h directed backup) against a permit cap of {} h/yr (headroom {} h).",
                fx(need, 0),
                fx(gen_hrs, 0),
                fx(deploy, 0),
                fx(p.gen_cap_h, 0),
                fx(p.gen_cap_h - need, 0)
            )
        },
    ));
    if !permit_ok {
        let renegotiate = |action: String| RemediationOption {
            id: "PERMIT_RENEGOTIATE",
            rule_id: TceqAirPermitCap,
            title: "Amend the air permit or reduce backup deployment exposure",
            action,
            changes: vec![],
            value_usd_per_year: 0.0,
            pv_5y_usd: 0.0,
            delta_vs_other_usd_per_year: 0.0,
            capex_usd: 0.0,
            recommended: true,
            resolves: false,
        };
        if deploy > p.gen_cap_h {
            rem.push(renegotiate("Directed backup deployment alone exceeds the permit cap; storage cannot fix this.".into()));
        } else {
            let ok_mw = |mw: f64| f64::from(gen_hours_for(p, &r.year, mw)) + deploy <= p.gen_cap_h;
            let hi_mw = (p.bess_mw + g).min(REM_MAX_BESS_MW);
            if ok_mw(hi_mw) {
                let (mut lo, mut hi) = (p.bess_mw, hi_mw);
                for _ in 0..64 {
                    let mid = (lo + hi) / 2.0;
                    if ok_mw(mid) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let mw_apply = round_up(hi, 0.1).min(REM_MAX_BESS_MW);
                let d_mw = (mw_apply - p.bess_mw).max(0.0);
                rem.push(RemediationOption {
                    id: "PERMIT_ADD_BESS",
                    rule_id: TceqAirPermitCap,
                    title: "Displace genset hours with BESS",
                    action: format!(
                        "Raise BESS to {} MW (+{} MW / +{} MWh) so generators stay within {} h/yr.",
                        fx(mw_apply, 1),
                        fx(d_mw, 1),
                        fx(d_mw * p.bess_h, 1),
                        fx(p.gen_cap_h, 0)
                    ),
                    changes: vec![("bess_mw", ChangeValue::Num(qz(mw_apply).unwrap_or(mw_apply)))],
                    value_usd_per_year: 0.0,
                    pv_5y_usd: 0.0,
                    delta_vs_other_usd_per_year: 0.0,
                    capex_usd: capex_power_energy(p, d_mw, d_mw * p.bess_h),
                    recommended: true,
                    resolves: true,
                });
            } else {
                rem.push(renegotiate(format!("No BESS size up to {} MW keeps generators within the cap.", fx(hi_mw, 0))));
            }
        }
    }
    let verdict = verdict_for(&checks);
    let rp = JurisdictionRulepack::ErcotSb6_2026;
    ComplianceReport { rulepack: rp, rulepack_id: rp.id(), verdict, checks, financial_drag: fin, remediations: rem }
}

fn ferc(st: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let f = &r.flex;
    let (mut max_grid, mut max_step) = (0.0f64, 0.0f64);
    if let Some(tr) = f.trace.as_ref() {
        for t in 0..HRS.min(tr.grid.len()) {
            if tr.grid[t] > max_grid {
                max_grid = tr.grid[t];
            }
            if t > 0 {
                let d = (tr.grid[t] - tr.grid[t - 1]).abs();
                if d > max_step {
                    max_step = d;
                }
            }
        }
    }
    let onset = if p.ramp_min > 0.0 { f.max_gap / p.ramp_min } else { 0.0 };
    let ramp = (max_step / 60.0).max(onset);
    let mut checks = vec![check(
        FercDocketStatus,
        Severity::Info,
        true,
        None,
        None,
        "",
        "RM26-4 has no final rule; checks reflect anticipated flexible large-load interconnection terms.".into(),
    )];
    checks.push(check(
        FercPoiImport,
        Severity::Blocker,
        max_grid <= st.poi_import_limit_mw + 1e-9,
        Some(max_grid),
        Some(st.poi_import_limit_mw),
        "MW",
        format!("Peak hourly grid import {} MW vs POI limit {} MW.", fx(max_grid, 2), fx(st.poi_import_limit_mw, 2)),
    ));
    checks.push(check(
        FercRampRate,
        Severity::Warning,
        ramp <= st.max_ramp_mw_per_min + 1e-9,
        Some(ramp),
        Some(st.max_ramp_mw_per_min),
        "MW/min",
        format!(
            "Worst ramp {} MW/min (event onset {}, hourly {}) vs limit {} MW/min.",
            fx(ramp, 2),
            fx(onset, 2),
            fx(max_step / 60.0, 2),
            fx(st.max_ramp_mw_per_min, 2)
        ),
    ));
    checks.push(check(
        FercPowerFactor,
        Severity::Warning,
        st.power_factor >= st.pf_floor - 1e-12,
        Some(st.power_factor),
        Some(st.pf_floor),
        "-",
        format!("Power factor {} vs floor {}.", fx(st.power_factor, 3), fx(st.pf_floor, 3)),
    ));
    let ready = p.interim_month + p.bess_lead;
    checks.push(check(
        FercStagedEnergization,
        Severity::Warning,
        ready <= st.milestone_energize_month + 1e-9,
        Some(ready),
        Some(st.milestone_energize_month),
        "month",
        format!("Bridge ready at month {} vs milestone month {}.", fx(ready, 0), fx(st.milestone_energize_month, 0)),
    ));
    let fin = compliance_financials(st, p, r, false);
    let rp = JurisdictionRulepack::FercRm26_4;
    ComplianceReport {
        rulepack: rp,
        rulepack_id: rp.id(),
        verdict: verdict_for(&checks),
        checks,
        financial_drag: fin,
        remediations: vec![],
    }
}

fn eed(st: &Site, p: &Params, r: &Run) -> ComplianceReport {
    use RuleId::*;
    let applicable = st.it_power_kw >= 500.0;
    let shed1 = shed_at_duration(st, 1.0);
    let score = if st.gross_peak_mw > 0.0 { shed1 / st.gross_peak_mw } else { 0.0 };
    let mut checks = vec![check(
        EedApplicability,
        Severity::Info,
        true,
        Some(st.it_power_kw),
        Some(500.0),
        "kW",
        if applicable {
            format!("Installed IT power {} kW: annual Art. 12 KPI reporting applies.", fx(st.it_power_kw, 0))
        } else {
            "Below 500 kW: Art. 12 reporting does not apply.".into()
        },
    )];
    let sev = if applicable { Severity::Warning } else { Severity::Info };
    checks.push(check(
        EedReportingReadiness,
        sev,
        !applicable || st.eed_metering_ready,
        Some(flag(st.eed_metering_ready)),
        Some(1.0),
        "flag",
        if st.eed_metering_ready {
            "KPI metering and data collection in place.".into()
        } else {
            "KPI metering not ready for the 15 May reporting deadline.".into()
        },
    ));
    checks.push(check(
        EedPue,
        sev,
        !applicable || st.pue <= st.pue_target + 1e-12,
        Some(st.pue),
        Some(st.pue_target),
        "-",
        format!("PUE {} vs target {} (national or contractual target).", fx(st.pue, 2), fx(st.pue_target, 2)),
    ));
    checks.push(check(
        EedRenewableDisclosure,
        sev,
        !applicable || st.renewable_disclosed,
        Some(st.renewable_share),
        None,
        "fraction",
        format!(
            "Renewable share {}%{}",
            fx(st.renewable_share * 100.0, 1),
            if st.renewable_disclosed { ", disclosure prepared." } else { ", disclosure NOT prepared." }
        ),
    ));
    checks.push(check(
        EedFlexReadiness,
        sev,
        !applicable || score >= st.flex_readiness_target - 1e-12,
        Some(score),
        Some(st.flex_readiness_target),
        "fraction",
        format!(
            "Certified 1-hour shed {} MW = {}% of gross peak (target {}%).",
            fx(shed1, 2),
            fx(score * 100.0, 1),
            fx(st.flex_readiness_target * 100.0, 1)
        ),
    ));
    let fin = compliance_financials(st, p, r, false);
    let rp = JurisdictionRulepack::EuEed2026;
    ComplianceReport {
        rulepack: rp,
        rulepack_id: rp.id(),
        verdict: verdict_for(&checks),
        checks,
        financial_drag: fin,
        remediations: vec![],
    }
}

impl ComplianceReport {
    /// Dossier representation (keys match the JavaScript reference).
    pub fn to_value(&self) -> Value {
        let checks = self
            .checks
            .iter()
            .map(|c| {
                obj(vec![
                    ("rule_id", s(c.rule_id.code())),
                    ("citation", s(c.rule_id.citation())),
                    ("severity", s(c.severity.as_str())),
                    ("passed", Value::Bool(c.passed)),
                    ("observed", opt(c.observed)),
                    ("limit", opt(c.limit)),
                    ("unit", s(c.unit)),
                    ("summary", Value::Str(c.summary.clone())),
                ])
            })
            .collect();
        let fd = &self.financial_drag;
        let fin = obj(vec![
            ("study_fee_usd", num(fd.study_fee_usd)),
            ("security_locked_usd", num(fd.security_locked_usd)),
            ("security_months_equivalent", num(fd.security_months_equivalent)),
            ("working_capital_carry_usd", num(fd.working_capital_carry_usd)),
            ("forfeiture_exposure_usd", num(fd.forfeiture_exposure_usd)),
            ("expected_forfeiture_usd", num(fd.expected_forfeiture_usd)),
            ("sla_ld_exposure_usd_per_year", num(fd.sla_ld_exposure_usd_per_year)),
            ("total_drag_usd", num(fd.total_drag_usd)),
        ]);
        let rems = self
            .remediations
            .iter()
            .map(|o| {
                let changes = o
                    .changes
                    .iter()
                    .map(|(k, v)| {
                        obj(vec![
                            ("key", s(k)),
                            (
                                "value",
                                match v {
                                    ChangeValue::Num(x) => num(*x),
                                    ChangeValue::Bool(b) => Value::Bool(*b),
                                },
                            ),
                        ])
                    })
                    .collect();
                obj(vec![
                    ("id", s(o.id)),
                    ("rule_id", s(o.rule_id.code())),
                    ("title", s(o.title)),
                    ("action", Value::Str(o.action.clone())),
                    ("changes", Value::Arr(changes)),
                    ("value_usd_per_year", num(o.value_usd_per_year)),
                    ("pv_5y_usd", num(o.pv_5y_usd)),
                    ("delta_vs_other_usd_per_year", num(o.delta_vs_other_usd_per_year)),
                    ("capex_usd", num(o.capex_usd)),
                    ("recommended", Value::Bool(o.recommended)),
                    ("resolves", Value::Bool(o.resolves)),
                ])
            })
            .collect();
        obj(vec![
            ("rulepack_id", s(self.rulepack_id)),
            ("jurisdiction", s(self.rulepack.cli_id())),
            ("verdict", s(self.verdict.as_str())),
            ("checks", Value::Arr(checks)),
            ("financial_drag", fin),
            ("remediations", Value::Arr(rems)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_matches_reference_rounding() {
        assert_eq!(fx(2.5, 0), "3");
        assert_eq!(fx(-2.5, 0), "-2");
        assert_eq!(fx(0.004, 2), "0.00");
        assert_eq!(fx(-0.004, 2), "0.00");
        assert_eq!(fx(42.75, 2), "42.75");
        assert_eq!(fx(0.05, 1), "0.1");
        assert_eq!(fx(100.0, 1), "100.0");
        assert_eq!(fx(f64::NAN, 1), "n/a");
        assert_eq!(usd_txt(5_000_000.0), "$5.00M");
        assert_eq!(usd_txt(944_060.4), "$944.1k");
        assert_eq!(usd_txt(-12.4), "-$12");
    }

    #[test]
    fn site_inputs_validate_and_default() {
        let mut si = SiteInputs::default();
        assert!(si.set("gross_peak_mw", "100").is_ok());
        assert!(si.set("gross_peak_mw", "-1").is_err());
        assert!(si.set("firm_shed_telemetry", "maybe").is_err());
        assert!(si.set("nope", "1").is_err());
        assert!(si.set("participates_in_reliability_service", "true").is_ok());
        let st = si.resolve(&Params::default());
        assert_eq!(st.gross_peak_mw, 100.0);
        assert!(st.participates_in_reliability_service);
        assert_eq!(st.security_usd_per_mw, 50000.0);
        assert_eq!(st.flexible_share, 0.35);
    }

    #[test]
    fn rulepack_ids_roundtrip() {
        let mut ids = std::collections::BTreeSet::new();
        for rp in JurisdictionRulepack::ALL {
            assert_eq!(JurisdictionRulepack::parse(rp.cli_id()), Some(rp));
            assert!(ids.insert(rp.id()), "duplicate rulepack id");
            assert!(!rp.rules().is_empty());
            for r in rp.rules() {
                assert!(!r.citation().is_empty() && r.code().bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'));
            }
        }
        assert_eq!(JurisdictionRulepack::parse("mars"), None);
    }
}
