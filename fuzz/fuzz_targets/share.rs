// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Fuzz the proof-link decoder (`#s=<base64url>`): arbitrary input must never panic, and any token
//! that decodes must re-encode to a token that decodes to the same scenario.
//!
//! Run: `cargo +nightly fuzz run share -- -max_total_time=300`

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let _ = firmgap::portfolio::b64u_decode(text);
    if let Ok(sc) = firmgap::share_decode(text) {
        if let Ok((token, _)) = firmgap::share_encode(sc.rulepack, &sc.params, &sc.site, &sc.path) {
            let again = firmgap::share_decode(&token).expect("re-encoded token must decode");
            assert_eq!(again.params, sc.params);
            assert_eq!(again.rulepack, sc.rulepack);
        }
    }
});
