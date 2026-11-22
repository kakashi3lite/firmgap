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
| T2a | Tampering of an audit dossier | Edit a number in `dossier.json` before sending it to an investor or utility | SHA-256 over canonical JSON; `firmgap audit --verify` exits 1; single-byte tampering test; `--expect <sha256>` pins a hash recorded out of band | **The embedded hash is not a signature**: an attacker who edits the file can recompute it. Owner: record the hash out of band or sign the file (cosign / minisign) |
| T2b | DoS via hostile dossier | Huge, deeply nested, duplicate-key or invalid-escape JSON passed to `--verify` or the web drop zone | CLI: 4 MiB cap, depth 64, duplicate keys rejected, strict RFC 8259 parser, linear-time string decoding, `dossier` fuzz target. Web: files over 1 MB refused before reading; results rendered with `textContent` | Low |
| T2c | Misleading compliance claims | A planning assumption presented as law, or a rule that is stale | Every check carries its citation and legal basis; rulepack ids are versioned; `docs/COMPLIANCE.md` lists what is anticipated vs. adopted | Medium: rules change. Owner: re-verify before each IC use |
| T2d | Hostile proof link (`#s=`) | A crafted link that injects markup, pollutes prototypes, exhausts memory or misrepresents a campus | Token capped at 16 KiB before decoding; base64url charset check; printable-ASCII payload; allowlisted top-level, parameter and site keys (decoded into `Object.create(null)`); typed values; path charset `[A-Za-z0-9._/-]` without `..`; browser clamps to input ranges, CLI rejects out-of-range values; the compliance UI builds DOM with `textContent` only (`scripts/hygiene.sh` fails on any HTML sink in that block); the embedded `h` must be reproduced or the link is flagged | Low. A link proves the *inputs* reproduce a hash, not who sent it |
| T2e | Batch-audit DoS | A directory with millions of files, huge files, symlink loops or hostile names | Non-recursive; regular files only (symlinks skipped); names `[A-Za-z0-9._-]{1,120}.toml`; at most 1,000 files; each file capped at 64 KiB / 512 lines; one simulation per site; bounded output | Low |
| T2f | Browser network exfiltration or third-party script | A future edit adds a CDN, fetch or beacon | CSP `default-src 'none'` (no connect, frame, font or object sources), `base-uri 'none'`, `form-action 'none'`; no external URLs in the page (checked in review) | Low |
| T8 | CI compliance-gate bypass | A PR edits the gate, injects shell through file names or PR metadata, or a fork steals secrets | `pull_request` event only, `contents: read`, no secrets, `persist-credentials: false`, SHA-pinned checkout, only commit SHAs passed via env, file names validated, all variables quoted, the workflow path is itself gated, CODEOWNERS on `.github/` | Medium until `compliance-gate` is a required check. Owner: add it to `protect-main` |
| T9 | Release artifact substitution | A tampered binary or web bundle on the release page | Tag-triggered build on GitHub runners, `SHA256SUMS` published with the assets, release job alone gets `contents: write`, Pages job alone gets `pages: write` + `id-token: write` | Medium. Owner: sign tags and consider Sigstore attestations (`actions/attest-build-provenance`, SHA-pinned) |
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
