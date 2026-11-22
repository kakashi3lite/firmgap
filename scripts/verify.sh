#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Swanand Tanavade
#
# Local quality gate: the same checks CI runs. Run before every push.
#   ./scripts/verify.sh
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n\033[1;36m==> %s\033[0m\n' "$*"; }

command -v cargo >/dev/null || { echo "Rust is required: https://rustup.rs"; exit 1; }

step "license text present"
grep -q "GNU AFFERO GENERAL PUBLIC LICENSE" LICENSE
grep -q "END OF TERMS AND CONDITIONS" LICENSE

step "format (rewrites files in place)"
cargo fmt --all

step "format check (CI runs cargo fmt --check)"
cargo fmt --all --check

step "lint"
cargo clippy --all-targets --locked -- -D warnings

step "unit, invariant, golden-parity and CLI tests"
cargo test --all-targets --locked
cargo test --doc --locked

if command -v node >/dev/null; then
  step "web reference model matches tests/golden.json, golden_v2.json and golden_audit.json (dossier, matrix and proof-link parity)"
  node scripts/golden.mjs --check
else
  echo "node not found: skipping web parity check (CI will run it)"
fi

step "no unsafe, no network, no process spawning in shipped code"
if grep -RInE 'unsafe[[:space:]]*\{|std::net|TcpStream|UdpSocket|std::process::Command' src/; then
  echo "forbidden construct found in src/"; exit 1
fi

step "secrets scan (best effort)"
if grep -RInE '(ghp_|github_pat_|AKIA[0-9A-Z]{16}|-----BEGIN (RSA|OPENSSH|EC) PRIVATE KEY-----)' --exclude-dir=target --exclude-dir=.git --exclude=verify.sh . ; then
  echo "possible secret committed"; exit 1
fi

step "CLI smoke: investment-committee report"
cargo run --quiet --release --locked -- diligence --world base --design robust -n 4 | tail -n 3

step "release build"
cargo build --release --locked

step "CLI smoke from /tmp: audit, verify (valid + 1-byte tamper), compare-jurisdictions, batch-audit"
bin="$PWD/target/release/firmgap"
repo="$PWD"
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
(
  cd "$tmp"
  cp -R "$repo/scenarios" ./scenarios
  "$bin" audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --jurisdiction ercot-sb6 --out dossier.json | grep -E 'VERDICT|dossier_sha256'
  "$bin" audit --verify dossier.json
  # flip exactly one byte in the middle of the file
  size=$(wc -c < dossier.json); half=$((size / 2))
  head -c "$half" dossier.json > tampered.json
  orig=$(dd if=dossier.json bs=1 skip="$half" count=1 2>/dev/null)
  if [ "$orig" = "1" ]; then printf '2' >> tampered.json; else printf '1' >> tampered.json; fi
  tail -c +"$((half + 2))" dossier.json >> tampered.json
  if "$bin" audit --verify tampered.json 2>/dev/null; then echo "1-byte tampered dossier was accepted"; exit 1; fi
  echo "1-byte tampered dossier rejected (non-zero exit) as expected"
  "$bin" audit --scenario scenarios/ercot-sb6-illegal-stack.toml --format markdown --fail-on-blocker | grep -q 'Verdict: StatutoryBlocker'
  "$bin" compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml --format markdown | head -n 14
  "$bin" batch-audit --dir scenarios --jurisdiction all --format markdown | tail -n 3
)

step "performance gate (release): single audit < 10 ms, 100-site sweep < 1 s"
./scripts/bench.sh

step "hygiene: to-do markers, HTML sinks in hostile-input UI, pinned actions, workflow permissions"
./scripts/hygiene.sh

step "all checks passed"
