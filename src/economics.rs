// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Month-by-month energization phasing and horizon economics for three
//! pathways:
//!
//! | Pathway | Months `< interim_month` | Interim window | Bridge window | `>= queue_months` |
//! |---|---|---|---|---|
//! | **A** wait for firm | 0 MW | 0 MW | 0 MW | full, firm |
//! | **B** interim only | 0 MW | `interim_mw` | `interim_mw` | full, firm |
//! | **C** flex bridge | 0 MW | `interim_mw` | full, non-firm | full, firm (+BESS arbitrage) |
//!
//! The bridge window starts at `F = min(queue, interim + bess_lead)` only if
//! every curtailment event passes the first-minutes test; otherwise the
//! utility will not certify the expansion and C degenerates to B.
//!
//! Monthly energy and costs are the annual simulation / 12. Delay carry in a
//! month is `carry x (1 - energized_mw / target_mw)`.

use crate::params::Params;
use crate::sim::SimResult;

/// Accumulated horizon totals for one pathway.
#[derive(Debug, Clone, Default)]
pub struct Pathway {
    /// Compute energy served (MWh).
    pub mwh: f64,
    /// Grid energy cost ($).
    pub energy: f64,
    /// Annualized BESS CapEx charged over the horizon ($).
    pub bess: f64,
    /// Genset rental + fuel ($).
    pub gen: f64,
    /// Non-compliance penalties ($).
    pub penalty: f64,
    /// Queue-delay carry ($).
    pub carry: f64,
    /// Energized MW by month.
    pub series: Vec<f64>,
    /// Cumulative carry by month ($).
    pub carry_cum: Vec<f64>,
    /// Months spent in the non-firm bridge window.
    pub flex_months: u32,
    /// Sum of all cost components ($).
    pub total: f64,
    /// `total / mwh` ($/MWh), infinite if nothing is served.
    pub per_mwh: f64,
    /// Carry expressed in full-campus carry-months.
    pub carry_months_eq: f64,
}

impl Pathway {
    fn add(&mut self, sim: &SimResult) {
        self.mwh += sim.demand / 12.0;
        self.energy += sim.energy_cost / 12.0;
        self.gen += sim.gen_fuel / 12.0;
        self.penalty += sim.penalty_cost / 12.0;
    }

    /// Everything except grid energy, per MWh served.
    pub fn overhead_per_mwh(&self) -> f64 {
        (self.total - self.energy) / self.mwh
    }

    fn fixed_per_mwh(&self) -> f64 {
        (self.total - self.carry) / self.mwh
    }
}

/// Economics across the three pathways.
#[derive(Debug, Clone)]
pub struct Economics {
    /// A: wait for firm.
    pub a: Pathway,
    /// B: interim only.
    pub b: Pathway,
    /// C: flex bridge.
    pub c: Pathway,
    /// Month the full campus is usable under C.
    pub full_month: f64,
    /// Whether the bridge passed the first-minutes test on every event.
    pub certified: bool,
    /// BESS installed cost ($).
    pub capex: f64,
    /// Annualized BESS cost ($/yr).
    pub bess_ann: f64,
    /// Genset rental ($/yr, flex months only).
    pub gen_rent_ann: f64,
    /// BESS energy (MWh).
    pub bess_mwh: f64,
    /// Months of full capacity pulled forward vs. firm.
    pub pull_forward_months: f64,
    /// Monthly carry at which C is exactly 25% cheaper per MWh than A.
    pub breakeven_carry: f64,
    /// `1 - C.per_mwh / A.per_mwh`.
    pub reduction: f64,
}

/// Two-part BESS cost: power block ($/kW) + energy block, calibrated so that a
/// 4-hour system costs exactly `bess_cost` $/kWh.
pub fn bess_capex(p: &Params) -> f64 {
    let e_rate = (p.bess_cost - p.bess_power_cost / 4.0).max(0.0);
    p.bess_mw * 1000.0 * p.bess_power_cost + p.bess_mw * p.bess_h * 1000.0 * e_rate
}

/// Capital recovery factor.
pub fn crf(r: f64, n: f64) -> f64 {
    if r == 0.0 {
        1.0 / n
    } else {
        r * (1.0 + r).powf(n) / ((1.0 + r).powf(n) - 1.0)
    }
}

/// Compute pathway economics from the four annual simulations.
pub fn economics(p: &Params, interim: &SimResult, flex: &SimResult, firm: &SimResult, firm_bess: &SimResult) -> Economics {
    let t = p.target_mw;
    let i = p.interim_month;
    let q = p.queue_months;
    let bess_mwh = p.bess_mwh();
    let certified = flex.pass5_all;
    let f = if certified { q.min(i + p.bess_lead) } else { q };
    let capex = bess_capex(p);
    let bess_ann = capex * crf(p.wacc, p.life);
    let gen_rent_ann = p.gen_mw * 1000.0 * p.gen_rental;
    let (mut a, mut b, mut c) = (Pathway::default(), Pathway::default(), Pathway::default());

    let mut m: u32 = 0;
    while f64::from(m) < p.horizon {
        let mf = f64::from(m);
        // A
        let mw_a = if mf < q { 0.0 } else { t };
        if mw_a != 0.0 {
            a.add(firm);
        }
        // B
        let mw_b = if mf < i {
            0.0
        } else if mf < q {
            p.interim_mw
        } else {
            t
        };
        if (i..q).contains(&mf) {
            b.add(interim);
        } else if mf >= q {
            b.add(firm);
        }
        // C
        let mw_c = if mf < i {
            0.0
        } else if mf < f {
            c.add(interim);
            p.interim_mw
        } else if mf < q {
            c.add(flex);
            c.gen += gen_rent_ann / 12.0;
            c.flex_months += 1;
            t
        } else {
            c.add(if bess_mwh > 0.0 { firm_bess } else { firm });
            t
        };
        if certified && bess_mwh > 0.0 && mf >= f {
            c.bess += bess_ann / 12.0;
        }
        for (s, mw) in [(&mut a, mw_a), (&mut b, mw_b), (&mut c, mw_c)] {
            s.carry += p.carry * (1.0 - mw / t);
            s.series.push(mw);
            s.carry_cum.push(s.carry);
        }
        m += 1;
    }

    let carry_unit = if p.carry == 0.0 { 1.0 } else { p.carry };
    for s in [&mut a, &mut b, &mut c] {
        s.total = s.energy + s.bess + s.gen + s.penalty + s.carry;
        s.per_mwh = if s.mwh != 0.0 { s.total / s.mwh } else { f64::INFINITY };
        s.carry_months_eq = s.carry / carry_unit;
    }

    let k_a = a.carry_months_eq / a.mwh;
    let k_c = c.carry_months_eq / c.mwh;
    let denom = 0.75 * k_a - k_c;
    let breakeven_carry = if denom > 0.0 { (c.fixed_per_mwh() - 0.75 * a.fixed_per_mwh()) / denom } else { f64::INFINITY };
    let reduction = 1.0 - c.per_mwh / a.per_mwh;

    Economics {
        full_month: f,
        certified,
        capex,
        bess_ann,
        gen_rent_ann,
        bess_mwh,
        pull_forward_months: q - f,
        breakeven_carry,
        reduction,
        a,
        b,
        c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_hour_system_prices_at_reference_rate() {
        let p = Params { bess_mw: 10.0, bess_h: 4.0, ..Params::default() };
        assert!((bess_capex(&p) - 10.0 * 4.0 * 1000.0 * p.bess_cost).abs() < 1e-6);
    }

    #[test]
    fn crf_limits() {
        assert!((crf(0.0, 10.0) - 0.1).abs() < 1e-12);
        assert!((crf(0.09, 10.0) - 0.1558200899).abs() < 1e-9);
    }
}
