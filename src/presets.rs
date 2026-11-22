// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Named scenarios that reproduce the strategic diagnosis. Each preset is a
//! patch applied on top of [`Params::default`] (the case study) through the
//! validated setter, so presets cannot bypass input checks.

use crate::error::Error;
use crate::params::Params;

/// A named parameter patch.
#[derive(Debug, Clone, Copy)]
pub struct Preset {
    /// CLI name.
    pub name: &'static str,
    /// One-line description.
    pub description: &'static str,
    /// `(key, value)` overrides applied to the case defaults.
    pub patch: &'static [(&'static str, &'static str)],
}

/// All presets.
pub const PRESETS: &[Preset] = &[
    Preset { name: "case", description: "Engineering proposal from the case: 23 MW / 4 h BESS, 4 h throttle hold.", patch: &[] },
    Preset {
        name: "right-sized",
        description: "Optimiser result: 13.5 MW / 2 h BESS + throttle-through (12 h hold).",
        patch: &[("bess_mw", "13.5"), ("bess_h", "2"), ("max_throttle_h", "12")],
    },
    Preset {
        name: "software-only",
        description: "No BESS: batch deferral and GPU power-capping only (fails the first-minutes test).",
        patch: &[("bess_mw", "0"), ("max_throttle_h", "12")],
    },
    Preset {
        name: "diesel-bandaid",
        description: "17 MW rental gensets under a 50 h/yr non-emergency permit, no BESS.",
        patch: &[("bess_mw", "0"), ("gen_mw", "17"), ("gen_cap_h", "50")],
    },
    Preset {
        name: "bess-first",
        description: "Right-sized hardware dispatched battery-first instead of throttle-first.",
        patch: &[("bess_mw", "13.5"), ("bess_h", "2"), ("max_throttle_h", "12"), ("strategy", "bess")],
    },
    Preset {
        name: "deep-curtailment",
        description: "Right-sized hardware under stress: 15 MW ceiling and 260 signal hours/yr.",
        patch: &[("bess_mw", "13.5"), ("bess_h", "2"), ("max_throttle_h", "12"), ("ceiling_mw", "15"), ("stress_hours", "260")],
    },
    Preset {
        name: "market-carry",
        description: "Right-sized bridge with carry at ~8% cost of capital on $68M stranded CapEx.",
        patch: &[("bess_mw", "13.5"), ("bess_h", "2"), ("max_throttle_h", "12"), ("carry", "453000")],
    },
];

/// Find a preset by name.
pub fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// Reset `p` to the case defaults and apply the named preset.
pub fn apply(p: &mut Params, name: &str) -> Result<(), Error> {
    let preset =
        find(name).ok_or_else(|| Error::Usage(format!("unknown preset '{}' (run `firmgap presets`)", crate::scenario::sanitize(name))))?;
    *p = Params::default();
    for (k, v) in preset.patch {
        p.set(k, v)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_presets_apply_and_validate() {
        for pr in PRESETS {
            let mut p = Params::default();
            apply(&mut p, pr.name).unwrap();
            p.validate().unwrap();
        }
    }
}
