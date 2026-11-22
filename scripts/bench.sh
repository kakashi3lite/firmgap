#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 Swanand Tanavade
#
# Release-mode performance gate for the compliance tools.
#   single-site audit                         < 10 ms   (in-process wall time, --timings)
#   100-site batch-audit, all 8 rulepacks     < 1000 ms
# Timings are measured inside the binary (std::time::Instant), so process start-up is excluded.
# Override the limits with FIRMGAP_MAX_AUDIT_MS / FIRMGAP_MAX_BATCH_MS on slow machines.
set -euo pipefail
cd "$(dirname "$0")/.."

max_audit="${FIRMGAP_MAX_AUDIT_MS:-10}"
max_batch="${FIRMGAP_MAX_BATCH_MS:-1000}"
bin="./target/release/firmgap"
[ -x "$bin" ] || [ -x "$bin.exe" ] || cargo build --release --locked --quiet

ms() { sed -n 's/^firmgap: elapsed \([0-9.]*\) ms$/\1/p' "$1" | tail -n 1; }
over() { awk -v a="$1" -v b="$2" 'BEGIN { exit !(a > b) }'; }

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# warm-up, then the best of 5 single-site audits
best=""
for _ in 1 2 3 4 5 6; do
  "$bin" audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --format json --timings > /dev/null 2> "$tmp/t.txt"
  t="$(ms "$tmp/t.txt")"
  if [ -z "$best" ] || over "$best" "$t"; then best="$t"; fi
done
echo "single-site audit: ${best} ms (limit ${max_audit} ms)"

# 100 sites: the bundled scenarios cycled under distinct names
mkdir -p "$tmp/portfolio"
i=0
while [ "$i" -lt 100 ]; do
  for f in scenarios/*.toml; do
    [ "$i" -lt 100 ] || break
    cp "$f" "$tmp/portfolio/site-$(printf '%03d' "$i").toml"
    i=$((i + 1))
  done
done
"$bin" batch-audit --dir "$tmp/portfolio" --jurisdiction all --format json --timings > "$tmp/batch.json" 2> "$tmp/b.txt"
bt="$(ms "$tmp/b.txt")"
echo "100-site batch-audit x 8 rulepacks: ${bt} ms (limit ${max_batch} ms)"

# determinism: a second sweep must be byte-identical
"$bin" batch-audit --dir "$tmp/portfolio" --jurisdiction all --format json > "$tmp/batch2.json"
cmp -s "$tmp/batch.json" "$tmp/batch2.json" || { echo "batch output is not deterministic"; exit 1; }
echo "batch output deterministic across runs"

fail=0
if over "$best" "$max_audit"; then echo "FAIL: single-site audit slower than ${max_audit} ms"; fail=1; fi
if over "$bt" "$max_batch"; then echo "FAIL: 100-site batch slower than ${max_batch} ms"; fail=1; fi
exit "$fail"
