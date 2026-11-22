# Investor diligence audit (firmgap v0.2)

This audit stress-tests the v0.1 diagnosis and MVP from an infrastructure-IC point of view. Every number
below is reproducible in `web/index.html` (world presets, design presets and the stress suite).

Unless a line says otherwise, figures use the **Base Case**: the case-study inputs plus realistic
engineering derates. Those derates are:
* a 0.5 MW metering / control deadband;
* a 90% usable SoC window;
* 95% BESS power availability;
* $25/MWh deferral compensation paid to workload owners.

The gross margin is **$320/MWh before energy**. That's an *assumption*, so the break-even is reported
next to it everywhere.

## 0. Corrections to v0.1 (stated first, on purpose)

| # | v0.1 claim | Finding | Effect |
|---|---|---|---|
| C1 | "Right-sized 13.5 MW / 2 h BESS is the optimum" | Optimal only with **zero** derates. With a 0.5 MW deadband, 90% SoC and 95% availability it **fails the first-minutes test in 30 of 30 weather years** | Recommendation changes to **18.5 MW / 2 h ($12.9M)**, compliant in 30/30 years |
| C2 | "Net value $87.7M at $250/MWh margin" | **Double count**: the case's carry includes *delayed revenue*, which the margin term already counts | v0.2 NPV excludes carry and counts revenue once |
| C3 | Cost per MW-month $6.2k | **Horizon truncation**: the 60-month horizon charged only 50 of 120 months of annualized BESS cost | Full-life PV: **$17.4k per MW-month** (net of post-firm value) |
| C4 | Single synthetic weather year | The optimum was tuned to one seed | v0.2 adds a weather-year Monte Carlo and a design × world matrix |
| C5 | Rust port "complete" | It has never been compiled | Model 0.2 is now ported with 18 parity tests; compile with `scripts/verify.sh` |

## 1. Value creation and willingness to pay

**Value to the campus owner (PV vs. interim-only):**

| Flow | PV | Who receives it |
|---|---|---|
| Gross margin on 157 GWh/yr of unlocked compute (28 months) | **+$99.3M** | campus owner |
| Incremental grid energy | −$21.4M | utility / ISO |
| BESS CapEx (committed at month 4) | −$12.5M | BESS OEM / EPC |
| Post-firm arbitrage over BESS life | +$1.4M | campus owner |
| Deferral fees at $25/MWh | −$0.1M | workload owners |
| **Owner NPV before software** | **$66.7M** | |
| Software at $250k setup + $1,500/MW-month | −$1.06M (**1.6%** of value) | vendor |

Returns on the robust design:
* **Payback** is 9 months from commitment, and the IRR is above 100%.
* The **downside** is −$11.1M if the witness test fails: the BESS is built, no bridge happens, and the
  battery is kept for post-firm arbitrage.

**Pricing power.** The value comes from capacity unlocked, not from energy saved. Price per *unlocked
MW-month* so the fee scales with the customer's gain:
* $1,500/MW-month takes 1.6% of value, and the customer recovers the fee in 2.6 days of bridge cash flow.
* Taking **10%** of value means about **$12.0k/MW-month**. That's the theoretical ceiling.
* Realistic pricing is set by the alternatives: in-house scheduler teams at hyperscalers, BESS EMS
  bundles, consultants, and venture-funded orchestration vendors. Emerald AI raised a $150M Series A
  at a $1.05B valuation in Aug 2026, which validates the category and confirms the competition.
* A defensible opening price is $2–5k per unlocked MW-month plus a sizing / interconnection-support
  fee, to be tested with 3–5 design-partner LOIs.

**The software policy is the asset.** The engineering proposal's *hardware* (23 MW / 4 h) is compliant
in only 17% of weather years with a 4-hour hold (case as stated; 0% with Base Case derates). With a 12-hour hold policy, the same hardware is
compliant in **100%** of 24 years. Right-sizing then saves a further $13.4M of CapEx (18.5 MW / 2 h).

**Where value leaks:**
* **Utility.** It gets flexibility for free and can tighten the ceiling or raise the signal hours over
  time. The contract needs caps on hours and duration and grandfathering.
* **BESS OEM / EPC.** It captures more value than the software vendor, and right-sizing *shrinks* OEM
  revenue. Selling through OEM channels is therefore misaligned.
* **Workload owners.** Value leaks to them only if they price deferral rights. At the break-even fee of
  **$14.8k per MWh deferred** this leak is not a threat, but the *right* to defer is.

## 2. Adversarial sensitivity and break-even thresholds

Each threshold below is the value at which NPV reaches zero or compliance is lost, holding everything
else at the robust design.

| Variable | Base | Break-even / cliff | Headroom |
|---|---|---|---|
| Gross margin before energy | $320/MWh | **$105/MWh** (≈ $36 after energy) | 3.0× |
| BESS installed cost | $285/kWh | $2,139/kWh | 7.5× |
| Firm service arrival (queue compression) | month 38 | **month 14** | 24 months earlier |
| BESS delivery + witness test | 6 months | 30 months | +24 months (≈ −$3.0M NPV per month of delay) |
| WACC | 9% | > 300% | n/a |
| Deferral fee | $25/MWh | $14,837/MWh | large |
| Signal hours | 180 h/yr | still 100% compliant at 600 h/yr (6 years per level) | large |
| **Signal ceiling** | 22 MW | **22 MW** | **zero** |
| **Deferrable share** | 35% | **35%** | **zero** |
| **Throttle hold** | 12 h | **12 h** | **zero** |
| **BESS power** | 18.5 MW | **18.5 MW** | **zero** |
| **Control deadband** | 0.5 MW | **0.5 MW** | **zero** |

What this shows:
* **Economically the bridge is very robust; physically it's knife-edge.** A least-cost compliant
  design has zero headroom on every physical lever by construction.
* An IC should approve a design **with explicit margin**. Example: compliant with the ceiling 2 MW
  lower *and* deferrable load 5 points lower gives **22 MW / 4 h, $25.1M CapEx, NPV $56.0M**.

The cost of each concession (re-optimized battery for that concession alone):

| Concession | Battery needed | CapEx |
|---|---|---|
| 6-hour hold | 30.5 MW / 4 h | $34.8M |
| 4-hour hold | none ≤ 33.5 MW / 4 h | n/a |
| Deferrable share 30% | 15.5 MW / 4 h | $17.7M |
| Deferrable share 25% | 21.5 MW / 4 h | $24.5M |
| Ceiling 20 MW | 16 MW / 4 h | $18.2M |
| Ceiling 18 MW | 22.5 MW / 4 h | $25.7M |

The 3–4 variables that break the thesis, in order:
1. **Workload hold rights** (contractual): at a 4 h cap no battery up to 33.5 MW / 4 h works.
2. **Ceiling tightening** by the utility.
3. **Queue compression** (firm arrives before month 14).
4. **BESS delivery slip** (about $3.0M of NPV per month).

The NPV of each design under each world scenario ($M; ✓ compliant · ~ certified but breaching ·
✕ not certifiable):

| Design | Case as stated | Base | Harsh grid | Fast-track (M18) | Capital crunch | Tariff (18 MW) | Workload reality |
|---|---|---|---|---|---|---|---|
| Robust 18.5 MW / 2 h | 66.9 ✓ | 66.6 ✓ | 69.0 ✓ | 13.2 ✓ | 27.3 ✓ | 66.5 ~ | 65.5 ~ |
| v0.1 optimum 13.5 / 2 h | 69.8 ✓ | −8.1 ✕ | −5.5 ✕ | −7.8 ✕ | −11.2 ✕ | −8.1 ✕ | −8.1 ✕ |
| Engineering 23 / 4 h (4 h hold) | 55.0 ~ | 54.7 ~ | 59.9 ~ | 1.7 ~ | 9.2 ~ | 54.5 ~ | 54.4 ~ |
| Software-only / diesel | ✕ | ✕ | ✕ | ✕ | ✕ | ✕ | ✕ |

## 3. Moat, lock-in and services drag

* **The simulator is not a moat.** It's open source (AGPL) and the math can be reproduced. It is a
  credibility and lead-generation asset with utilities and IC teams.
* **Defensible layers:**
  1. **The compliance system of record.** Signed telemetry, event logs and witness-test evidence that
     the utility accepts in the interim agreement. That is the switching cost.
  2. **Workload-elasticity data** across sites: deferrable share and hold behaviour by workload class.
     It improves every future sizing.
  3. **A library of productized connectors**: Slurm / Kubernetes / Ray schedulers, BESS EMS over
     Modbus / DNP3 / IEC 61850, OpenADR 3 utility signals, revenue-meter feeds.
  4. **Standards position** in the new flexible large-load transmission services.
* **Services-drag risk is high at first.** Each site needs tariff mapping, scheduler integration, EMS
  integration and witness-test support. To exceed 75% gross margin, per-site delivery labour must stay
  under about 25% of first-year revenue. At $414k/yr fees + $250k setup, that's about $166k
  (≈ 800 engineer-hours at $200/h, an assumption).
* **Gating metric for Series A:** the third deployment needs ≤ 50% of the first one's integration hours.

## 4. Regulatory, grid and counterparty risk

| Risk | Evidence | Mitigation |
|---|---|---|
| Tariff regime in flux during the bridge window | FERC issued show-cause orders to all six RTOs/ISOs on 18 Jun 2026, including *new transmission services for flexible large loads*, following DOE's Oct 2025 large-load ANOPR | Contract grandfathering; caps on signal hours/yr, event duration and notice; change-in-law reopener; model each draft tariff as a world preset |
| Large-load reliability scrutiny | NERC reviewed about 1,500 MW of voltage-sensitive data-center load lost in Loudoun County in Jul 2024 and about 1,800 MW in Feb 2025 | Ride-through settings; coordinated **ramp-rate limits** on shed *and* on the post-event rebound (not yet modelled, see §5) |
| Telemetry and metering verification | Witness tests need revenue-grade POI metering and time-synchronised interval data | Signed telemetry, meter-to-model reconciliation, fail-safe controller (PRODUCTION_SPEC §4) |
| Genset permits | Federal emergency-engine rules: 50 h/yr non-emergency | Gensets aren't a bridge strategy, only backup |
| Counterparties | BESS OEM availability and delivery; EPC schedule; workload-owner deferral rights; the utility's termination right | Availability LDs ≥ 95%; delivery LDs ≥ $3M/month (the model's NPV cost of delay); deferral-rights addenda by workload class |
| Force majeure / SLA | BESS outage or scheduler failure during an event; inference SLAs must not be shed | N-1 inverter design (`bessDerate`), pre-emptive curtailment on stale telemetry, inference never in the deferrable pool |

## 5. MVP-to-production gap map

| # | Prototype shortcut | Production requirement | v0.2 status |
|---|---|---|---|
| 1 | Browser-side math is the only engine | Deterministic, versioned engine service (Rust kernel), with the browser as a viewer; WASM build for offline parity | **Rust 0.2 port done** (`firmgap diligence / robust / stress`), golden-pinned to the web reference; server API and WASM build pending |
| 2 | One synthetic year | Historical nodal LMP / congestion replay + Monte Carlo | Monte Carlo added; replay pending |
| 3 | Sliders only, no cross-field checks | Schema validation at every boundary, clamping, repairs, provenance | Done (clamp, repair, badges, audit) |
| 4 | Ephemeral state | Immutable scenario store, signed snapshots, approval workflow | localStorage + JSON bundles with digest; server store pending |
| 5 | No error states | Typed errors, last-good-result fallback, user-visible messages | Done |
| 6 | Single-world optimizer (knife-edge) | Chance-constrained / robust optimization with explicit design margin | Robust optimizer (≥ 90% of weather years); explicit-margin targets pending |
| 7 | Metric errors (C2, C3) | Investor cash-flow model with full asset life and downside | Done |
| 8 | No engineering derates | Deadband, SoC window, availability, RTE | Done |
| 9 | No ramp-rate / rebound limits | Ramp constraints on shed and recovery; ride-through | **Pending (top modelling gap)** |
| 10 | Monthly = annual ÷ 12 | Calendar-aligned monthly simulation (summer-heavy events) | Pending |
| 11 | No auth / RBAC / multi-user | OIDC SSO, RBAC, tenant isolation | Spec (PRODUCTION_SPEC §3) |
| 12 | No telemetry ingestion | Contracts in `schemas/`, freshness state machine | Spec |
| 13 | CDN runtime code (v0.1 used the Tailwind CDN) | Zero third-party runtime code, CSP | Done (v0.2 has no external code) |
| 14 | Rust never compiled | Green CI on three operating systems | Pending: run `scripts/verify.sh` (now covers model 0.2) |
