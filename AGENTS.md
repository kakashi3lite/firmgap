# AGENTS.md: rules for human and AI contributors

This file is read by coding agents (and humans) before they change anything. It keeps the
project's context intact across contributors and sessions.

## Read first
1. `docs/CONTEXT_MAP.md`: where every concept lives, its invariants and the decision log.
2. `docs/MODEL.md`: the authoritative equations.
3. `docs/THREAT_MODEL.md`: what must never be weakened.

## Hard rules
- **Never add a runtime dependency** to `Cargo.toml` `[dependencies]`. CI fails if you do (decision D1).
- **Never use `unsafe`**, `std::net`, `std::process::Command` or environment variables in `src/`.
- **Model changes go to both implementations** (`web/index.html` FIRMGAP-CORE block and `src/`),
  then regenerate golden values (`node scripts/golden.mjs --write` + `python3 scripts/gen_golden_v2.py`, CONTEXT_MAP §4). A PR that changes only one side is rejected.
- **Compliance rules are law-adjacent.** Any change to `src/compliance.rs` must keep its citation accurate, change the
  web `CIT` / `complianceReport` block identically, bump the rulepack id's `YYYY.MM` suffix when the rule text changes,
  and regenerate `tests/golden_audit.json` (`node scripts/golden.mjs --write`). Never present a planning assumption
  as a statute (see `docs/COMPLIANCE.md`).
- **New corridors** are a new `JurisdictionRulepack` variant (id with a `YYYY.MM` suffix, `status()`, `rules()`),
  rule ids and citations in `compliance.rs`, an evaluation function, the identical web `INTL_REPORTS` entry, golden
  cases that pass and fail, and a row in `docs/COMPLIANCE.md`. Never touch `sim.rs` or `audit_bundle.rs` for this.
- **The web compliance block handles hostile input** (proof links, dropped dossiers): build DOM with `cwEl` /
  `textContent` only. `scripts/hygiene.sh` fails on any HTML sink there.
- **Do not loosen validation ranges** in `params.rs` without a cited reason in the PR.
- **Do not edit `.github/workflows`** to add secrets, `pull_request_target`, write permissions or
  unpinned actions.
- Every new source file starts with the SPDX header:
  `// SPDX-License-Identifier: AGPL-3.0-only` and `// Copyright (C) 2026 Swanand Tanavade`.
- Run `./scripts/verify.sh` before proposing a change and paste its last line in the PR.

## Style
- `cargo fmt` (see `rustfmt.toml`), `cargo clippy -- -D warnings` clean.
- Public items carry `///` docs that state units.
- Prefer clarity and parity with the reference over cleverness: the hot loop is only 8,760 iterations.
