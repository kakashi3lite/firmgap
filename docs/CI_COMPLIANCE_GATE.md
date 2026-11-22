# Compliance-as-code CI gate

`.github/workflows/firmgap-compliance-gate.yml` re-audits scenarios on every pull request that changes a
scenario file or the engine. It writes the verdicts and SHA-256 digests to the job summary and fails the
check when a campus would hit an undeclared statutory blocker.

## What it does

1. Builds `firmgap` in release mode from the PR head (no third-party actions except the SHA-pinned
   `actions/checkout`).
2. Lists the changed `scenarios/*.toml` files (added, copied, modified or renamed). If no scenario changed
   (an engine-only PR), it re-audits every scenario that declares a `jurisdiction`, so a code change cannot
   silently flip a fixture.
3. For each file:
   * if it declares `jurisdiction = "<id>"`, it runs
     `firmgap audit --scenario <file> --format markdown --fail-on-blocker` and appends the report to
     `$GITHUB_STEP_SUMMARY`;
   * otherwise it appends the informational `compare-jurisdictions` matrix (never fails).
4. Fails the job if any audit exits non-zero.

## Exit codes the gate relies on

| Code | Meaning |
|---|---|
| 0 | Pass, or the verdict matches the file's `expected_verdict` |
| 1 | Dossier integrity failure (only with `--verify` / `--share`) |
| 2 | Usage or input error (bad key, out-of-range value, unreadable file) |
| 3 | Gate failure: an undeclared `StatutoryBlocker`, or a verdict that differs from `expected_verdict` |

## Declaring intent in a scenario

```toml
jurisdiction = "ercot-sb6"
expected_verdict = "BankablePass"        # the gate fails if this campus stops passing
```

For deliberate failing fixtures (demos, regression tests), declare the blocker:

```toml
jurisdiction = "ercot-sb6"
expected_verdict = "StatutoryBlocker"   # the gate passes only while the blocker is reproduced
```

Both keys are metadata. They are validated, but they are not simulation inputs and change no hash.

## Running the same gate locally

```bash
firmgap audit --scenario scenarios/my-site.toml --format markdown --fail-on-blocker; echo "exit $?"
firmgap batch-audit --dir scenarios --jurisdiction declared --fail-on-blocker --format markdown
```

`batch-audit --jurisdiction declared --fail-on-blocker` also fails (exit 3) on files that cannot be audited,
including files with no `jurisdiction` metadata, so use it on directories where every file declares one.

## Integrity of the gate itself

| Risk | Control |
|---|---|
| A fork PR exfiltrates secrets or pushes code | The workflow runs on `pull_request` only, with `permissions: contents: read`, no secrets and `persist-credentials: false` |
| Script injection through PR metadata | Only commit SHAs are passed in, through environment variables. No PR title, body or branch name reaches `run:` |
| Hostile file names | Names must match `scenarios/[A-Za-z0-9._-]+.toml`; anything else fails the job. All variables are quoted |
| A compromised action | The only action is `actions/checkout`, pinned to a full commit SHA; Dependabot proposes updates |
| Someone weakens the gate in the same PR | The workflow file is itself a gated path. Require the check in the `protect-main` ruleset (see [REPO_HARDENING.md](REPO_HARDENING.md)) and require CODEOWNERS review for `.github/` |
| Hostile scenario content | The same parser as the CLI: 64 KiB / 512-line caps, key allowlist, finiteness and range checks, fuzzed weekly |

## Making it a required check

In **Settings › Rules › protect-main**, add `compliance-gate` to the required status checks. Because the
workflow only runs when its paths change, also keep `ci` required. GitHub treats a required check that did
not run as passing only when the ruleset allows it, so test this once with a scenario-only PR.
