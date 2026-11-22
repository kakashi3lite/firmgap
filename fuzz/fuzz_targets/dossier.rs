// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Fuzz the dossier verifier and canonical JSON round-trip: arbitrary bytes must never panic, and
//! any document that parses must re-serialize to a canonical form that parses to the same value.
//!
//! Run: `cargo +nightly fuzz run dossier -- -max_total_time=300`

#![no_main]

use firmgap::canon::{canonical, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let _ = firmgap::verify_text(text);
    if let Ok(v) = parse(text) {
        let c = canonical(&v);
        let again = parse(&c).expect("canonical output must parse");
        assert_eq!(canonical(&again), c, "canonical form must be a fixed point");
    }
});
