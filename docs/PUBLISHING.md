# Publishing guide (first push to GitHub)

About 15 minutes, done once, on **your own computer**. You never share a password or token with
anyone, including AI assistants. Login happens in your browser on github.com.

## 0. Prerequisites

| Tool | Install | Check |
|---|---|---|
| Git | https://git-scm.com/downloads (Windows: includes *Git Bash*; use it for the commands below) | `git --version` |
| GitHub CLI | https://cli.github.com | `gh --version` |
| Rust (recommended) | https://rustup.rs | `cargo --version` |
| Node.js (optional, web parity check) | https://nodejs.org | `node --version` |

Set your commit identity once:
```bash
git config --global user.name  "Swanand Tanavade"
git config --global user.email "<your GitHub noreply or verified email>"
```

## 1. Unpack and verify
```bash
unzip firmgap.zip && cd firmgap
chmod +x scripts/*.sh
./scripts/verify.sh        # formats, lints, runs all tests; fix anything red before pushing
```
If `verify.sh` reports a compile error, paste the error (not your credentials) into the chat to get
a fix.

## 2. Publish
```bash
./scripts/publish.sh                     # or: ./scripts/publish.sh --require-signed
```
What happens:
1. `gh auth login --web` shows a one-time code and opens github.com. Approve it there.
2. The `OWNER` placeholders are replaced with your username.
3. The local quality gate runs.
4. The initial commit is created, signed off (DCO), and signed if you configured a key.
5. `gh repo create <you>/firmgap --public --push` runs.
6. 20 discoverability topics are added.
7. The security baseline from [REPO_HARDENING.md](REPO_HARDENING.md) is applied. Anything that
   couldn't be applied is printed as a warning for you to set by hand.

## 3. After the first push
1. Open **Actions** and wait for `ci`, `codeql` and `scorecard` to go green.
2. **Settings › Rules** should show `protect-main` and `protect-release-tags`.
3. **Settings › Code security** should show secret scanning, push protection, Dependabot and private
   reporting all *enabled*.
4. Contact addresses are already filled in (`SECURITY.md`, `COMMERCIAL-LICENSE.md`). Once the
   `protect-main` ruleset is active, changes to `main` go through a PR:
   ```bash
   git switch -c docs/contact && git commit -am "docs: add contact" -s && git push -u origin HEAD
   gh pr create --fill && gh pr merge --squash --auto
   ```
5. Optional: `cargo publish --dry-run`, then `cargo publish` to reserve the crate name (see
   REPO_HARDENING › Releases).

## Manual alternative (without the script)
```bash
gh auth login --web
git init -b main && git add -A && git commit -s -m "firmgap v0.2.0"
gh repo create firmgap --public --source . --remote origin --push
```
Then apply [REPO_HARDENING.md](REPO_HARDENING.md) in **Settings**.
