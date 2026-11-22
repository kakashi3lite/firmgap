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


## Models 0.3 and 0.4: compliance sources (checked 22 Nov 2026)

| Claim used in a rule | Source | Status |
|---|---|---|
| 75 MW large-load threshold; study fee of at least $100,000 | Tex. SB 6 enrolled, PURA §37.0561(c), (f) | Statute |
| Reliability service excludes price-responsive loads and loads in other reliability or ancillary services | SB 6, PURA §39.170(b)(3) | Statute |
| Firm-load-shed curtailment equipment before interconnection (loads interconnected after 2025-12-31) | SB 6, PURA §39.170(a) | Statute |
| Backup generation that can serve ≥ 50% of demand: disclosure and directed deployment | SB 6, PURA §37.0561 | Statute |
| $50,000/MW security; 20% retained on post-allocation withdrawal; 20% returned at energization, then 20% per SLLIA milestone; T-bill interest floor | PUCT Project 58481, order adopting 16 TAC §25.194 (filed 2026-09-18) | Adopted rule (the proposal had $100,000/MW) |
| Requested peak demand is not netted for co-located generation | Same order | Adopted rule |
| No codified 30-minute curtailment window | Same order | Hence `SB6_DISCONNECT_30M` is a Warning-level planning test |
| NOGRR 282 ride-through | ERCOT NOGRR 282 (PUCT approved 2026-07-09, effective 2026-08-01); NPRR 1308 definition | Approved protocol |
| FERC large-load interconnection | Docket RM26-4 (DOE ANOPR); show-cause orders of 2026-06-18 | **No final rule** |
| Virginia SB 253: SCC may assign capacity-auction and distribution/substation financing costs to high-load customers | Virginia Mercury (2026-03-17, 2026-04-22, 2026-04-23); bill summaries (25 MW threshold, SCC staff estimate +15.8% for data centers) | Statute; threshold and estimate *reported* |
| Virginia HB 507: Tier IV or equivalent for data-center generator permits after July 2026 | Virginia Mercury (2026-03-17); Governor's release (2026-06-30) on stricter backup-generator standards | Passed; confirm the signed text |
| Virginia HB 284 / SB 371 voluntary demand-flexibility programs | Governor's release, 2026-06-30 | Signed |
| GB Gate 2 readiness; Ofgem data-centre commitment fee and queue milestones | NESO connections-reform documents; Ofgem consultation 2026-07-29 to 2026-09-17 (awaiting decision); Ofgem call for input 2026-02-12 (Curate, Plan, Connect) | Gate 2 approved; fee and milestones **proposed** |
| Ireland: dispatchable capacity matching MIC (≥ 10 MVA) or autoproducer (1–10 MVA), de-rated on CRM T-4, no run-hour limits, ≥ 80% additional renewables with six-year glide path | CRU/2025/236 (Arthur Cox, 2025-12-16; William Fry; DLA Piper); EirGrid process v3 (May 2026) | Adopted |
| Spain RD 1032/2026: ≥ 10 MW, PUE ≤ 1.25, WUE ≤ 0.1, 80% hourly new renewables, interruptible path, sanctions, MITECO reporting | ecoticias (2026-11-22, citing BOE and MITECO); El Confidencial, La Vanguardia, okdiario (2026-11-20) | In force 2026-11-22; figures *reported*, check the BOE text |
| Singapore: PUE 1.3 roadmap target; DC-CFA2 ≥ 200 MW, ≥ 50% green energy, closed 2026-03-31 | IMDA Green Data Centre Roadmap (2024-05-30); Morgan Lewis (2026-03-03) | Adopted policy; 1.25 award bar *reported* |
| EED data-centre reporting ≥ 500 kW IT | Directive (EU) 2023/1791 Art. 12 and Annex VII; Delegated Regulation (EU) 2024/1364 | Reporting obligation |
| GPU power-capping workload allocation | arXiv:2611.19073 (Mishra, Omidi, Almassalkhi, 2026-11-14) | Preprint |
| Storage and compute co-optimisation | arXiv:2605.16190 v2 (Liu, Shin, Deka, 2026-08-17) | Preprint |

**Assumptions, not sourced:** the reliability-service value ($40k/MW-yr), withdrawal probability (10%), collateral
yield (4%), months to energization (24), milestone spacing (12 months), the SLA liquidated-damages rate, workload
values, directed-backup hours, the Virginia uplift (15.8% as reported), the GB commitment fee ($20k/MW), the Irish
de-rating factors and the 8,000 h/yr run-hour threshold. All are flagged `ASSUMPTION` in the site specs and the web inputs.

### Evidence for 0.4 (release package, 22 Nov 2026)

Run in the packaging environment against the exact files in the release zip:

| Check | Result |
|---|---|
| `scripts/golden.mjs` logic: golden.json, golden_v2.json, golden_audit.json | All three reproduce. 19 dossiers (every rulepack has passing and failing cases), 2 bankability matrices and 3 proof-link tokens byte-identical |
| Browser SHA-256 vs `node:crypto` | Identical on the NIST vectors and UTF-8 input; all dossiers and matrices re-verify with an independent Node verifier |
| Proof-link round trip | 3/3 tokens decode and reproduce the sender's dossier hash; 12 hostile tokens rejected (bad charset, wrong type, unknown keys, `__proto__`, `../` paths, > 16 KiB, unknown jurisdiction) |
| Remediations | 14/14 resolving remediations turn their rule green when applied |
| Web war-room functional suite (DOM emulation) | 35/35: every zone, role lens, captions + live line on all 17 rail controls, Apply remediation, curve sliders, Explain this screen, matrix navigation, proof link round trip, hostile link, dossier verify (intact, 1-byte tamper, > 1 MB, markup in a dossier), print memo |
| v0.2 regression suite | 20/20 (Base Case NPV $66.6M unchanged) |
| Built-in self-test | 28/28 |
| JSON Schemas | 22/22 dossiers and 2/2 matrices valid |
| Rust ↔ JS rule text | All 139 message literals of the international rulepacks and all 40 citations identical |
| Hygiene | `scripts/hygiene.sh` uses portable `grep -w` (BSD and GNU). No to-do markers, no HTML sinks in the compliance UI block, every action pinned to a 40-character SHA, top-level `permissions` in all 6 workflows, plain `pull_request` events only |

| Static Rust review (no compiler) | Every new Rust file read against the borrow checker, type inference and current clippy defaults. Fixed: 1 compile error (`&str == &String`), 2 closure-pattern inference failures, 3 clippy risks (redundant closure, `map().unwrap_or(false)`, clone-assign), 1 test `is_err() \|\| unwrap()` pattern. Bracket balance clean in all 16 touched files; MSRV 1.74 features only (`let-else`, `is_some_and`, `is_ok_and`, `div_ceil`) |
| Release workflow review | Fixed: the GitHub Pages archive (`github-pages` artifact) would have been published as a release download; release assets are now limited to `bin-*` artifacts |

**Not run here (no Rust toolchain in the packaging sandbox):** `cargo fmt --check`, `cargo clippy`, `cargo test`,
`cargo build --release`, the `/tmp` smoke tests and `scripts/bench.sh`. They are all in `scripts/verify.sh` and CI.
The Rust test suite requires the Rust dossiers, matrices and proof-link tokens to equal the browser's byte for byte
and prints the first differing byte on a mismatch. The release performance targets (single-site audit < 10 ms,
100-site sweep across 8 rulepacks < 1 s) are enforced by `scripts/bench.sh` on the maintainer's machine; the
browser engine, for reference, builds one dossier in about 60 ms and a full 8-corridor matrix in about 66 ms.

Reference hash: `ercot-sb6-100mw-compliant` → `dossier_sha256 = ec6bf71a694acbd4e75a1022c5ff052558ab41fa2231bd39107466c0f6c1662e`.
