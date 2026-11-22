// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Fuzz the untrusted-input surface: scenario parsing, parameter setting and
//! validation must never panic, hang or accept a value outside its range.
//!
//! Run: `cargo +nightly fuzz run scenario -- -max_total_time=300`

#![no_main]

use firmgap::{params::SPECS, scenario, Params};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok(pairs) = scenario::parse(text) else { return };
    let mut p = Params::default();
    if scenario::apply(&mut p, &pairs).is_ok() && p.validate().is_ok() {
        for sp in SPECS {
            let v = p.get(sp.key).unwrap();
            assert!(v.is_finite() && v >= sp.min && v <= sp.max, "{} escaped its range: {v}", sp.key);
        }
    }
});
