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

step "lint"
cargo clippy --all-targets --locked -- -D warnings

step "unit, invariant, golden-parity and CLI tests"
cargo test --all-targets --locked
cargo test --doc --locked

if command -v node >/dev/null; then
  step "web reference model still matches tests/golden.json and tests/golden_v2.json"
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

step "all checks passed"
