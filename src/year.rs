// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Synthetic 8,760-hour year: utilization profile plus clustered curtailment
//! signals.
//!
//! * **Utilization**: `util x diurnal(±6%, peak 15:00) x seasonal(±3%, peak
//!   ~19 Jul)`, clipped at 1.0. Values are rounded to `f32`, exactly as the
//!   reference model stores them in a `Float32Array`.
//! * **Signals**: `round(stress_hours / avg_event_h)` events with exponential
//!   durations (1–12 h), rebalanced so the durations sum to `stress_hours`, then
//!   placed without overlap (3 h guard band): 75% on summer afternoons
//!   (Jun–Sep, 12:00–16:00 starts) and 25% on winter evening peaks
//!   (Dec–Feb, 16:00–18:00 starts).

use crate::params::Params;
use crate::rng::Mulberry32;
use crate::HOURS_PER_YEAR as HRS;
use std::f64::consts::PI;

/// One curtailment signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    /// Chronological index.
    pub id: usize,
    /// First hour of the signal (hour of year).
    pub start: usize,
    /// Duration in hours.
    pub dur: usize,
}

/// A modelled year.
#[derive(Debug, Clone)]
pub struct Year {
    /// Hourly utilization (0–1).
    pub u: Vec<f64>,
    /// Event id active in each hour, or -1.
    pub ev: Vec<i32>,
    /// 1-based hour index within the active event, 0 outside events.
    pub k: Vec<u8>,
    /// Events sorted by start.
    pub events: Vec<Event>,
    /// Signal hours requested (`round(stress_hours)`).
    pub requested_hours: usize,
}

impl Year {
    /// Signal hours actually placed (may be below `requested_hours` if the
    /// seasonal windows are saturated).
    pub fn placed_hours(&self) -> usize {
        self.events.iter().map(|e| e.dur).sum()
    }
}

/// Build the year for the given parameters (uses `util`, `stress_hours`,
/// `avg_event_h`, `seed`).
pub fn build_year(p: &Params) -> Year {
    let mut u = vec![0.0f64; HRS];
    let mut ev = vec![-1i32; HRS];
    let mut k = vec![0u8; HRS];
    for t in 0..HRS {
        let hod = (t % 24) as f64;
        let doy = (t / 24) as f64;
        let d = 1.0 + 0.06 * (2.0 * PI * (hod - 15.0) / 24.0).cos();
        let s = 1.0 + 0.03 * (2.0 * PI * (doy - 200.0) / 365.0).cos();
        let v = (p.util * d * s).min(1.0);
        u[t] = f64::from(v as f32);
    }

    let mut rnd = Mulberry32::new(p.seed);
    let target = p.stress_hours.round().max(0.0) as usize;
    let mut n = if target == 0 { 0 } else { ((target as f64 / p.avg_event_h).round() as usize).max(1) };
    n = n.min(target);

    let mut durs: Vec<usize> = Vec::with_capacity(n);
    for _ in 0..n {
        let x = -(1.0 - rnd.next_f64() * 0.999).ln() * p.avg_event_h;
        durs.push((x.round() as usize).clamp(1, 12));
    }
    let mut sum: usize = durs.iter().sum();
    let mut guard: u32 = 0;
    // Mirrors `while (sum < target && guard++ < 20000)` exactly.
    while sum < target {
        let g = guard;
        guard += 1;
        if g >= 20_000 {
            break;
        }
        let i = (rnd.next_f64() * n as f64).floor() as usize;
        if durs[i] < 12 {
            durs[i] += 1;
            sum += 1;
        }
    }
    while sum > target {
        let g = guard;
        guard += 1;
        if g >= 40_000 {
            break;
        }
        let i = (rnd.next_f64() * n as f64).floor() as usize;
        if durs[i] > 1 {
            durs[i] -= 1;
            sum -= 1;
        }
    }

    let mut occ = vec![false; HRS];
    let mut placed: Vec<Event> = Vec::with_capacity(n);
    for &dur in &durs {
        for _try in 0..400 {
            let (day, hr) = if rnd.next_f64() < 0.75 {
                let day = 152 + (rnd.next_f64() * 122.0).floor() as usize;
                let hr = 12 + (rnd.next_f64() * 5.0).floor() as usize;
                (day, hr)
            } else {
                let w = (rnd.next_f64() * 90.0).floor() as usize;
                let day = if w < 31 { 334 + w } else { w - 31 };
                let hr = 16 + (rnd.next_f64() * 3.0).floor() as usize;
                (day, hr)
            };
            let t0 = day * 24 + hr;
            if t0 + dur >= HRS {
                continue;
            }
            let lo = t0.saturating_sub(3);
            let hi = (t0 + dur + 3).min(HRS);
            if occ[lo..hi].iter().any(|&o| o) {
                continue;
            }
            for o in &mut occ[t0..t0 + dur] {
                *o = true;
            }
            placed.push(Event { id: 0, start: t0, dur });
            break;
        }
    }
    placed.sort_by_key(|e| e.start);
    for (id, e) in placed.iter_mut().enumerate() {
        e.id = id;
        for j in 0..e.dur {
            ev[e.start + j] = id as i32;
            k[e.start + j] = (j + 1) as u8;
        }
    }
    Year { u, ev, k, events: placed, requested_hours: target }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_year_places_all_hours_without_overlap() {
        let y = build_year(&Params::default());
        assert_eq!(y.requested_hours, 180);
        assert_eq!(y.placed_hours(), 180);
        assert_eq!(y.events.len(), 51);
        for w in y.events.windows(2) {
            assert!(w[0].start + w[0].dur + 3 <= w[1].start, "events overlap or violate guard band");
        }
        let flagged = y.ev.iter().filter(|&&e| e >= 0).count();
        assert_eq!(flagged, 180);
    }

    #[test]
    fn utilization_mean_matches_input() {
        let y = build_year(&Params::default());
        let mean = y.u.iter().sum::<f64>() / HRS as f64;
        assert!((mean - 0.78).abs() < 1e-3, "mean {mean}");
    }

    #[test]
    fn zero_stress_hours_gives_no_events() {
        let p = Params { stress_hours: 0.0, ..Params::default() };
        assert!(build_year(&p).events.is_empty());
    }
}
