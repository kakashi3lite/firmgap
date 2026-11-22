# Model specification

This is the authoritative description of what firmgap computes. Code references point to the Rust
kernel; the web reference implements the same rules line by line (between the
`FIRMGAP-CORE-BEGIN/END` markers in `web/index.html`).

Notation: `t` = hour of year (0…8759), `m` = month (0…horizon−1), `L_t` = campus load (MW, and
MWh for a 1-hour step), `C` = curtailment ceiling (`ceiling_mw`).

---

## 1. Synthetic year — `src/year.rs`

### 1.1 Utilization
```
d(t) = 1 + 0.06 · cos(2π (hod − 15) / 24)        diurnal, peak 15:00
s(t) = 1 + 0.03 · cos(2π (doy − 200) / 365)      seasonal, peak ≈ 19 July
u(t) = f32( min(1, util · d(t) · s(t)) )
L_t  = cap · u(t)
```
The mean of `u` equals `util` to within 0.1% (test `utilization_mean_matches_input`). At the defaults,
`45 MW × 0.78 × 8,760 h = 307,476 MWh/yr`, which matches the case.

> **Case-data note:** the case also quotes "0.36 MWh/MW-hour". That is inconsistent with its own
> 307,476 MWh/yr total, which implies 0.78. firmgap uses 0.78.

### 1.2 Curtailment signals
* `target = round(stress_hours)`, `n = clamp(round(target / avg_event_h), 1, target)`.
* Durations: `round(−ln(1 − 0.999·U) · avg_event_h)` clipped to [1, 12] h, then ±1 h random
  adjustments until they sum to `target` (bounded loops).
* Placement, up to 400 tries per event: 75% summer afternoons (day 152–273, start 12:00–16:00),
  25% winter evenings (Dec–Feb, start 16:00–18:00). There's a 3-hour guard band between events.
* Mulberry32 PRNG seeded by `seed`, bit-identical in Rust and JS.
* Signals are assumed coincident with the high-LMP hours: the stress price applies to every signal
  hour.

## 2. Dispatch — `src/sim.rs::simulate`

Four configurations are simulated on the same year:

| Config | cap | ceiling during signal | BESS | genset | mode |
|---|---|---|---|---|---|
| `interim` | interim_mw | ceiling_mw | – | – | Compliance |
| `flex` | target_mw | ceiling_mw | yes | yes | Compliance |
| `firm` | target_mw | none | – | – | Plain |
| `firm_bess` | target_mw | none | yes | – | Arbitrage |

### 2.1 Compliance mode, signal hour with `gap = L_t − C > 0`
1. `k` = hour index within the event (1-based). Throttling is allowed only if `k ≤ max_throttle_h`.
2. Throttle capacity `T = defer_share · L_t`. Instant power-cap capacity
   `F = min(fast_cap, defer_share) · L_t`.
3. Segments: if `k = 1`, there is a **ramp segment** of length `δ = min(1, ramp_min/60)` h using `F`,
   then the remainder `1−δ` using `T`. Otherwise one segment of 1 h using `T`.
4. In each segment of length `δ`, the gap power is covered in strategy order:
   * throttle-first: `x = min(gap, cap_thr)`, then `y = min(gap − x, min(P, SoC/δ))`;
   * BESS-first: the reverse.

   Then genset `z = min(rest, gen_mw)` if genset run hours `< gen_cap_h`, and shortfall `r = rest`.
   Energies are `x·δ, y·δ, z·δ, r·δ`; `SoC −= y·δ`.
5. **First-minutes test:** in the ramp segment, if `r > 10⁻⁶` the event fails (`pass5 = false`).
   Coverage `= (gap − r)/gap`.
6. Grid draw `= L_t − throttle − BESS − genset`, so the draw above `C` equals the shortfall
   (invariant test).
7. Throttled energy is queued FIFO with deadline `t + sla_window + 1`.

### 2.2 Signal hour without a gap
Backlog is replayed into headroom `max(0, min(cap, C) − L_t)`.

### 2.3 Non-signal hour
* Expired backlog becomes an **SLA breach**. It's billed at the base price when it's eventually run.
* Backlog replays into `max(0, cap − L_t)`.
* The BESS recharges at `min(P, grid_cap − grid, (E − SoC)/rte)`. It never charges during a signal.

### 2.4 Arbitrage mode (firm + BESS)
During stress hours the BESS discharges `min(P, SoC, L_t)`. It recharges off-peak as above. This
gives the post-firm arbitrage value.

### 2.5 Annual outputs
`demand, energy_cost = Σ grid·price + Σ breach·base_price, thr_e, bes_e, gen_e,
gen_fuel = gen_e·gen_cost, short_e, penalty_cost = short_e·penalty, breach, breach_pct =
breach/demand, failed5, pass5_all`.

## 3. Economics — `src/economics.rs`

* **Certification:** `certified = pass5_all(flex)`. If any event fails, the utility does not sign
  the non-firm expansion.
* **Bridge month:** `F = min(queue_months, interim_month + bess_lead)` if certified, otherwise
  `queue_months`.
* **BESS CapEx (two-part):** `e_rate = max(0, bess_cost − bess_power_cost/4)`;
  `capex = MW·1000·bess_power_cost + MWh·1000·e_rate`. A 4-hour system costs exactly
  `bess_cost` $/kWh.
* **Annualization:** `CRF(r,n) = r(1+r)ⁿ / ((1+r)ⁿ − 1)`; `bess_ann = capex · CRF(wacc, life)`,
  charged monthly from `F` onward when certified.
* **Genset rental:** `gen_mw · 1000 · gen_rental` per year, charged during flex months only.

Monthly phasing (energy and costs = annual simulation ÷ 12):

| month | A (wait) | B (interim only) | C (flex bridge) |
|---|---|---|---|
| `< interim_month` | 0 | 0 | 0 |
| `[interim, F)` | 0 | interim sim | interim sim |
| `[F, queue)` | 0 | interim sim | flex sim + rental |
| `≥ queue` | firm sim | firm sim | firm_bess sim (or firm) |

* **Carry:** `carry_m = carry · (1 − energized_MW / target_MW)`.
* **All-in:** `total = energy + bess + gen + penalty + carry`; `per_mwh = total / mwh`.
* **Reduction vs. waiting:** `1 − C.per_mwh / A.per_mwh`.
* **Breakeven carry** for a 25% reduction. With `f_X = (total_X − carry_X)/mwh_X` and
  `k_X = carry_months_X / mwh_X`:
  `carry* = (f_C − 0.75 f_A) / (0.75 k_A − k_C)` (∞ if the denominator ≤ 0).

## 4. Optimizer — `src/optimise.rs`

Minimize `C.total` subject to `certified ∧ short_e < 0.5 MWh ∧ breach_pct < sla_target`.

The search runs over hold `{3,4,6,12}` × duration `{1,2,4}` × power: coarse 0–32 MW in 2 MW steps,
refined in 0.5 MW steps within ±1.5 MW of each coarse optimum. That's 228 full-year simulations on
the case.

## 5. Model 0.2: derates, investor view, robustness (`src/invest.rs`, `src/robust.rs`, `src/stress.rs`, `src/guard.rs`)

**Engineering derates** (`Params::effective`). The defaults (0, 1, 1) reproduce model 0.1 exactly.

| Effective quantity | Formula |
|---|---|
| Ceiling | `max(0, ceiling_mw − safety_margin)`, used in both the interim and flex simulations |
| BESS power | `bess_mw × bess_derate` |
| BESS energy | `bess_mw × bess_h × soc_window` |

CapEx is always charged on nameplate.

**Investor NPV vs. interim-only (pathway B).** Monthly rate `rm = (1+wacc)^(1/12) − 1`; discount factor
`df(m) = (1+rm)^−m`.

* Month `interim_month`: −CapEx (`capex = 0` without a BESS).
* Bridge months `[F, queue)`, where `F = round(full_month)` if certified, else `queue`:
  * `+ margin_pre_energy × ΔMWh/12`
  * `− Δenergy/12 − Δpenalty/12`
  * `− (genset fuel + rental)/12`
  * `− defer_cost × throttled/12`
  * `− sla_penalty × SLA-late/12`
* After `max(queue, online)` (with `online = interim_month + bess_lead`), whichever is worth more:
  * keep the BESS for arbitrage at `max(0, Δfirm energy cost)/12` per month until end of life
    (`online + 12·life`);
  * resell it at `salvage × capex` (at `queue` on success, `min(online, queue)` on failure).
* **Downside:** the same flows with no bridge months (built, then the witness test fails).
* **Payback:** months from commitment until cumulative undiscounted cash flow ≥ 0.
* **IRR:** bisection on the monthly rate in [−0.99, 5], annualised.
* **Break-evens** are closed form:
  * margin: `margin − NPV / Σdf·ΔMWh`;
  * deferral fee: `defer_cost + NPV / Σdf·throttled/12`;
  * SLA penalty: `sla_penalty + NPV / Σdf·late/12`.
* **Cost per MW-month:** `(PV margin + PV energy − NPV) / ((target − interim) × bridge months)`, which is
  the PV of every non-energy bridge cost net of post-firm value.
* **Queue carry is excluded** (decision D9 in CONTEXT_MAP).

**Compliance:** `certified ∧ shortfall < 0.5 MWh/yr ∧ SLA-late share < sla_target`.

**Monte Carlo:**
* Seeds are `seed + i × 2654435761 (mod 2^32)`.
* Each year is a full re-run.
* Quantiles are nearest rank, at index `round(x·(n−1))`.

**Robust optimiser:**
1. Holds `{3,4,6,8,12,16,24} ≤ current hold` ∪ `{current}`, times duration `{1,2,4} h`.
2. For each pair, scan power 0–40 MW in 2 MW steps, then refine ±1.5 MW in 0.5 MW steps around that pair's best.
3. Maximise NPV subject to compliance on the base year. The ranking NPV excludes post-firm arbitrage.
4. The interim-only simulation is recomputed per hold.
5. Then add 0.5 MW at a time (to at most 60 MW) until the design is compliant in ≥ 90% of
   `min(16, mc_seeds)` weather years.

**Stress suite:**
* a design × world matrix;
* a 12-driver tornado (cost and timing drivers are re-priced exactly on the base simulations);
* break-evens: BESS cost and WACC by 50-step bisection, queue and lead time by integer scan;
* compliance cliffs by stepping each physical lever until compliance is lost;
* compliance vs. 180–600 signal h/yr over 6 weather years per level.

**Guardrails and verdict:**
* 22 coded checks (see `src/guard.rs`).
* Any violation gives NOT INVESTABLE.
* Otherwise: no Monte Carlo gives PENDING, any warning gives CONDITIONAL, and none gives INVESTABLE.

## 6. Known simplifications

See [VERIFICATION.md §4](VERIFICATION.md#4-known-limitations). Main ones (plus: no ramp-rate or rebound
limits on shed and recovery yet):
* hourly resolution with an analytic sub-hour ramp segment;
* one synthetic year (re-roll `seed` for weather risk);
* a stylised two-price tariff;
* no BESS degradation or augmentation;
* no demand-charge or capacity-market revenue;
* SLA breach measured in energy, not per job.
