# Repository hardening checklist

`scripts/publish.sh` applies items marked ⚙ automatically. Verify each item in the GitHub UI after
the first push, and do the ✋ items yourself.

## Account (most important)
- ✋ **Two-factor authentication with a passkey or hardware security key**. Avoid SMS. Store recovery
  codes offline. Go to Settings › Password and authentication.
- ✋ Use a dedicated, verified email for commits. Turn on *Keep my email address private*.
- ✋ **Never paste a token or password into any chat, AI assistant, issue or script.** `publish.sh`
  uses `gh auth login --web`, GitHub's own device-flow login.
- ✋ Review authorized OAuth apps and SSH keys every quarter. Remove anything unknown.

## Signed commits
- ✋ Set up an SSH signing key: `git config --global gpg.format ssh`,
  `git config --global user.signingkey ~/.ssh/id_ed25519.pub`,
  `git config --global commit.gpgsign true`, then add the key in GitHub as a **Signing key**.
- ✋ Enable **vigilant mode** (Settings › SSH and GPG keys).
- ⚙ With `--require-signed`, the `main` ruleset rejects unsigned commits.

## Repository settings
- ⚙ Rulesets on `main`:
  * no deletion;
  * no force-push;
  * linear history;
  * PRs required, with conversation resolution;
  * required checks `test (ubuntu-latest|macos-latest|windows-latest)`, `msrv`, `web-parity`,
    `supply-chain`, up to date before merge.
- ⚙ Ruleset on tags `v*`: no deletion, update or force-push.
- ⚙ Squash-merge only; delete the branch on merge; wiki and projects off.
- ⚙ Dependabot alerts and security updates; ✋ check that Dependabot version updates run (`.github/dependabot.yml`).
- ⚙ Secret scanning + **push protection**.
- ⚙ Private vulnerability reporting.
- ✋ Code scanning: the repo ships `codeql.yml` (advanced setup). If GitHub enabled *default setup*
  automatically, turn it off to avoid duplicate runs.

## GitHub Actions
- ⚙ Default `GITHUB_TOKEN` permission **read**; Actions can't approve PRs.
- ⚙ Allowed actions: GitHub-owned plus `ossf/scorecard-action` only, with SHA pinning required where
  supported.
- ⚙ Fork PR workflows need approval for all external contributors.
- ✋ Never add `pull_request_target`, `workflow_run` with checkout of PR code, or repository secrets
  to CI. This project needs none.

## Releases (when you publish binaries)
- Tag from a signed commit: `git tag -s v0.1.0 -m "v0.1.0"`.
- Add a release workflow that builds with `--locked` and attaches GitHub build-provenance
  attestations (pin `actions/attest-build-provenance` by SHA), plus SHA-256 checksums.
- Reserve the name on crates.io (`cargo publish`) with a crates.io token scoped to
  *publish-new / publish-update for firmgap*, short expiry. Use it once from your own terminal. Never
  store it in the repo.

## Ongoing
- Merge Dependabot PRs only after CI passes and you've skimmed the upstream release notes.
- Watch the OpenSSF Scorecard result (badge in README) and fix new findings.
- Rotate any credential that might have leaked, then check *Security › Audit log*.

## v0.4 additions

* **Required checks:** add `compliance-gate` (from `firmgap-compliance-gate.yml`) next to `ci` in the `protect-main` ruleset.
* **Pages:** Settings › Pages › Source: *GitHub Actions*. The `github-pages` environment is created on the first
  release; restrict it to the `v*` tags (Settings › Environments › github-pages › Deployment branches and tags).
* **Releases:** only tag pushes run `release.yml`; protect `v*` tags with the `protect-release-tags` ruleset so only
  maintainers can create them.
