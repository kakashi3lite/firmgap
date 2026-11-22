# Context map

This is the single page that tells a new contributor, reviewer, auditor or AI coding agent where
every concept lives, which test protects it, and why it is designed that way. Keep it current: a PR
that moves a concept without updating this file is incomplete.

## 1. Concept → code → test → doc

| Concept | Code | Protected by | Explained in |
|---|---|---|---|
| International rulepacks (0.4) | `src/rulepacks_intl.rs`, web `INTL_REPORTS` | `golden_audit.rs::every_rulepack_has_golden_coverage`, `invariants.rs::every_rulepack_evaluates_every_listed_rule_and_nothing_else`, `applying_a_resolving_remediation_turns_its_rule_green` | COMPLIANCE §1.4–1.8, JURISDICTIONS_2026 |
| Bankability matrix, batch audit, proof links (0.4) | `src/portfolio.rs`, web `compareWith`, `shareToken`, `shareDecode` | `golden_audit.rs::bankability_matrix_is_byte_identical_to_the_web_reference`, `share_tokens_roundtrip_bit_exact`, `invariants.rs::batch_audit_is_deterministic_and_bounded`, fuzz `share`, `scripts/bench.sh` | COMPLIANCE §5, THREAT_MODEL T2d/T2e |
| Compliance gate and releases (0.4) | `.github/workflows/firmgap-compliance-gate.yml`, `release.yml`, `scripts/hygiene.sh` | `cli.rs::fail_on_blocker_respects_expected_verdict`, CI smoke | CI_COMPLIANCE_GATE, THREAT_MODEL T8/T9 |
| Compliance war room UI (0.4) | web Compliance & Bankability block (DOM built with `textContent`) | functional suite (35 checks), built-in self-test, `hygiene.sh` | USER_GUIDE §1–2 |
| Compliance rulepacks & remediation (0.3) | `src/compliance.rs` (`SITE_SPECS`, `compliance_report`) | `tests/golden_audit.rs`, `compliance::tests`, `cli.rs::audit_*` | COMPLIANCE §1-2 |
| Shedding capability curve (0.3) | `src/shed_curve.rs` | `invariants.rs::shedding_curve_is_monotonically_non_increasing_in_duration`, golden curve hash | COMPLIANCE §3 |
| Audit dossier, SHA-256, canonical JSON (0.3) | `src/audit_bundle.rs`, `src/canon.rs` | NIST vectors, `tampering_any_single_byte_fails_verification`, fuzz `dossier`, `golden.mjs` node:crypto cross-check | COMPLIANCE §4, THREAT_MODEL T2a/T2b |
| Case-study defaults & input domain | `src/params.rs` (`numeric_params!`, `SPECS`) | `params::tests`, `cli.rs::rejects_bad_input_*` | MODEL §1, `firmgap params` |
| Input validation & cross-checks | `Params::set`, `Params::validate` | `params::tests`, fuzz target `scenario` | THREAT_MODEL T1 |
| Deterministic randomness | `src/rng.rs` | `rng::tests` (bit-exact vs JS) | MODEL §1.2 |
| Utilization profile | `year::build_year` (first loop) | `year::tests::utilization_mean_matches_input` | MODEL §1.1 |
| Curtailment event generation | `year::build_year` (durations, placement) | `year::tests`, golden `first_events` | MODEL §1.2 |
| Throttle / BESS / genset dispatch | `sim::simulate` | `invariants.rs::hourly_balance_and_ceiling_compliance`, golden | MODEL §2.1 |
| First-minutes (5-min) witness test | ramp segment in `sim::simulate` | golden `failed5`, `certified` | MODEL §2.1 step 5 |
| SLA backlog & breach | `Deferred` queue, `replay` | `invariants.rs::deferred_work_is_conserved` | MODEL §2.2–2.3 |
| Post-firm BESS arbitrage | `Mode::Arbitrage` | golden `firm_arb` | MODEL §2.4 |
| Certification gate & bridge month | `economics::economics` (`certified`, `f`) | golden `F`, `pathways_are_ordered…` | MODEL §3 |
| Two-part BESS CapEx, CRF | `economics::bess_capex`, `crf` | `economics::tests` | MODEL §3 |
| Pathway phasing & carry | `economics::economics` loop | invariants, golden A/B/C | MODEL §3, ARCHITECTURE §5 |
| Breakeven carry for 25% KPI | `economics::economics` tail | golden `breakevenCarry` | MODEL §3 |
| Optimizer | `src/optimise.rs` | golden `optimise` (13.5 MW / 2 h / 12 h, 228 evals) | MODEL §4 |
| Scenario file format | `src/scenario.rs` | `scenario::tests`, fuzz | THREAT_MODEL T1 |
| Output safety | `src/json.rs`, `error.rs` Display | `json::tests`, `cli.rs::error_messages_do_not_echo…` | THREAT_MODEL T2 |
| Web reference & UI | `web/index.html` | `scripts/golden.mjs --check` (CI `web-parity`) | ARCHITECTURE §6 |
| Investor layer (NPV, downside, break-evens) | `src/invest.rs`, `sim::reprice` (web: `investor()`, `reprice()`) | `golden_v2.rs::v2_*`, `invariants.rs::investor_metamorphic_properties` | MODEL §5, INVESTOR_DILIGENCE §1–2 |
| Engineering derates | `Params::effective`, `SimConfig::*` (web: `effective()`) | defaults reproduce v0.1 golden; `golden_v2.rs` | MODEL §5 |
| Weather-year Monte Carlo | `src/robust.rs::monte_carlo`, `seed_list` (web: `monteCarlo`, `seedList`) | `golden_v2.rs::v2_monte_carlo_*`, `robust::tests` | MODEL §5 |
| Robust optimiser | `src/robust.rs::robust_optimise` (web: `robustOptimiseGen`) | `golden_v2.rs::v2_robust_optimiser` | MODEL §5 |
| Stress suite | `src/stress.rs` (web: `stressGen`) | `golden_v2.rs::v2_stress_suite` | MODEL §5, INVESTOR_DILIGENCE §2 |
| World scenarios & designs | `src/worlds.rs` (web: `WORLDS`, `DESIGNS`, `worldParams`) | `worlds::tests`, golden params check | INVESTOR_DILIGENCE §2 |
| Guardrails & verdict | `src/guard.rs` (web: `guardrails()`, `verdictOf()`) | golden codes per scenario, `cli.rs::diligence_*` | MODEL §5, INVESTOR_DILIGENCE §5 |
| Data contracts | `schemas/*.schema.json` | – (to be enforced at ingestion) | PRODUCTION_SPEC §2 |

## 2. Invariants (must always hold)

1. **Hourly balance:** `demand = (grid − catch-up − charge) + throttle + BESS + genset`.
2. **Ceiling:** during a signal, `max(0, grid − ceiling) ≤ shortfall`.
3. **No charging during a signal.**
4. **SoC bounds:** `0 ≤ SoC ≤ E`.
5. **Gap attribution:** `throttle + BESS + genset + shortfall = gap energy`.
6. **Work conservation:** `caught-up + breached ≤ throttled`.
7. **Pathway ordering:** energized MW `A ≤ B ≤ C` every month; carry `A ≥ B ≥ C`.
8. **Determinism:** same inputs give bit-identical outputs.
9. **Parity:** Rust = web model within 1e-6 relative (`tests/golden.json` for model 0.1, `tests/golden_v2.json` for 0.2).
11. **Downside bound:** downside NPV ≥ −BESS CapEx; downside < certified NPV.
12. **Monotonicity:** NPV non-increasing in BESS cost and deferral fee, non-decreasing in margin; shortfall non-increasing in BESS MW and non-decreasing as the ceiling falls.
10. **Supply chain:** zero runtime dependencies; no `unsafe`; no network, process or env access in `src/`.

## 3. Decision log (lightweight ADRs)

| # | Decision | Why | Revisit when |
|---|---|---|---|
| D1 | Zero runtime dependencies (hand-written arg parser and JSON writer) | Removes the supply-chain attack surface and keeps the binary auditable | a dependency gives clear value and passes `deny.toml` |
| D2 | Hourly model + analytic sub-hour ramp segment | Hourly data is what tariffs and LMPs use; the 5-minute test only needs first-minutes coverage | real 5-minute telemetry is integrated |
| D3 | Certification is all-or-nothing on the first-minutes test | A utility witness test is pass/fail; one failure blocks the expansion | the interim agreement allows probation or partial credit |
| D4 | Shortfall must be ~0 in the optimizer, not just priced | The $450/MWh penalty understates termination risk of the interim agreement | contract terms say otherwise |
| D5 | Two-part BESS cost calibrated to the case $/kWh at 4 h | Short-duration systems are power-dominated; flat $/kWh overstates their cost advantage | vendor quotes are available |
| D6 | Web file is the numerical reference; Rust must match it | Stakeholders explore in the browser; the CLI must give the same numbers | the model moves to WASM from the Rust kernel |
| D7 | AGPL-3.0-only + commercial dual license | Keeps improvements open, deters closed SaaS forks, and allows paid licensing | the business model changes (needs all contributors' consent or a CLA) |
| D9 | NPV excludes queue carry | Carry contains delayed revenue that the margin term already counts | carry is split into financing vs. revenue components |
| D10 | Robust optimizer requires ≥ 90% compliant weather years | A single-year optimum is knife-edge | an explicit-margin or chance-constrained policy is adopted |
| D8 | Utilization 0.78 rather than the case's 0.36 MWh/MW-h | 0.36 contradicts the case's own 307,476 MWh/yr | the case is corrected |

## 4. Changing the model

1. Change the rule in **both** `web/index.html` (inside `FIRMGAP-CORE`) and `src/`.
2. `node scripts/golden.mjs --write` regenerates `tests/golden.json` and `tests/golden_v2.json`.
3. `python3 scripts/gen_golden_v2.py` regenerates `tests/golden_v2.rs`; update the literals in `tests/golden.rs` by hand if `golden.json` changed.
4. `./scripts/verify.sh`.
5. Update `docs/MODEL.md`, this file, and `CHANGELOG.md` (state the old and new key numbers).

## 5. Glossary pointer

Terms are defined in [GLOSSARY.md](GLOSSARY.md).
