// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! World scenarios (what the grid, capital markets and workload owners do) and bridge designs (what
//! the developer builds). A world overlays fixed stress values on the current inputs; a design sets
//! the five design levers. Identical to `WORLDS` / `DESIGNS` in the web workbench.

use crate::error::Error;
use crate::params::{Params, Strategy};

/// A world scenario.
#[derive(Debug, Clone, Copy)]
pub struct World {
    /// CLI id.
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// What the world assumes.
    pub description: &'static str,
    /// Overrides applied after resetting every world key to the case value (applied in order).
    pub patch: &'static [(&'static str, f64)],
    /// Optional cap on the throttle hold limit (contractual).
    pub hold_cap: Option<f64>,
}

/// A bridge design.
#[derive(Debug, Clone, Copy)]
pub struct Design {
    /// CLI id.
    pub id: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Description.
    pub description: &'static str,
    /// BESS power (MW).
    pub bess_mw: f64,
    /// BESS duration (h).
    pub bess_h: f64,
    /// Throttle hold (h).
    pub max_throttle_h: f64,
    /// Genset (MW).
    pub gen_mw: f64,
    /// Dispatch priority.
    pub strategy: Strategy,
}

const D: [(&str, f64); 4] = [("safety_margin", 0.5), ("soc_window", 0.9), ("bess_derate", 0.95), ("defer_cost", 25.0)];

/// All world scenarios.
pub const WORLDS: &[World] = &[
    World {
        id: "stated",
        label: "Case as stated",
        description: "Exact case-study inputs with no engineering derates (reproduces model v0.1; optimistic).",
        patch: &[],
        hold_cap: None,
    },
    World {
        id: "base",
        label: "Base Case",
        description: "Case + 0.5 MW control deadband, 90% usable SoC, 95% BESS availability, $25/MWh deferral fee.",
        patch: &[D[0], D[1], D[2], D[3]],
        hold_cap: None,
    },
    World {
        id: "harsh",
        label: "Harsh Grid Stress",
        description: "Base + 360 signal h/yr, 5 h mean events, $600/MWh stress LMP, $900/MWh penalty.",
        patch: &[D[0], D[1], D[2], D[3], ("stress_hours", 360.0), ("avg_event_h", 5.0), ("stress_price", 600.0), ("penalty", 900.0)],
        hold_cap: None,
    },
    World {
        id: "fasttrack",
        label: "Queue Compression / Fast-Track",
        description: "Base + firm service at month 18 instead of 38; the bridge asset strands early.",
        patch: &[D[0], D[1], D[2], D[3], ("queue_months", 18.0)],
        hold_cap: None,
    },
    World {
        id: "crunch",
        label: "Capital Crunch",
        description: "Base + BESS $400/kWh, WACC 14%, 9-month delivery & witness test, $240/MWh margin, no resale.",
        patch: &[
            D[0],
            D[1],
            D[2],
            D[3],
            ("bess_cost", 400.0),
            ("wacc", 0.14),
            ("bess_lead", 9.0),
            ("margin_pre_energy", 240.0),
            ("salvage", 0.0),
        ],
        hold_cap: None,
    },
    World {
        id: "tariff",
        label: "Tariff Tightening",
        description: "Base + signal ceiling cut to 18 MW and a 1 MW required control deadband.",
        patch: &[D[0], D[1], D[2], D[3], ("ceiling_mw", 18.0), ("safety_margin", 1.0)],
        hold_cap: None,
    },
    World {
        id: "workload",
        label: "Workload Reality Check",
        description: "Base + holds capped at 4 h, 25% deferrable, 5% instant power-capping, $100/MWh deferral fee.",
        patch: &[D[0], D[1], D[2], D[3], ("defer_share", 0.25), ("fast_cap", 0.05), ("defer_cost", 100.0)],
        hold_cap: Some(4.0),
    },
];

/// All bridge designs.
pub const DESIGNS: &[Design] = &[
    Design {
        id: "robust",
        label: "Robust hybrid 18.5 MW / 2 h",
        description: "18.5 MW / 37 MWh BESS + 12 h hold; compliant in 30/30 weather years under Base Case derates.",
        bess_mw: 18.5,
        bess_h: 2.0,
        max_throttle_h: 12.0,
        gen_mw: 0.0,
        strategy: Strategy::ThrottleFirst,
    },
    Design {
        id: "edge",
        label: "v0.1 optimum 13.5 MW / 2 h",
        description: "Optimal only with zero derates; fails the first-minutes test once realistic derates apply.",
        bess_mw: 13.5,
        bess_h: 2.0,
        max_throttle_h: 12.0,
        gen_mw: 0.0,
        strategy: Strategy::ThrottleFirst,
    },
    Design {
        id: "eng",
        label: "Engineering proposal 23 MW / 4 h",
        description: "BESS sized to the full increment with a 4 h hold; long events breach the ceiling.",
        bess_mw: 23.0,
        bess_h: 4.0,
        max_throttle_h: 4.0,
        gen_mw: 0.0,
        strategy: Strategy::ThrottleFirst,
    },
    Design {
        id: "sw",
        label: "Software-only (no BESS)",
        description: "Deferral and GPU power-capping only; cannot cover the first minutes.",
        bess_mw: 0.0,
        bess_h: 2.0,
        max_throttle_h: 12.0,
        gen_mw: 0.0,
        strategy: Strategy::ThrottleFirst,
    },
    Design {
        id: "diesel",
        label: "Diesel band-aid 17 MW",
        description: "17 MW rental gensets under a 50 h/yr non-emergency permit, no BESS.",
        bess_mw: 0.0,
        bess_h: 2.0,
        max_throttle_h: 4.0,
        gen_mw: 17.0,
        strategy: Strategy::ThrottleFirst,
    },
];

/// Every key any world touches, plus the hold limit (which a world may only cap).
pub fn world_keys() -> Vec<&'static str> {
    let mut keys: Vec<&'static str> = Vec::new();
    for w in WORLDS {
        for &(k, _) in w.patch {
            if !keys.contains(&k) {
                keys.push(k);
            }
        }
    }
    keys
}

/// Find a world.
pub fn find_world(id: &str) -> Option<&'static World> {
    WORLDS.iter().find(|w| w.id == id)
}

/// Find a design.
pub fn find_design(id: &str) -> Option<&'static Design> {
    DESIGNS.iter().find(|d| d.id == id)
}

/// Apply a world: reset every world key to the case value, apply the patch, cap the hold, repair.
pub fn world_params(current: &Params, w: &World) -> Result<Params, Error> {
    let case = Params::default();
    let mut q = current.clone();
    for k in world_keys() {
        q.set_f64(k, case.get(k).unwrap_or(0.0))?;
    }
    for &(k, v) in w.patch {
        q.set_f64(k, v)?;
    }
    if let Some(cap) = w.hold_cap {
        q.max_throttle_h = q.max_throttle_h.min(cap);
    }
    q.repair();
    q.validate()?;
    Ok(q)
}

/// Apply a design's five levers.
pub fn design_params(current: &Params, d: &Design) -> Result<Params, Error> {
    let mut q = current.clone();
    q.set_f64("bess_mw", d.bess_mw)?;
    q.set_f64("bess_h", d.bess_h)?;
    q.set_f64("max_throttle_h", d.max_throttle_h)?;
    q.set_f64("gen_mw", d.gen_mw)?;
    q.strategy = d.strategy;
    q.repair();
    q.validate()?;
    Ok(q)
}

/// Apply a world by id.
pub fn apply_world(p: &Params, id: &str) -> Result<Params, Error> {
    let w =
        find_world(id).ok_or_else(|| Error::Usage(format!("unknown world '{}' (run `firmgap worlds`)", crate::scenario::sanitize(id))))?;
    world_params(p, w)
}

/// Apply a design by id.
pub fn apply_design(p: &Params, id: &str) -> Result<Params, Error> {
    let d = find_design(id)
        .ok_or_else(|| Error::Usage(format!("unknown design '{}' (run `firmgap worlds`)", crate::scenario::sanitize(id))))?;
    design_params(p, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_world_and_design_is_valid() {
        let p = Params::default();
        for w in WORLDS {
            for d in DESIGNS {
                let q = world_params(&design_params(&p, d).unwrap(), w).unwrap();
                q.validate().unwrap();
            }
        }
    }

    #[test]
    fn worlds_reset_previous_world() {
        let harsh = apply_world(&Params::default(), "harsh").unwrap();
        assert_eq!(harsh.stress_hours, 360.0);
        let base = apply_world(&harsh, "base").unwrap();
        assert_eq!(base.stress_hours, 180.0);
        assert_eq!(base.safety_margin, 0.5);
        let tariff = apply_world(&base, "tariff").unwrap();
        assert_eq!(tariff.safety_margin, 1.0);
        let wl = apply_world(&apply_design(&Params::default(), "robust").unwrap(), "workload").unwrap();
        assert_eq!(wl.max_throttle_h, 4.0);
    }
}
