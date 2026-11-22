// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Investor cash-flow view (model v0.2): the flex bridge vs. interim-only (pathway B).
//!
//! Monthly incremental cash flow, discounted at the monthly equivalent of `wacc`:
//!
//! * month `interim_month`: −BESS CapEx (nameplate), committed when interim service starts;
//! * bridge months `[F, queue)`: gross margin before energy on the extra compute − extra energy −
//!   penalties − genset (fuel + rental) − deferral fees − SLA penalties;
//! * after firm service: keep the BESS for stress-hour arbitrage until end of life, or resell it at
//!   `salvage x CapEx`, whichever is worth more;
//! * downside path: the BESS is built but the witness test fails, so there are no bridge months.
//!
//! Queue carry is deliberately **excluded**. It contains delayed revenue, which the margin term
//! already counts, and tied-up capital is sunk in both pathways (decision D9 in CONTEXT_MAP).

use crate::economics::Economics;
use crate::params::Params;
use crate::sim::SimResult;
use std::fmt;

/// Monthly cash-flow components during the bridge window ($/month).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CashParts {
    /// Gross margin before energy on incremental compute.
    pub margin: f64,
    /// Incremental grid energy (negative).
    pub energy: f64,
    /// Incremental non-compliance penalties (negative).
    pub penalty: f64,
    /// Genset fuel + rental (negative).
    pub genset: f64,
    /// Deferral fees to workload owners (negative).
    pub deferral: f64,
    /// SLA penalties on late work (negative).
    pub sla: f64,
}

impl CashParts {
    /// Net monthly bridge cash flow.
    pub fn total(&self) -> f64 {
        self.margin + self.energy + self.penalty + self.genset + self.deferral + self.sla
    }
}

/// What happens to the BESS after firm service.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Disposition {
    /// No BESS.
    None,
    /// Kept for post-firm stress-hour arbitrage until end of life.
    Retain,
    /// Resold at this percentage of CapEx.
    Resell(u32),
    /// Neither arbitrage nor resale has value.
    NoResidual,
}

impl fmt::Display for Disposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Disposition::None => write!(f, "none"),
            Disposition::Retain => write!(f, "retain for post-firm arbitrage"),
            Disposition::Resell(p) => write!(f, "resell at {p}% of CapEx"),
            Disposition::NoResidual => write!(f, "no residual value"),
        }
    }
}

/// Investor metrics.
#[derive(Debug, Clone)]
pub struct Investor {
    /// NPV of the certified bridge vs. interim-only ($).
    pub npv: f64,
    /// NPV if the BESS is built but the witness test fails ($).
    pub npv_fail: f64,
    /// BESS disposition on the success path.
    pub disposition: Disposition,
    /// BESS disposition on the failure path.
    pub fail_disposition: Disposition,
    /// PV of the post-firm residual value on the success path ($).
    pub post_pv: f64,
    /// Monthly cash flows, success path ($, index = month).
    pub cf: Vec<f64>,
    /// Monthly cash flows, failure path ($).
    pub cf_fail: Vec<f64>,
    /// Monthly bridge components ($/month).
    pub parts: CashParts,
    /// Net monthly bridge cash flow ($/month).
    pub bridge_monthly: f64,
    /// Sum of discount factors over the bridge months (multiply a monthly flow to get its PV).
    pub pv_factor: f64,
    /// PV of BESS CapEx at commitment ($).
    pub capex_pv: f64,
    /// Post-firm arbitrage cash flow ($/month).
    pub arb_monthly: f64,
    /// Undiscounted payback from commitment (months), if the bridge is certified and cash-positive.
    pub payback: Option<u32>,
    /// Annualised IRR of the success path (`Some(inf)` if NPV stays positive at every tested rate).
    pub irr: Option<f64>,
    /// Bridge months (0 if not certified).
    pub bridge_months: f64,
    /// Unlocked MW-months (`(target − interim) x bridge_months`).
    pub mw_months: f64,
    /// PV of all non-energy bridge costs net of post-firm value per unlocked MW-month ($).
    pub cost_per_mw_month: Option<f64>,
    /// Gross margin before energy at which NPV = 0 ($/MWh).
    pub break_even_margin: Option<f64>,
    /// Deferral fee at which NPV = 0 ($/MWh).
    pub break_even_defer_cost: Option<f64>,
    /// SLA penalty at which NPV = 0 ($/MWh).
    pub break_even_sla_penalty: Option<f64>,
    /// PV of software fees under the pricing hypothesis ($).
    pub sw_pv: f64,
    /// Software fees as a share of total value created (`sw_pv / (npv + sw_pv)`).
    pub sw_share: Option<f64>,
    /// Days of bridge cash flow needed to repay setup + one month of fees.
    pub sw_payback_days: Option<f64>,
    /// BESS CapEx exposed on the downside path ($).
    pub capex_at_risk: f64,
}

struct Flows {
    cf: Vec<f64>,
    npv: f64,
    disposition: Disposition,
    post_pv: f64,
}

fn round_usize(v: f64) -> usize {
    if v.is_finite() && v > 0.0 {
        v.round() as usize
    } else {
        0
    }
}

/// Compute the investor view from the four annual simulations and the pathway economics.
pub fn investor(p: &Params, interim: &SimResult, flex: &SimResult, firm: &SimResult, firm_bess: &SimResult, econ: &Economics) -> Investor {
    let rm = (1.0 + p.wacc).powf(1.0 / 12.0) - 1.0;
    let df = |m: usize| (1.0 + rm).powf(-(m as f64));
    let i = round_usize(p.interim_month);
    let q = round_usize(p.queue_months);
    let life_m = round_usize(p.life * 12.0).max(12);
    let has_bess = p.bess_mw * p.bess_h > 0.0;
    let capex = if has_bess { econ.capex } else { 0.0 };
    let certified = econ.certified;
    let f = if certified { round_usize(econ.full_month) } else { q };
    let d_mwh = (flex.demand - interim.demand) / 12.0;
    let parts = CashParts {
        margin: p.margin_pre_energy * d_mwh,
        energy: -(flex.energy_cost - interim.energy_cost) / 12.0,
        penalty: -(flex.penalty_cost - interim.penalty_cost) / 12.0,
        genset: -(flex.gen_fuel + econ.gen_rent_ann) / 12.0,
        deferral: -(p.defer_cost * flex.thr_e) / 12.0,
        sla: -(p.sla_penalty * flex.breach) / 12.0,
    };
    let bridge_monthly = parts.total();
    let arb_monthly = if has_bess { ((firm.energy_cost - firm_bess.energy_cost) / 12.0).max(0.0) } else { 0.0 };
    let online = i + round_usize(p.bess_lead);
    let end_life = online + life_m;
    let horizon_m = if p.horizon.is_finite() && p.horizon > 0.0 { p.horizon.ceil() as usize } else { 0 };

    let flows = |success: bool| -> Flows {
        let n = horizon_m.max(end_life + 1).max(q + 1) + 1;
        let mut cf = vec![0.0f64; n];
        if capex > 0.0 {
            cf[i] -= capex;
        }
        if success {
            for m in f..q {
                cf[m] += bridge_monthly;
            }
        }
        let mut disposition = Disposition::None;
        let mut post = 0.0;
        if has_bess {
            let sell_at = if success { q } else { online.min(q) };
            let from = q.max(online);
            let mut retain_pv = 0.0;
            for m in from..end_life {
                retain_pv += arb_monthly * df(m);
            }
            let sell_pv = p.salvage * capex * df(sell_at);
            if retain_pv >= sell_pv && retain_pv > 0.0 {
                disposition = Disposition::Retain;
                for m in from..end_life {
                    cf[m] += arb_monthly;
                }
                post = retain_pv;
            } else if sell_pv > 0.0 {
                disposition = Disposition::Resell((p.salvage * 100.0).round() as u32);
                cf[sell_at] += p.salvage * capex;
                post = sell_pv;
            } else {
                disposition = Disposition::NoResidual;
            }
        }
        let mut npv = 0.0;
        for (m, c) in cf.iter().enumerate() {
            npv += c * df(m);
        }
        Flows { cf, npv, disposition, post_pv: post }
    };
    let ok = flows(certified);
    let fail = flows(false);

    let mut payback = None;
    if certified && bridge_monthly > 0.0 {
        if capex <= 0.0 {
            payback = Some(0);
        } else {
            let mut cum = 0.0;
            for m in i..ok.cf.len() {
                cum += ok.cf[m];
                if cum >= 0.0 {
                    payback = Some((m - i) as u32);
                    break;
                }
            }
        }
    }

    let npv_at = |r: f64| -> f64 {
        let mut a = 0.0;
        for (m, c) in ok.cf.iter().enumerate() {
            a += c / (1.0 + r).powf(m as f64);
        }
        a
    };
    let mut irr = None;
    if capex > 0.0 && certified && ok.cf.iter().any(|&c| c > 0.0) {
        let (mut lo, mut hi) = (-0.99f64, 5.0f64);
        if npv_at(lo) * npv_at(hi) < 0.0 {
            for _ in 0..200 {
                let mid = (lo + hi) / 2.0;
                if npv_at(lo) * npv_at(mid) <= 0.0 {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            irr = Some((1.0 + (lo + hi) / 2.0).powf(12.0) - 1.0);
        } else if npv_at(hi) > 0.0 {
            irr = Some(f64::INFINITY);
        }
    }

    let (mut b_margin, mut b_defer, mut b_sla, mut pv_energy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    if certified {
        for m in f..q {
            b_margin += d_mwh * df(m);
            b_defer += flex.thr_e / 12.0 * df(m);
            b_sla += flex.breach / 12.0 * df(m);
            pv_energy += parts.energy * df(m);
        }
    }
    let bridge_months = if certified { q.saturating_sub(f) as f64 } else { 0.0 };
    let d_mw = (p.target_mw - p.interim_mw).max(0.0);
    let mw_months = d_mw * bridge_months;
    let pv_margin = p.margin_pre_energy * b_margin;
    let mut sw_pv = 0.0;
    if certified && bridge_months > 0.0 {
        sw_pv += p.sw_setup * df(i);
        for m in f..q {
            sw_pv += p.sw_price_mw_month * d_mw * df(m);
        }
    }

    Investor {
        npv: ok.npv,
        npv_fail: fail.npv,
        disposition: ok.disposition,
        fail_disposition: fail.disposition,
        post_pv: ok.post_pv,
        pv_factor: if b_margin > 0.0 { b_margin / d_mwh } else { 0.0 },
        capex_pv: capex * df(i),
        arb_monthly,
        payback,
        irr,
        bridge_months,
        mw_months,
        cost_per_mw_month: if mw_months > 0.0 { Some((pv_margin + pv_energy - ok.npv) / mw_months) } else { None },
        break_even_margin: if b_margin > 0.0 { Some(p.margin_pre_energy - ok.npv / b_margin) } else { None },
        break_even_defer_cost: if b_defer > 0.0 { Some(p.defer_cost + ok.npv / b_defer) } else { None },
        break_even_sla_penalty: if b_sla > 0.0 { Some(p.sla_penalty + ok.npv / b_sla) } else { None },
        sw_share: if ok.npv + sw_pv > 0.0 { Some(sw_pv / (ok.npv + sw_pv)) } else { None },
        sw_payback_days: if bridge_monthly > 0.0 {
            Some((p.sw_setup + p.sw_price_mw_month * d_mw) / (bridge_monthly / 30.4))
        } else {
            None
        },
        sw_pv,
        capex_at_risk: capex,
        parts,
        bridge_monthly,
        cf: ok.cf,
        cf_fail: fail.cf,
    }
}
