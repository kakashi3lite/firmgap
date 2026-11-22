# Threat model (red-team view)

**Assets:** the integrity of the model's numbers (decisions worth tens of millions of dollars rely
on them), the source repository, release artifacts, the maintainer's GitHub account, and the
project's name and license.

**Adversaries:**
* opportunistic attackers (secret scrapers, typosquatters);
* malicious contributors (poisoned PRs);
* supply-chain attackers (compromised actions or crates);
* targeted attackers aiming to alter results;
* commercial free-riders (license or brand abuse).

| # | Threat (STRIDE) | Attack | Mitigation in this repo | Residual risk / owner action |
|---|---|---|---|---|
| T1 | Tampering / DoS via input | Hostile scenario file or `--set`: huge file, NaN/inf, out-of-range, unknown keys, deep numbers, non-UTF-8 args | 64 KiB / 512-line / 512-char caps; key whitelist; finiteness and range checks; cross-field validation; bounded loops in year generation; `args_os` UTF-8 check; weekly fuzzing | Low |
| T2 | Tampering of output / log injection | Terminal escape codes or JSON breakout via echoed input | `Error` Display sanitizes every user string to printable ASCII; JSON writer escapes all control chars and U+2028/2029 | Low |
| T3 | Elevation via the binary | Code execution, file writes, SSRF | No `unsafe`, no network, no process spawning, no env vars, single read-only file open; `verify.sh` greps for forbidden APIs | Low |
| T4 | Supply chain: crates | Malicious or typosquatted dependency | **Zero runtime deps**, enforced by the CI guard; `cargo-deny` (sources, licenses, bans), `cargo-audit`, committed `Cargo.lock`, `--locked` builds | Fuzz crate uses `libfuzzer-sys` (dev-only, never shipped) |
| T5 | Supply chain: CI | Compromised or retagged GitHub Action (tj-actions-style) | Every action **pinned to a full commit SHA**; allow-list limited to GitHub-owned + `ossf/scorecard-action`; SHA-pinning policy requested at repo level; Dependabot for updates | Review Dependabot action bumps by diffing the SHA's release notes |
| T6 | CI token abuse | PR from a fork exfiltrates `GITHUB_TOKEN` or writes to the repo | Top-level `permissions: contents: read`; `persist-credentials: false`; no `pull_request_target`; no secrets used in CI; fork PR workflows need approval | Never add `pull_request_target` or secrets to PR workflows |
| T7 | Malicious PR | Subtle model change that biases results (e.g., halves the penalty) | Golden parity tests on both implementations; invariant tests; CODEOWNERS review; required status checks; PR template forces evidence for model changes | Review every diff of `tests/golden.*` with the CHANGELOG entry |
| T8 | Account takeover | Phished password or stolen token pushes a backdoor or deletes the repo | Owner action: 2FA with passkey or security key; no classic PATs; fine-grained tokens with expiry only if ever needed; rulesets block force-push and deletion of `main` and `v*` tags | Highest residual risk: protect the account |
| T9 | Spoofing of commits | Commits forged with the maintainer's email | Signed commits (SSH or GPG) and optional `required_signatures` rule (`publish.sh --require-signed`); DCO sign-off | Enable vigilant mode in GitHub settings |
| T10 | Secret leakage | Token committed by mistake | `.gitignore` for keys and `.env`; secret scanning + push protection enabled by `publish.sh`; `verify.sh` pattern scan | Rotate immediately if a secret is ever pushed |
| T11 | Vulnerability disclosure in public | Exploit posted as an issue | Private vulnerability reporting enabled; issue template points to it; SECURITY.md SLA | – |
| T12 | Repudiation / provenance | "That binary isn't from you" | Reproducible `--locked` builds, version in output, OpenSSF Scorecard | Next step: release workflow with build-provenance attestations |
| T13 | License abuse | Closed SaaS fork of the model | AGPL-3.0-only (network-use clause); SPDX header in every file; NOTICE; commercial license offer | Enforcement is a legal matter; keep authorship records (signed commits) |
| T14 | Brand abuse / name squatting | Someone registers "firmgap" crate, org or domain, or claims the mark | TRADEMARKS.md policy | Owner action: reserve the crates.io name, GitHub org and domain, and run a trademark search and filing |

## Explicitly out of scope
* Correctness of real-world inputs (tariffs, permits) supplied by the user.
* Using firmgap as a real-time controller. It is a planning model.
