#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Swanand Tanavade
#
# One-time publisher: creates the GitHub repository from YOUR machine, using
# YOUR browser login, and applies the security baseline in docs/REPO_HARDENING.md.
#
#   ./scripts/publish.sh                    # public repo "firmgap"
#   ./scripts/publish.sh --name my-firmgap  # different repo name
#   ./scripts/publish.sh --require-signed   # also require signed commits on main
#   ./scripts/publish.sh --skip-verify      # skip local cargo checks (not recommended)
#
# No token or password is ever typed into this script or stored by it:
# authentication is GitHub's own OAuth device flow via `gh auth login --web`.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO_NAME="firmgap"
REQUIRE_SIGNED=0
SKIP_VERIFY=0
while [ $# -gt 0 ]; do
  case "$1" in
    --name) REPO_NAME="$2"; shift 2 ;;
    --require-signed) REQUIRE_SIGNED=1; shift ;;
    --skip-verify) SKIP_VERIFY=1; shift ;;
    *) echo "unknown option $1"; exit 2 ;;
  esac
done
case "$REPO_NAME" in (*[!A-Za-z0-9._-]*|'') echo "invalid repo name"; exit 2 ;; esac

say()  { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m[warn] %s\033[0m\n' "$*"; }

for bin in git gh; do command -v "$bin" >/dev/null || { echo "missing '$bin' - see docs/PUBLISHING.md"; exit 1; }; done

say "1/7 GitHub login (browser device flow - nothing secret is typed here)"
if ! gh auth status >/dev/null 2>&1; then
  gh auth login --web --git-protocol https --hostname github.com
fi
OWNER="$(gh api user --jq .login)"
echo "logged in as: $OWNER"

say "2/7 personalise placeholders (OWNER -> $OWNER)"
for f in Cargo.toml README.md CITATION.cff SECURITY.md COMMERCIAL-LICENSE.md .github/CODEOWNERS .github/ISSUE_TEMPLATE/config.yml docs/*.md schemas/*.json; do
  [ -f "$f" ] && sed -i.bak "s#OWNER/firmgap#$OWNER/$REPO_NAME#g; s#@OWNER#@$OWNER#g" "$f" && rm -f "$f.bak"
done

if [ "$SKIP_VERIFY" -eq 0 ]; then
  say "3/7 local quality gate"
  ./scripts/verify.sh
else
  warn "skipping local verification; CI will be the first compiler to see this code"
fi

say "4/7 initial commit"
[ -d .git ] || git init -b main
git add -A
SIGN_FLAG=""
if [ -n "$(git config --get user.signingkey || true)" ]; then SIGN_FLAG="-S"; else warn "no signing key configured: commit will be unsigned (see docs/REPO_HARDENING.md#signed-commits)"; fi
git commit -s $SIGN_FLAG -m "firmgap v0.1.0: interim non-firm energization & curtailment economics simulator" || echo "nothing new to commit"

say "5/7 create public repository and push"
DESC="Interim non-firm grid energization & curtailment economics simulator for AI data centers: BESS sizing, workload throttling, SLA risk, interconnection-queue delay cost. Rust CLI + web."
if ! gh repo view "$OWNER/$REPO_NAME" >/dev/null 2>&1; then
  gh repo create "$OWNER/$REPO_NAME" --public --source . --remote origin --description "$DESC" --push
else
  warn "repository exists; pushing to it"
  git remote get-url origin >/dev/null 2>&1 || git remote add origin "https://github.com/$OWNER/$REPO_NAME.git"
  git push -u origin main
fi
R="repos/$OWNER/$REPO_NAME"

say "6/7 discoverability: topics"
gh repo edit "$OWNER/$REPO_NAME" \
  --add-topic data-center,ai-infrastructure,curtailment,demand-response,flexible-load \
  --add-topic interconnection-queue,battery-storage,bess,energy-storage,grid \
  --add-topic power-systems,energy-economics,simulation,rust,load-flexibility \
  --add-topic non-firm,lmp,techno-economic-analysis,datacenter-energy,cli

say "7/7 security baseline"
try() { "$@" >/dev/null 2>&1 || warn "could not apply: $* (set it manually, see docs/REPO_HARDENING.md)"; }
gh repo edit "$OWNER/$REPO_NAME" --enable-wiki=false --enable-projects=false --delete-branch-on-merge \
  --enable-merge-commit=false --enable-rebase-merge=false --enable-squash-merge --allow-update-branch || true
try gh api -X PUT "$R/vulnerability-alerts"
try gh api -X PUT "$R/automated-security-fixes"
try gh api -X PUT "$R/private-vulnerability-reporting"
printf '%s' '{"security_and_analysis":{"secret_scanning":{"status":"enabled"},"secret_scanning_push_protection":{"status":"enabled"}}}' \
  | gh api -X PATCH "$R" --input - >/dev/null 2>&1 || warn "secret scanning: enable in Settings > Code security"
try gh api -X PUT "$R/actions/permissions/workflow" -f default_workflow_permissions=read -F can_approve_pull_request_reviews=false
printf '%s' '{"enabled":true,"allowed_actions":"selected","sha_pinning_required":true}' \
  | gh api -X PUT "$R/actions/permissions" --input - >/dev/null 2>&1 \
  || printf '%s' '{"enabled":true,"allowed_actions":"selected"}' | gh api -X PUT "$R/actions/permissions" --input - >/dev/null 2>&1 \
  || warn "actions allow-list: set manually"
printf '%s' '{"github_owned_allowed":true,"verified_allowed":false,"patterns_allowed":["ossf/scorecard-action@*"]}' \
  | gh api -X PUT "$R/actions/permissions/selected-actions" --input - >/dev/null 2>&1 || warn "selected actions: set manually"
printf '%s' '{"approval_policy":"all_external_contributors"}' \
  | gh api -X PUT "$R/actions/permissions/fork-pr-contributor-approval" --input - >/dev/null 2>&1 || warn "fork PR approval: set manually"

SIGNED_RULE=""
[ "$REQUIRE_SIGNED" -eq 1 ] && SIGNED_RULE=',{"type":"required_signatures"}'
RULESET_FILE="$(mktemp)"; TAGS_FILE="$(mktemp)"
trap 'rm -f "$RULESET_FILE" "$TAGS_FILE"' EXIT
cat > "$RULESET_FILE" <<JSON
{
  "name": "protect-main",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    { "type": "pull_request", "parameters": {
        "required_approving_review_count": 0, "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false, "require_last_push_approval": false,
        "required_review_thread_resolution": true } },
    { "type": "required_status_checks", "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [
          { "context": "test (ubuntu-latest)" }, { "context": "test (macos-latest)" },
          { "context": "test (windows-latest)" }, { "context": "msrv" },
          { "context": "web-parity" }, { "context": "supply-chain" } ] } }
    $SIGNED_RULE
  ]
}
JSON
gh api -X POST "$R/rulesets" --input "$RULESET_FILE" >/dev/null 2>&1 || warn "branch ruleset: create manually from docs/REPO_HARDENING.md"
cat > "$TAGS_FILE" <<JSON
{ "name": "protect-release-tags", "target": "tag", "enforcement": "active",
  "conditions": { "ref_name": { "include": ["refs/tags/v*"], "exclude": [] } },
  "rules": [ { "type": "deletion" }, { "type": "non_fast_forward" }, { "type": "update" } ] }
JSON
gh api -X POST "$R/rulesets" --input "$TAGS_FILE" >/dev/null 2>&1 || warn "tag ruleset: create manually"

say "done: https://github.com/$OWNER/$REPO_NAME"
cat <<EOF
Remaining manual steps (cannot be automated safely):
  1. Account security: enable 2FA with a passkey or security key (github.com/settings/security).
  2. Watch the first CI run: https://github.com/$OWNER/$REPO_NAME/actions
  3. Settings > Code security: confirm CodeQL default setup is OFF (the repo ships its own workflow).
  4. Put a real contact address in SECURITY.md and COMMERCIAL-LICENSE.md.
  5. Brand protection: see TRADEMARKS.md (trademark search + filing for "firmgap").
EOF
