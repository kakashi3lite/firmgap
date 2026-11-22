# Verification report

This report covers what was checked, how, against which sources, and what is still unverified.
Status as of **22 Nov 2026**.

## 1. Numerical verification

| Check | Method | Result |
|---|---|---|
| Rust ⇄ web parity (model 0.1) | 7 scenarios + optimizer, about 40 quantities each, asserted to 1e-6 relative in `tests/golden.rs` | Golden values generated from the reference model; `cargo test` enforces them |
| Web model stability | `node scripts/golden.mjs --check` in CI (`web-parity` job) | Fails on any drift beyond 1e-9 |
| PRNG | first 5 Mulberry32 draws compared bit-exactly | `rng::tests` |
| Physical invariants | 11 scenarios × 8,760 h: balance, ceiling, SoC, no charging in signal, gap attribution, work conservation | 0 violations in the f64 reference run (Rust runs the same assertions) |
| Annual energy | 45 MW × 0.78 × 8,760 h | 307,476 MWh (model: 307,475.99996) |
| Case carry | $145k × 38 months | $5.51M (model pathway A: $5,510,000) |
| Optimizer | 228 full-year evaluations | 13.5 MW / 2 h, hold 12 h: CapEx $9.38M, shortfall 0.19 MWh/yr, SLA 0.30% |

> **First compile happens in CI.** The Rust sources were written and cross-checked against the
> reference model line by line, but this environment had no Rust toolchain. The first
> `./scripts/verify.sh` (or the first CI run) is the compiler's first pass. Any failure there is
> a porting defect to fix before release, not a modelling question.

### Model 0.2 (Rust port)

| Check | Method | Result |
|---|---|---|
| Web reference parity | `scripts/golden.mjs --check` run on the shipped `web/index.html` against both golden files | 0 differences (both files) |
| Rust ⇄ web parity, model 0.2 | 18 generated tests in `tests/golden_v2.rs`: 12 world × design scenarios (params, dispatch, economics, 24 investor fields, guardrail codes, verdict), 3 Monte Carlo runs (24 years each), robust optimiser (351 evaluations), full stress suite (42 matrix cells, 12 tornado drivers, 13 thresholds, 6 intensity levels) | Written; **first compile and run happen in your `scripts/verify.sh` or CI** |
| Re-pricing exactness | cost/timing changes re-priced on base sims vs. full re-run | Bit-identical (web: 0 diffs over 10 parameters; Rust: `to_bits` assertion in `invariants.rs`) |
| Web workbench end-to-end | 20 functional tests driving the real UI code (presets, clamping, repair, stress suite, import tamper, CSV injection, self-test 19/19, robust optimiser) | 20/20 pass |
| Bug found during the port | Both optimisers reused the interim-only simulation across hold policies, but it depends on the hold once a deadband applies | Fixed in both implementations; case numbers unchanged |

> **The Rust code has still never been compiled.** It was ported line by line against the reference, with
> manual type and lint review, but this environment has no Rust toolchain. Treat the first
> `./scripts/verify.sh` as the real test. If it reports an error, paste it into the chat and it gets fixed.

## 2. Case-study consistency findings

1. **Energy intensity contradiction:** the case says "0.36 MWh/MW-hour" and also
   "~307,476 MWh/yr at 45 MW". Those two can't both hold: 307,476 / (45 × 8,760) = 0.78. firmgap
   uses 0.78.
2. **Carry looks low:** $145k/month on $68M stranded CapEx is about 2.6%/yr, below any plausible
   cost of capital. That's why the ">25% all-in $/MWh" KPI is unreachable. The model reports the
   breakeven carry ($470,725/month, about 8.3%/yr).
3. **Penalty understates risk:** at $450/MWh, the uncontrolled annual gap (2,651 MWh) costs only
   about $1.19M/yr. The real exposure is losing the interim agreement. Hence decision D4 in
   CONTEXT_MAP.

## 3. External research check

The claims below support the problem framing and the default assumptions. Each was checked
against the linked source; dates are publication dates.

| Claim used | Finding | Source |
|---|---|---|
| Flexible large loads can connect faster if they accept curtailment | Duke Nicholas Institute "Rethinking Load Growth" (Feb 2025): 76 GW at 0.25% curtailment, 98 GW at 0.5%, 126 GW at 1% across 22 balancing authorities | https://www.utilitydive.com/news/us-grid-headroom-flexible-load-data-center-ai-ev-duke-report/739767/ ; https://gridlockedunlocked.substack.com/p/from-flexible-loads-to-ai-factories |
| ML / batch workloads are a real demand-response resource | Google signed DR agreements with Indiana Michigan Power and TVA targeting ML workloads (Aug 2025) and later reported 1 GW of data-center DR | https://www.publicpower.org/periodical/article/google-tva-enter-agreement-tied-data-center-demand-response ; https://blog.google/innovation-and-ai/infrastructure-and-cloud/global-network/demand-response-data-center-milestone |
| AI clusters can cut power for hours without breaking QoS | Emerald AI / Oracle / NVIDIA / SRP / EPRI field test in Phoenix: 256-GPU cluster cut power 25% for 3 h during a grid peak while meeting QoS | https://arxiv.org/html/2507.00909v1 |
| Policy is moving toward curtailable large loads | Texas SB 6 (signed 20 Jun 2025) requires transmission-level large loads to install equipment allowing curtailment during firm load shed; PJM's Critical Issue Fast Path on large load additions debated non-capacity-backed load | https://capitol.texas.gov/tlodocs/89R/analysis/pdf/SB00006F.pdf ; https://www.pjm.com/-/media/DotCom/committees-groups/cifp-lla/2025/20251024/20251024-item-04b---abe-silverman---sue-glatz-proposed-package.pdf |
| BESS cost of about $285/kWh (4 h) is plausible | NREL "Cost Projections for Utility-Scale Battery Storage: 2025 Update" projects 4-h systems at $152 / $247 / $349 per kWh (low/mid/high) in 2035, so today's cost sits above the 2035 mid case | https://www.osti.gov/biblio/2583471 |
| Diesel gensets can't be the bridge | RICE NESHAP: emergency engines may run 50 h/yr in non-emergency situations, counted within 100 h/yr for maintenance and testing; the court-vacated emergency-DR provisions were removed in 2022 | https://www.govinfo.gov/content/pkg/CFR-2024-title40-vol15/pdf/CFR-2024-title40-vol15-part63-subpartZZZZ-subjectgroup-id33.pdf ; https://www.govinfo.gov/content/pkg/FR-2022-08-10/pdf/2022-17060.pdf ; https://www.epa.gov/stationary-sources-air-pollution/clean-air-act-resources-data-centers |
| Security tooling choice | CodeQL scanning for Rust became generally available on 14 Oct 2025 | https://github.blog/changelog/2025-10-14-codeql-scanning-rust-and-c-c-without-builds-is-now-generally-available/ |

**Read these with care:**
* The genset cap applies to *emergency* engines under federal rules. A non-emergency or
  state-permitted engine may have different limits. `gen_cap_h` is a parameter for that reason.
* The Phoenix result covers a 256-GPU cluster for 3 hours. firmgap's 12-hour hold on 35% batch load
  is a planning assumption that needs workload-specific validation.
* The BESS cost is a case input, not an NREL figure. Use vendor quotes for decisions.

## 4. Known limitations

1. **Synthetic weather year.** Events are generated, not taken from the ISO's historical LMP or
   congestion record. Re-roll `seed` and stress `stress_hours`, `avg_event_h` and `ceiling_mw`.
   Historical replay is on the roadmap.
2. **Two-price tariff:** base vs. stress price; no nodal LMP time series, demand charges or
   capacity-market value.
3. **Hourly resolution:** the sub-hour ramp is analytic, with no 4-second telemetry.
4. **BESS:** constant round-trip efficiency, no degradation, augmentation, auxiliary load or SoC
   reserve policy.
5. **SLA:** measured as late energy, not per job. Deferral is assumed to be work-conserving.
6. **Economics:** monthly = annual ÷ 12 (no seasonality across the phasing), no tax, no escalation,
   no salvage value.
7. **Single site, single interim agreement.**

## 5. How to re-verify

```bash
./scripts/verify.sh                         # fmt, clippy, tests, web parity, forbidden-API scan
firmgap run --preset right-sized --json | jq .kpis
node scripts/golden.mjs --check
```
