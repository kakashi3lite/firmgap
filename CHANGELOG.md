# Changelog

All notable changes are documented here. Format: [Keep a Changelog](https://keepachangelog.com),
versioning: [SemVer](https://semver.org).

## [0.4.0] - 2026-11-22
### Added
- **Five more rulepacks** (`src/rulepacks_intl.rs`), checked against sources on 2026-11-22:
  Virginia SB 253 + HB 507 + HB 284 (`US-VA-SB253-2026.06`), GB NESO Gate 2 + Ofgem proposed data-centre reforms
  (`GB-NESO-OFGEM-DC-2026.09`), Ireland CRU/2025/236 (`IE-CRU-2025-236-2026.05`), Spain Real Decreto 1032/2026
  (`ES-RD-1032-2026.11`, in force 2026-11-22) and Singapore GDCR / DC-CFA2 (`SG-IMDA-GDCR-DCCFA2-2026.03`). Each has
  remediations (Tier IV backup, Gate 2 and milestone evidence, de-rated storage for Irish MIC, hourly renewables or the
  interruptible path, green-energy pathways).
- **Rulepack registry**: `JurisdictionRulepack::ALL`, `status()`, `rules()`; `firmgap rulepacks [--json]`.
- **`firmgap compare-jurisdictions`**: a ranked global bankability matrix with its own `matrix_sha256`; text, JSON or Markdown.
- **`firmgap batch-audit --dir <dir> --jurisdiction <id|all|declared>`**: deterministic portfolio sweeps (files in name
  order, one simulation per site reused across rulepacks, at most 1,000 files of 64 KiB each), with `batch_sha256`.
- **Shareable proof links** `#s=<base64url>`: the web workbench encodes the scenario and its dossier hash;
  `firmgap audit --share <url>` reproduces the same dossier and fails (exit 1) if the hash differs.
- **Compliance gate**: scenario metadata `jurisdiction` and `expected_verdict` (not part of any hash),
  `--fail-on-blocker` (exit 3), `--format markdown`, and `.github/workflows/firmgap-compliance-gate.yml`.
- **Release pipeline** `.github/workflows/release.yml`: static binaries for Linux x86_64 / aarch64 (musl), macOS
  aarch64 / x86_64 and Windows (crt-static), `SHA256SUMS`, the standalone web workbench and a GitHub Pages deployment.
- **Compliance & Bankability war room** in `web/index.html`: four zones (law and role, verdict with three KPIs, inputs
  and live proof, cryptographic trust bar), a CFO / General Counsel / Grid Engineer role lens that rephrases every
  live takeaway, captions and a live line on every control, expandable rule rows (citation, why, what to deduce),
  a conditional path-to-compliance card with animated **Apply remediation**, an accessible SVG curve, a global
  matrix, **Copy proof link**, **Print filing memo** (print stylesheet) and an **Explain this screen** walkthrough.
- Strict CSP (`default-src 'none'`); proof links capped at 16 KiB and dossiers at 1 MB, allowlisted keys and no
  HTML sinks in the compliance UI (enforced by `scripts/hygiene.sh`).
- `scripts/bench.sh` (release performance gate) and `scripts/hygiene.sh`; fuzz target `share`.
- Docs: `docs/CI_COMPLIANCE_GATE.md`; COMPLIANCE, USER_GUIDE, JURISDICTIONS_2026, THREAT_MODEL, VERIFICATION,
  PRODUCTION_SPEC, INVESTOR_DILIGENCE, ARCHITECTURE and CONTEXT_MAP updated.
### Changed
- Engine version 0.4.0. Dossier hashes change because the body records `engine_version` and the 16 new site inputs;
  dossiers written by 0.3.0 still verify. No simulation number changed (golden.json and golden_v2.json unchanged) and
  the eight 0.3 golden cases keep their verdicts, failed rules and remediations.
- `audit --jurisdiction` is optional when the scenario declares `jurisdiction`.
- The three bundled B2B scenarios declare their rulepack and expected verdict.
- CLI exit codes: 0 ok, 1 integrity failure, 2 usage or input error, 3 compliance gate failure.
- Release assets are limited to the binary and web bundles (the Pages archive is deployed, not attached).

## [0.3.0] - 2026-11-22
### Added
- **Compliance rulepacks** (`src/compliance.rs`): ERCOT / Texas SB 6 (`TX-SB6-16TAC25.194-2026.10`), FERC RM26-4
  (anticipated terms, `US-FERC-RM26-4-2026.01`) and EU EED Art. 12 (`EU-EED-ART12-2026.01`), with a citation and legal
  basis per check, gross-MW SB 6 security and forfeiture drag, and a counterfactual path-to-compliance solver
  (Path A LMP vs Path B reliability service; minimum BESS vs minimum SLA slack; permit-hour BESS bisection).
- **Shedding capability curve** (`src/shed_curve.rs`): certified sheddable MW at 15 min / 30 min / 1 h / 2 h / 4 h from a
  KKT dual-price GPU power-capping allocator (64-step bisection), shadow prices and the BESS CapEx it avoids.
- **Audit dossier** (`src/audit_bundle.rs`, `src/canon.rs`): pure-std FIPS 180-4 SHA-256 (NIST vectors), canonical JSON
  that is byte-identical to the browser, `firmgap audit --scenario --jurisdiction [--out]`, `--verify` (exit 1 on
  tampering) and `--expect <sha256>` for out-of-band pinning.
- Web workbench: a **Compliance & Bankability** tab with jurisdiction presets, a traffic-light strip with citations,
  remediation cards with **Apply remediation**, a live SVG shedding curve, dossier download and a verify drop zone
  (WebCrypto cross-check). The built-in self-test grows to 25 checks.
- `scenarios/ercot-sb6-100mw-compliant.toml`, `ercot-sb6-illegal-stack.toml`, `ferc-rm26-4-colocated.toml`;
  `schemas/compliance_dossier.schema.json`, `schemas/compliance_rulepack.schema.json`; `docs/COMPLIANCE.md`,
  `docs/USER_GUIDE.md`, `docs/JURISDICTIONS_2026.md` (verified research on Virginia, GB, Ireland, Spain and Singapore), `docs/README.md`.
- Tests: `tests/golden_audit.rs` (8 dossiers compared byte for byte with the web), 4 new invariants (curve monotonicity,
  security invariant to BESS size, SHA-256 / determinism, single-byte tampering), CLI audit tests, and the `dossier` fuzz target.
### Changed
- The web dispatch trace uses Float64Array (it was Float32Array), so hourly POI checks match Rust exactly.
  No reported v0.1 or v0.2 number changed (golden.json and golden_v2.json are unchanged).
- Scenario files may carry compliance site keys; `--set` accepts them too. Existing files are unaffected.

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
