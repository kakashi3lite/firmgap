// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Deterministic Mulberry32 PRNG.
//!
//! Bit-for-bit identical to the JavaScript reference (`Math.imul`, `>>>`), so a
//! given seed produces the same synthetic weather year in the CLI and in the
//! browser simulator. Not cryptographic, and never used for anything security
//! relevant.

/// 32-bit Mulberry32 generator returning uniform `f64` in `[0, 1)`.
#[derive(Debug, Clone)]
pub struct Mulberry32 {
    state: u32,
}

impl Mulberry32 {
    /// Create a generator from a seed.
    pub fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    /// Next uniform draw in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x6D2B_79F5);
        let a = self.state;
        let mut t = (a ^ (a >> 15)).wrapping_mul(1 | a);
        t = t.wrapping_add((t ^ (t >> 7)).wrapping_mul(61 | t)) ^ t;
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_javascript_reference_sequence() {
        let mut r = Mulberry32::new(2026);
        let expected = [0.45540769933722913, 0.30849614599719644, 0.6611574492417276, 0.6184752183035016, 0.15228010178543627];
        for e in expected {
            assert_eq!(r.next_f64(), e);
        }
    }

    #[test]
    fn stays_in_unit_interval() {
        let mut r = Mulberry32::new(0);
        for _ in 0..100_000 {
            let x = r.next_f64();
            assert!((0.0..1.0).contains(&x));
        }
    }
}
