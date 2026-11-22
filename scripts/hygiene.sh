#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Swanand Tanavade
#
# Repository hygiene gate (run by verify.sh and CI):
#   1. no TODO / FIXME markers in shipped code, docs or workflows;
#   2. no innerHTML in the web Compliance & Bankability block, which handles hostile input
#      (proof links and dropped dossiers) and must build DOM with textContent only;
#   3. every third-party GitHub Action pinned to a full 40-character commit SHA;
#   4. every workflow declares top-level `permissions:` and none uses pull_request_target.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0

if grep -RInwE '(TODO|FIXME)' --exclude-dir=target --exclude-dir=.git --exclude=hygiene.sh src tests web docs scripts .github README.md CHANGELOG.md AGENTS.md; then
  echo "hygiene: TODO/FIXME markers found"; fail=1
fi

block="$(awk '/Compliance & Bankability war room \(model 0\.4\)/{on=1} /^const restored = restore\(\);/{on=0} on' web/index.html)"
if [ -z "$block" ]; then echo "hygiene: compliance UI block not found in web/index.html"; fail=1; fi
if printf '%s\n' "$block" | grep -nE '\.innerHTML|outerHTML|insertAdjacentHTML|document\.write'; then
  echo "hygiene: HTML sinks found in the hostile-input compliance block"; fail=1
fi

if grep -RhnE '^[[:space:]]*(-[[:space:]]*)?uses:[[:space:]]*[^.][^@[:space:]]*@' .github/workflows | grep -vE '@[0-9a-f]{40}([[:space:]]|$)'; then
  echo "hygiene: GitHub Actions not pinned to a full commit SHA"; fail=1
fi

for wf in .github/workflows/*.yml; do
  grep -qE '^permissions:' "$wf" || { echo "hygiene: $wf has no top-level permissions"; fail=1; }
  if grep -q 'pull_request_target' "$wf"; then echo "hygiene: $wf uses pull_request_target"; fail=1; fi
done

[ "$fail" -eq 0 ] && echo "hygiene: OK (no TODO/FIXME, no HTML sinks in the compliance block, all actions SHA-pinned, permissions declared)"
exit "$fail"
