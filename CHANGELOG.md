# Changelog

All notable changes are documented here. Format: [Keep a Changelog](https://keepachangelog.com),
versioning: [SemVer](https://semver.org).

## [0.2.0] - 2026-11-22
### Added
- Investment workbench `web/index.html` (model 0.2.0): engineering derates (control deadband, usable SoC window,
  BESS availability), deferral fees and SLA penalties, a full-life investor NPV vs. interim-only, downside NPV,
  payback, IRR, break-evens, a weather-year Monte Carlo, a design × world stress matrix, a tornado chart,
  a robust optimizer, strict input bounds with clamping, cross-field repair and guardrail badges,
  localStorage persistence with an in-memory fallback, an audit log, JSON audit bundles with a digest,
  CSV export with a formula-injection guard, and a validated import.
- `docs/INVESTOR_DILIGENCE.md`, `docs/PRODUCTION_SPEC.md`, `schemas/*.schema.json`.
### Changed
- The recommended design is now **18.5 MW / 2 h + 12 h hold** (was 13.5 MW / 2 h, which fails under realistic derates).
- v0.2 does not use the net-value metric: v0.1 double-counted carry and margin and truncated the BESS life.
- Zero third-party runtime code in the web app (v0.1 loaded the Tailwind CDN).
### Unchanged
- At default (v0.1) inputs the kernel reproduces `tests/golden.json` exactly (verified: 0 differences).
- **Rust crate 0.2.0**: full port of model 0.2. New modules: `invest` (investor NPV), `robust` (Monte Carlo + robust
  optimiser), `stress` (matrix, tornado, break-evens, cliffs, intensity), `worlds` (scenario overlays), `guard`
  (guardrails + verdict). New CLI commands: `diligence`, `robust`, `stress`, `montecarlo`, `worlds`, plus the
  `--world`, `--design` and `--years` options. New parameters: `safety_margin`, `bess_derate`, `soc_window`, `defer_cost`,
  `sla_penalty`, `salvage`, `margin_pre_energy`, `sw_setup`, `sw_price_mw_month`, `mc_seeds`.
- `tests/golden_v2.json` + generated `tests/golden_v2.rs` (18 parity tests); `scripts/gen_golden_v2.py`;
  `scripts/golden.mjs` now checks both golden files.
### Fixed
- Both optimisers reused the interim-only simulation across throttle-hold policies. Once a control deadband pushes
  the effective ceiling below interim capacity, that simulation depends on the hold policy. It is now computed per
  policy. No reported number changed: at the case utilisation the interim campus never exceeds the derated ceiling.
### Removed
- Parameter `margin` (v0.1 contribution margin used by the double-counted net-value metric). Use `margin_pre_energy`.
  Scenario files that still set `margin` are rejected with "unknown parameter".
### Known gaps
- Ramp-rate and rebound limits are not yet modelled.
- Historical nodal LMP replay is not yet supported.

## [0.1.0] - 2026-11-22
### Added
- Rust library + CLI (`run`, `optimise`, `sweep`, `params`, `presets`) with zero runtime dependencies.
- 8,760-hour dispatch engine: throttle / BESS / genset, first-minutes certification test, SLA backlog.
- Pathway economics A/B/C, two-part BESS CapEx, CRF annualization, breakeven-carry diagnostic.
- Constrained optimizer (228 evaluations on the case): 13.5 MW / 2 h BESS + 12 h hold.
- Single-file interactive web simulator `web/index.html` (also the numerical reference).
- Golden parity tests, invariant tests, CLI tests, fuzz target.
- CI: cross-platform tests, MSRV 1.74, web parity, cargo-deny, cargo-audit, CodeQL, Scorecard, Dependabot.
- Documentation: model, architecture, context map, verification, threat model, hardening, publishing.

### Key results (case study)
- Engineering proposal 23 MW / 4 h: +2.0% all-in $/MWh vs. waiting, 92 MWh/yr shortfall.
- Right-sized 13.5 MW / 2 h: -7.2%, ~0 shortfall, 0.30% SLA breach, 28 months pulled forward, -64% BESS CapEx.
