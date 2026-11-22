// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Model inputs, their supported ranges, and validated setters.
//!
//! Every numeric input is declared once in the `numeric_params!` table below.
//! That single declaration generates the struct field, its default (the
//! KilowattBridge case study), the [`SPECS`] metadata used by the CLI and
//! documentation, and the getter/setter, so the three cannot drift apart.

use crate::error::Error;

/// Order in which flexible resources cover the gap above the curtailment ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Defer batch compute first, then discharge BESS, then run gensets.
    ThrottleFirst,
    /// Discharge BESS first, then defer batch compute, then run gensets.
    BessFirst,
}

impl Strategy {
    /// Canonical name used in scenario files and JSON.
    pub fn as_str(self) -> &'static str {
        match self {
            Strategy::ThrottleFirst => "throttle",
            Strategy::BessFirst => "bess",
        }
    }

    /// Parse `throttle` / `bess` (case-insensitive, `-first` suffix optional).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "throttle" | "throttle-first" | "throttle_first" => Some(Strategy::ThrottleFirst),
            "bess" | "bess-first" | "bess_first" => Some(Strategy::BessFirst),
            _ => None,
        }
    }
}

/// Metadata for one numeric parameter.
#[derive(Debug, Clone, Copy)]
pub struct ParamSpec {
    /// Field / scenario-file key.
    pub key: &'static str,
    /// Inclusive minimum accepted value.
    pub min: f64,
    /// Inclusive maximum accepted value.
    pub max: f64,
    /// Case-study default.
    pub default: f64,
    /// Display unit.
    pub unit: &'static str,
    /// One-line description.
    pub help: &'static str,
}

macro_rules! numeric_params {
    ($( $field:ident : [$min:expr, $max:expr] = $default:expr, $unit:literal, $help:literal; )*) => {
        /// Complete set of model inputs. `Params::default()` is the case study.
        #[derive(Debug, Clone, PartialEq)]
        pub struct Params {
            $( #[doc = $help] pub $field: f64, )*
            /// Dispatch priority between throttling and BESS.
            pub strategy: Strategy,
            /// Seed of the synthetic weather / congestion year.
            pub seed: u32,
        }

        impl Default for Params {
            fn default() -> Self {
                Self { $( $field: $default, )* strategy: Strategy::ThrottleFirst, seed: 2026 }
            }
        }

        /// Supported domain and documentation of every numeric parameter.
        pub const SPECS: &[ParamSpec] = &[
            $( ParamSpec { key: stringify!($field), min: $min, max: $max, default: $default, unit: $unit, help: $help }, )*
        ];

        impl Params {
            /// Read a numeric parameter (or `seed`) by key.
            pub fn get(&self, key: &str) -> Option<f64> {
                match key {
                    $( stringify!($field) => Some(self.$field), )*
                    "seed" => Some(f64::from(self.seed)),
                    _ => None,
                }
            }

            fn slot(&mut self, key: &str) -> Option<&mut f64> {
                match key {
                    $( stringify!($field) => Some(&mut self.$field), )*
                    _ => None,
                }
            }
        }
    };
}

numeric_params! {
    // ---- interconnection & phasing ----
    queue_months:    [1.0, 120.0]   = 38.0,    "months",   "Months until firm interconnection (cluster study + network upgrades).";
    interim_month:   [0.0, 36.0]    = 4.0,     "month",    "Month the interim non-firm service is energized.";
    interim_mw:      [1.0, 1000.0]  = 22.0,    "MW",       "Capacity granted under the interim non-firm agreement.";
    ceiling_mw:      [0.0, 1000.0]  = 22.0,    "MW",       "Maximum grid draw allowed during a utility curtailment signal.";
    bess_lead:       [0.0, 36.0]    = 6.0,     "months",   "Months from interim energization to a commissioned, witness-tested bridge.";
    horizon:         [12.0, 240.0]  = 60.0,    "months",   "Evaluation horizon for pathway economics.";
    // ---- campus load & workload flexibility ----
    target_mw:       [1.0, 2000.0]  = 45.0,    "MW",       "Steady-state campus IT load at full build-out.";
    util:            [0.05, 1.0]    = 0.78,    "fraction", "Blended average utilization (mean of the hourly profile).";
    defer_share:     [0.0, 0.9]     = 0.35,    "fraction", "Share of load that is deferrable batch / training work.";
    fast_cap:        [0.0, 0.5]     = 0.10,    "fraction", "Load share shed instantly by GPU power-capping (before scheduler preemption).";
    ramp_min:        [0.5, 60.0]    = 4.0,     "minutes",  "Scheduler preemption latency; only fast resources count before it elapses.";
    sla_window:      [1.0, 48.0]    = 4.0,     "hours",    "Deadline for replaying deferred batch work before it counts as an SLA breach.";
    max_throttle_h:  [1.0, 24.0]    = 4.0,     "hours",    "Throttle hold limit: no new deferrals after this many hours into an event.";
    // ---- grid stress & tariff ----
    stress_hours:    [0.0, 600.0]   = 180.0,   "h/yr",     "Curtailment-signal hours per year (coincident with high-LMP hours).";
    avg_event_h:     [1.0, 12.0]    = 3.5,     "hours",    "Mean curtailment event duration (exponential, clipped to 1-12 h).";
    base_price:      [0.0, 1000.0]  = 68.0,    "$/MWh",    "Average grid energy price outside stress hours.";
    stress_price:    [0.0, 10000.0] = 310.0,   "$/MWh",    "Average LMP during stress / curtailment hours.";
    penalty:         [0.0, 20000.0] = 450.0,   "$/MWh",    "Non-compliance penalty per MWh drawn above the ceiling during a signal.";
    // ---- bridge assets ----
    bess_mw:         [0.0, 1000.0]  = 23.0,    "MW",       "Battery power rating.";
    bess_h:          [0.5, 12.0]    = 4.0,     "hours",    "Battery duration (energy = power x duration).";
    bess_cost:       [0.0, 2000.0]  = 285.0,   "$/kWh",    "Installed cost of a 4-hour reference system.";
    bess_power_cost: [0.0, 2000.0]  = 250.0,   "$/kW",     "Power-block share of cost (PCS, interconnect); energy block is calibrated so 4 h = bess_cost.";
    rte:             [0.5, 1.0]     = 0.88,    "fraction", "Battery round-trip efficiency.";
    gen_mw:          [0.0, 1000.0]  = 0.0,     "MW",       "On-site genset capacity available for curtailment events.";
    gen_cap_h:       [0.0, 8760.0]  = 50.0,    "h/yr",     "Air-permit cap on non-emergency genset run hours.";
    gen_cost:        [0.0, 5000.0]  = 290.0,   "$/MWh",    "Genset fuel plus variable O&M.";
    gen_rental:      [0.0, 1000.0]  = 65.0,    "$/kW-yr",  "Genset rental, charged during flex-bridge months only.";
    // ---- finance ----
    carry:           [0.0, 1.0e9]   = 145000.0, "$/month", "Queue-delay carry for the full campus (scales with un-energized share).";
    wacc:            [0.0, 0.5]     = 0.09,    "fraction", "Discount rate used to annualize BESS CapEx.";
    life:            [1.0, 40.0]    = 10.0,    "years",    "BESS economic life.";
    sla_target:      [0.0001, 0.5]  = 0.015,   "fraction", "Maximum SLA-breached share of annual compute energy.";
    // ---- v0.2 engineering derates (defaults reproduce model v0.1 exactly) ----
    safety_margin:   [0.0, 50.0]    = 0.0,     "MW",       "Control deadband: target draw this far below the ceiling (meter error, telemetry lag).";
    bess_derate:     [0.5, 1.0]     = 1.0,     "fraction", "Share of nameplate BESS power available in every event (N-1 inverter, thermal derate).";
    soc_window:      [0.5, 1.0]     = 1.0,     "fraction", "Usable share of nameplate BESS energy (SoC reserve + warranty window).";
    // ---- v0.2 investor layer ----
    defer_cost:      [0.0, 100000.0] = 0.0,    "$/MWh",    "Fee paid to workload owners per MWh deferred.";
    sla_penalty:     [0.0, 1.0e6]   = 0.0,     "$/MWh",    "Penalty per MWh of deferred work replayed after its SLA window.";
    salvage:         [0.0, 1.0]     = 0.0,     "fraction", "BESS resale value as a share of CapEx (used if it beats post-firm arbitrage).";
    margin_pre_energy: [0.0, 100000.0] = 320.0, "$/MWh",   "Gross margin before energy per incremental MWh of compute (ASSUMPTION: replace with contracted figures).";
    sw_setup:        [0.0, 1.0e9]   = 250000.0, "$",       "Software onboarding fee (value-capture hypothesis).";
    sw_price_mw_month: [0.0, 1.0e7] = 1500.0,  "$/MW-mo",  "Software fee per unlocked MW per bridge month (value-capture hypothesis).";
    mc_seeds:        [2.0, 1000.0]  = 24.0,    "years",    "Independent synthetic weather years for Monte Carlo robustness statistics.";
}

fn unquote(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && ((t.starts_with('"') && t.ends_with('"')) || (t.starts_with('\'') && t.ends_with('\''))) {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// Ceiling and BESS capability after engineering derates (deadband, availability, SoC window).
/// CapEx is always charged on nameplate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effective {
    /// Grid draw target during a signal: `ceiling_mw - safety_margin` (MW).
    pub ceiling_mw: f64,
    /// Available BESS power: `bess_mw x bess_derate` (MW).
    pub bess_mw: f64,
    /// Usable BESS energy: `bess_mw x bess_h x soc_window` (MWh).
    pub bess_mwh: f64,
}

/// Look up the spec of a numeric parameter.
pub fn spec(key: &str) -> Option<&'static ParamSpec> {
    SPECS.iter().find(|s| s.key == key)
}

impl Params {
    /// Battery energy capacity in MWh.
    pub fn bess_mwh(&self) -> f64 {
        self.bess_mw * self.bess_h
    }

    /// Set any parameter from text, enforcing type and range.
    ///
    /// ```
    /// let mut p = firmgap::Params::default();
    /// assert!(p.set("bess_mw", "13.5").is_ok());
    /// assert!(p.set("bess_mw", "NaN").is_err());
    /// assert!(p.set("bess_mw", "-1").is_err());
    /// assert!(p.set("no_such_key", "1").is_err());
    /// ```
    pub fn set(&mut self, key: &str, raw: &str) -> Result<(), Error> {
        let key = key.trim();
        let value = unquote(raw);
        match key {
            "strategy" => {
                self.strategy = Strategy::parse(value).ok_or_else(|| Error::InvalidValue {
                    key: key.to_string(),
                    value: value.to_string(),
                    reason: "expected 'throttle' or 'bess'",
                })?;
                return Ok(());
            }
            "seed" => {
                self.seed = value.replace('_', "").parse::<u32>().map_err(|_| Error::InvalidValue {
                    key: key.to_string(),
                    value: value.to_string(),
                    reason: "expected an integer in 0..=4294967295",
                })?;
                return Ok(());
            }
            _ => {}
        }
        let sp = spec(key).ok_or_else(|| Error::UnknownKey(key.to_string()))?;
        let v: f64 = value.replace('_', "").parse().map_err(|_| Error::InvalidValue {
            key: key.to_string(),
            value: value.to_string(),
            reason: "expected a number",
        })?;
        if !v.is_finite() {
            return Err(Error::InvalidValue { key: key.to_string(), value: value.to_string(), reason: "must be a finite number" });
        }
        if !(sp.min..=sp.max).contains(&v) {
            return Err(Error::OutOfRange { key: key.to_string(), value: v, min: sp.min, max: sp.max });
        }
        match self.slot(key) {
            Some(s) => {
                *s = v;
                Ok(())
            }
            None => Err(Error::UnknownKey(key.to_string())),
        }
    }

    /// Set a numeric parameter from a number, enforcing finiteness and range.
    pub fn set_f64(&mut self, key: &str, v: f64) -> Result<(), Error> {
        let sp = spec(key).ok_or_else(|| Error::UnknownKey(key.to_string()))?;
        if !v.is_finite() {
            return Err(Error::InvalidValue { key: key.to_string(), value: v.to_string(), reason: "must be a finite number" });
        }
        if !(sp.min..=sp.max).contains(&v) {
            return Err(Error::OutOfRange { key: key.to_string(), value: v, min: sp.min, max: sp.max });
        }
        self.set_unchecked(key, v);
        Ok(())
    }

    /// Internal: set a numeric field without validation. Used only by analytics that deliberately
    /// probe beyond the input domain (break-even bisection) and never by user-facing paths.
    /// Returns false for unknown keys.
    pub(crate) fn set_unchecked(&mut self, key: &str, v: f64) -> bool {
        match self.slot(key) {
            Some(s) => {
                *s = v;
                true
            }
            None => false,
        }
    }

    /// Copy with one parameter changed, clamped to its supported range and with cross-field
    /// inconsistencies repaired (the same rules as the web workbench). Used for stress perturbations.
    pub fn perturbed(&self, key: &str, v: f64) -> Params {
        let mut q = self.clone();
        if let Some(sp) = spec(key) {
            let v = if v.is_finite() { v.clamp(sp.min, sp.max) } else { sp.default };
            q.set_unchecked(key, v);
        }
        q.repair();
        q
    }

    /// Repair cross-field inconsistencies in place and return a description of each change.
    pub fn repair(&mut self) -> Vec<String> {
        let mut fixes = Vec::new();
        if self.interim_mw > self.target_mw {
            fixes.push(format!("interim_mw {} -> {} (cannot exceed target_mw)", self.interim_mw, self.target_mw));
            self.interim_mw = self.target_mw;
        }
        if self.ceiling_mw > self.target_mw {
            fixes.push(format!("ceiling_mw {} -> {} (cannot exceed target_mw)", self.ceiling_mw, self.target_mw));
            self.ceiling_mw = self.target_mw;
        }
        if self.interim_month >= self.queue_months {
            let to = (self.queue_months - 1.0).max(0.0);
            fixes.push(format!("interim_month {} -> {} (must precede firm service)", self.interim_month, to));
            self.interim_month = to;
        }
        if self.horizon < self.queue_months + 12.0 {
            let to = (self.queue_months + 12.0).min(spec("horizon").map_or(240.0, |s| s.max));
            fixes.push(format!("horizon {} -> {} (must cover 12 months of firm service)", self.horizon, to));
            self.horizon = to;
        }
        fixes
    }

    /// Effective ceiling and BESS capability after engineering derates.
    pub fn effective(&self) -> Effective {
        Effective {
            ceiling_mw: (self.ceiling_mw - self.safety_margin).max(0.0),
            bess_mw: self.bess_mw * self.bess_derate,
            bess_mwh: self.bess_mw * self.bess_h * self.soc_window,
        }
    }

    /// Number of Monte Carlo weather years.
    pub fn mc_years(&self) -> usize {
        self.mc_seeds.round().max(1.0) as usize
    }

    /// Check every range plus cross-parameter consistency.
    pub fn validate(&self) -> Result<(), Error> {
        for sp in SPECS {
            let v = self.get(sp.key).unwrap_or(f64::NAN);
            // `RangeInclusive::contains` is false for NaN, so NaN is rejected too.
            if !(sp.min..=sp.max).contains(&v) {
                return Err(Error::OutOfRange { key: sp.key.to_string(), value: v, min: sp.min, max: sp.max });
            }
        }
        if self.interim_mw > self.target_mw {
            return Err(Error::Inconsistent("interim_mw must not exceed target_mw".into()));
        }
        if self.queue_months >= self.horizon {
            return Err(Error::Inconsistent("horizon must be longer than queue_months".into()));
        }
        if self.interim_month >= self.queue_months {
            return Err(Error::Inconsistent("interim_month must be earlier than queue_months".into()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_valid_case_study() {
        let p = Params::default();
        p.validate().unwrap();
        assert_eq!(p.target_mw, 45.0);
        assert_eq!(p.carry, 145_000.0);
        let e = p.effective();
        assert_eq!((e.ceiling_mw, e.bess_mw, e.bess_mwh), (22.0, 23.0, 92.0));
    }

    #[test]
    fn perturbed_clamps_and_repairs() {
        let p = Params::default();
        assert_eq!(p.perturbed("bess_mw", 1e12).bess_mw, 1000.0);
        assert_eq!(p.perturbed("bess_mw", f64::NAN).bess_mw, 23.0);
        let q = p.perturbed("queue_months", 55.0);
        assert_eq!(q.horizon, 67.0);
        q.validate().unwrap();
        let mut r = Params::default();
        assert!(r.set_f64("wacc", 0.6).is_err());
        assert!(r.set_f64("wacc", f64::INFINITY).is_err());
        r.set_f64("wacc", 0.12).unwrap();
        assert_eq!(r.wacc, 0.12);
    }

    #[test]
    fn every_spec_has_a_slot_and_valid_default() {
        let mut p = Params::default();
        for sp in SPECS {
            assert!(sp.min <= sp.default && sp.default <= sp.max, "{}", sp.key);
            p.set(sp.key, &sp.default.to_string()).unwrap();
        }
    }

    #[test]
    fn rejects_bad_input() {
        let mut p = Params::default();
        assert!(matches!(p.set("util", "inf"), Err(Error::InvalidValue { .. })));
        assert!(matches!(p.set("util", "1.5"), Err(Error::OutOfRange { .. })));
        assert!(matches!(p.set("strategy", "diesel"), Err(Error::InvalidValue { .. })));
        assert!(matches!(p.set("seed", "-4"), Err(Error::InvalidValue { .. })));
        p.set("interim_mw", "40").unwrap();
        p.set("target_mw", "30").unwrap();
        assert!(matches!(p.validate(), Err(Error::Inconsistent(_))));
    }

    #[test]
    fn strategy_roundtrip() {
        let mut p = Params::default();
        p.set("strategy", "\"BESS-first\"").unwrap();
        assert_eq!(p.strategy, Strategy::BessFirst);
        assert_eq!(Strategy::parse(p.strategy.as_str()), Some(Strategy::BessFirst));
    }
}
