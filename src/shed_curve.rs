// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Utility-admissible shedding capability curve.
//!
//! Workload classes are curtailed by GPU power-capping. A class running at power cap `u` (fraction
//! of nominal power) runs at speed `u^beta`, so an event of duration `dt` hours loses
//! `(1 - u^beta) x dt` hours of work. To keep the SLA, the lost work must fit in the class slack:
//!
//! ```text
//! u_bar(dt) = min(1, max(u_min, max(0, 1 - slack / dt)^(1 / beta)))
//! ```
//!
//! `u_bar` is non-decreasing in `dt`, so the certified sheddable MW
//! `sum(nominal x (1 - u_bar))` is non-increasing in event duration (tested invariant).
//!
//! For a target curtailment `delta`, the minimum-value-loss allocation (KKT conditions) equalises
//! the marginal cost per MW shed across classes at a dual price `lambda`:
//! `u_k(lambda) = clip((lambda / (v_k dt beta_k))^(1 / (beta_k - 1)), u_bar_k, 1)`, solved by a
//! fixed 64-iteration bisection so the result is bit-reproducible. This follows the job-level
//! allocation idea of Mishra, Omidi and Almassalkhi (arXiv:2611.19073). The BESS CapEx and cycling
//! comparison follows Liu, Shin and Deka (arXiv:2605.16190) qualitatively: storage and compute
//! flexibility are complements, and sizing is driven by capital and cycling cost.

use crate::canon::{num, obj, s, Value};
use crate::compliance::Site;
use crate::params::Params;

/// Candidate event durations (hours): 15 min, 30 min, 1 h, 2 h, 4 h.
pub const SHED_DURATIONS_H: [f64; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

/// One workload class.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkloadClass {
    /// Class name.
    pub name: &'static str,
    /// Nominal power (MW).
    pub nominal_mw: f64,
    /// Power-cap floor (fraction of nominal).
    pub u_min: f64,
    /// Speed-power exponent.
    pub beta: f64,
    /// Value of throughput ($/MWh of compute).
    pub value_per_mwh: f64,
    /// SLA slack (h).
    pub sla_slack_hours: f64,
}

/// One point of the curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShedPoint {
    /// Event duration (minutes).
    pub duration_min: f64,
    /// Certified sheddable MW that keeps every SLA.
    pub certified_shed_mw: f64,
    /// Shed targeted for the shadow price (min(required, certified)).
    pub target_shed_mw: f64,
    /// Marginal value of throughput lost at the target ($/MWh).
    pub marginal_shadow_price_per_mwh: f64,
    /// BESS energy displaced by certified shedding over the event (MWh).
    pub bess_mwh_displaced: f64,
}

/// Shedding capability curve and the BESS it displaces.
#[derive(Debug, Clone, PartialEq)]
pub struct SheddingCapabilityCurve {
    /// Workload classes used.
    pub classes: Vec<WorkloadClass>,
    /// Points for 15 min / 30 min / 1 h / 2 h / 4 h.
    pub points: [ShedPoint; 5],
    /// Curtailment the site must deliver (MW).
    pub required_shed_mw: f64,
    /// BESS power needed without compute shedding (MW).
    pub bess_mw_without_shed: f64,
    /// BESS energy needed without compute shedding (4 h horizon, MWh).
    pub bess_mwh_without_shed: f64,
    /// BESS power needed with certified shedding (30 min binding horizon, MW).
    pub bess_mw_with_shed: f64,
    /// BESS energy needed with certified shedding (worst horizon up to 4 h, MWh).
    pub bess_mwh_with_shed: f64,
    /// CapEx without shedding ($).
    pub capex_without_shed_usd: f64,
    /// CapEx with shedding ($).
    pub capex_with_shed_usd: f64,
    /// BESS CapEx displaced by certified compute shedding ($).
    pub capex_avoided_usd: f64,
    /// BESS cycling cost avoided per year ($/yr).
    pub cycling_cost_avoided_usd_per_year: f64,
}

/// Default workload classes for a site: critical inference (not sheddable), batch and training.
pub fn shed_classes(st: &Site) -> Vec<WorkloadClass> {
    let p = st.gross_peak_mw;
    let f = st.flexible_share;
    let n_flex = p * f;
    let n_train = n_flex * st.training_share_of_flexible;
    let n_batch = n_flex - n_train;
    vec![
        WorkloadClass {
            name: "inference",
            nominal_mw: p * (1.0 - f),
            u_min: 1.0,
            beta: st.beta,
            value_per_mwh: st.critical_value_usd_per_mwh,
            sla_slack_hours: 0.0,
        },
        WorkloadClass {
            name: "batch",
            nominal_mw: n_batch,
            u_min: st.u_min,
            beta: st.beta,
            value_per_mwh: st.batch_value_usd_per_mwh,
            sla_slack_hours: st.sla_slack_hours,
        },
        WorkloadClass {
            name: "training",
            nominal_mw: n_train,
            u_min: st.u_min,
            beta: st.beta,
            value_per_mwh: st.training_value_usd_per_mwh,
            sla_slack_hours: st.sla_slack_hours * st.training_slack_multiplier,
        },
    ]
}

/// Minimum power cap that keeps the class SLA for an event of `dt` hours.
pub fn u_bar(c: &WorkloadClass, dt: f64) -> f64 {
    let r = (1.0 - c.sla_slack_hours / dt).max(0.0);
    c.u_min.max(r.powf(1.0 / c.beta)).min(1.0)
}

/// Certified sheddable MW for an event of `dt` hours.
pub fn certified_shed(classes: &[WorkloadClass], dt: f64) -> f64 {
    let mut total = 0.0;
    for c in classes {
        total += c.nominal_mw * (1.0 - u_bar(c, dt));
    }
    total
}

fn u_at(c: &WorkloadClass, dt: f64, lam: f64, ub: f64) -> f64 {
    let k = c.value_per_mwh * dt * c.beta;
    if k.is_nan() || k <= 0.0 {
        return ub;
    }
    if c.beta >= 1.0 {
        return if lam >= k { ub } else { 1.0 };
    }
    let u = (lam / k).powf(1.0 / (c.beta - 1.0));
    ub.max(u).min(1.0)
}

/// Minimum-value-loss allocation of `target` MW for an event of `dt` hours.
/// Returns `(lambda, shed_mw)` where `lambda` is $ per MW shed for the whole event.
pub fn allocate(classes: &[WorkloadClass], dt: f64, target: f64) -> (f64, f64) {
    let ubs: Vec<f64> = classes.iter().map(|c| u_bar(c, dt)).collect();
    let shed_at = |lam: f64| {
        let mut total = 0.0;
        for (c, ub) in classes.iter().zip(&ubs) {
            total += c.nominal_mw * (1.0 - u_at(c, dt, lam, *ub));
        }
        total
    };
    if target.is_nan() || target <= 0.0 {
        return (0.0, 0.0);
    }
    let mut hi = 0.0f64;
    for (c, &ub) in classes.iter().zip(&ubs) {
        if c.nominal_mw.is_nan() || c.nominal_mw <= 0.0 || ub >= 1.0 {
            continue;
        }
        let m = if ub > 0.0 { c.value_per_mwh * dt * c.beta * ub.powf(c.beta - 1.0) } else { 1e12 };
        if m > hi {
            hi = m;
        }
    }
    if hi.is_nan() || hi <= 0.0 {
        return (0.0, shed_at(0.0));
    }
    hi = hi.min(1e12);
    let mut lo = 0.0f64;
    for _ in 0..64 {
        let mid = (lo + hi) / 2.0;
        if shed_at(mid) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (hi, shed_at(hi))
}

/// Two-part BESS CapEx for explicit power and energy (delegates to
/// [`crate::economics::bess_capex_power_energy`], the same calibration as `bess_capex`).
pub fn capex_power_energy(p: &Params, mw: f64, mwh: f64) -> f64 {
    crate::economics::bess_capex_power_energy(p, mw, mwh)
}

/// Build the shedding capability curve for a resolved site.
pub fn shed_curve(st: &Site, p: &Params) -> SheddingCapabilityCurve {
    let classes = shed_classes(st);
    let r = st.required_shed_mw;
    let mut points = [ShedPoint {
        duration_min: 0.0,
        certified_shed_mw: 0.0,
        target_shed_mw: 0.0,
        marginal_shadow_price_per_mwh: 0.0,
        bess_mwh_displaced: 0.0,
    }; 5];
    for (i, &dt) in SHED_DURATIONS_H.iter().enumerate() {
        let cert = certified_shed(&classes, dt);
        let target = r.min(cert);
        let (lambda, _) = allocate(&classes, dt, target);
        points[i] = ShedPoint {
            duration_min: dt * 60.0,
            certified_shed_mw: cert,
            target_shed_mw: target,
            marginal_shadow_price_per_mwh: lambda / dt,
            bess_mwh_displaced: cert * dt,
        };
    }
    let p0 = r;
    let e0 = r * 4.0;
    let p1 = (r - points[1].certified_shed_mw).max(0.0);
    let mut e1 = 0.0f64;
    for (i, &dt) in SHED_DURATIONS_H.iter().enumerate() {
        let e = (r - points[i].certified_shed_mw).max(0.0) * dt;
        if e > e1 {
            e1 = e;
        }
    }
    let without = capex_power_energy(p, p0, e0);
    let with = capex_power_energy(p, p1, e1);
    SheddingCapabilityCurve {
        points,
        required_shed_mw: r,
        bess_mw_without_shed: p0,
        bess_mwh_without_shed: e0,
        bess_mw_with_shed: p1,
        bess_mwh_with_shed: e1,
        capex_without_shed_usd: without,
        capex_with_shed_usd: with,
        capex_avoided_usd: without - with,
        cycling_cost_avoided_usd_per_year: st.bess_degradation_usd_per_mwh * p.stress_hours * r.min(points[2].certified_shed_mw),
        classes,
    }
}

impl SheddingCapabilityCurve {
    /// Dossier representation (keys match the JavaScript reference).
    pub fn to_value(&self) -> Value {
        let classes = self
            .classes
            .iter()
            .map(|c| {
                obj(vec![
                    ("name", s(c.name)),
                    ("nominal_mw", num(c.nominal_mw)),
                    ("u_min", num(c.u_min)),
                    ("beta", num(c.beta)),
                    ("value_per_mwh", num(c.value_per_mwh)),
                    ("sla_slack_hours", num(c.sla_slack_hours)),
                ])
            })
            .collect();
        let points = self
            .points
            .iter()
            .map(|p| {
                obj(vec![
                    ("duration_min", num(p.duration_min)),
                    ("certified_shed_mw", num(p.certified_shed_mw)),
                    ("target_shed_mw", num(p.target_shed_mw)),
                    ("marginal_shadow_price_per_mwh", num(p.marginal_shadow_price_per_mwh)),
                    ("bess_mwh_displaced", num(p.bess_mwh_displaced)),
                ])
            })
            .collect();
        obj(vec![
            ("classes", Value::Arr(classes)),
            ("points", Value::Arr(points)),
            ("required_shed_mw", num(self.required_shed_mw)),
            ("bess_mw_without_shed", num(self.bess_mw_without_shed)),
            ("bess_mwh_without_shed", num(self.bess_mwh_without_shed)),
            ("bess_mw_with_shed", num(self.bess_mw_with_shed)),
            ("bess_mwh_with_shed", num(self.bess_mwh_with_shed)),
            ("capex_without_shed_usd", num(self.capex_without_shed_usd)),
            ("capex_with_shed_usd", num(self.capex_with_shed_usd)),
            ("capex_avoided_usd", num(self.capex_avoided_usd)),
            ("cycling_cost_avoided_usd_per_year", num(self.cycling_cost_avoided_usd_per_year)),
        ])
    }
}
