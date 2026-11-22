# Contributing

Thanks for helping improve firmgap.

## Ground rules
- Read `AGENTS.md` and `docs/CONTEXT_MAP.md` first.
- Open an issue before large changes; model changes need evidence (tariff, ISO rule, paper, field data).
- All checks in `./scripts/verify.sh` must pass.

## Developer Certificate of Origin (DCO)
Every commit must be signed off: `git commit -s`. This certifies the DCO 1.1
(https://developercertificate.org): you wrote the change or have the right to submit it under the
project license.

## Contributor license grant (needed for dual licensing)
firmgap is offered under AGPL-3.0-only **and** commercial licenses (`COMMERCIAL-LICENSE.md`). To
keep that possible, by submitting a contribution you also grant Swanand Tanavade a perpetual,
worldwide, non-exclusive, royalty-free, irrevocable license to use, modify, sublicense and
distribute your contribution under any license, including proprietary ones. You keep your
copyright. If you can't agree, say so in the PR before it's merged.
*(Maintainer: have a lawyer review this clause, or replace it with a formal CLA, before accepting
substantial outside contributions.)*

## Regenerating golden values after an intentional model change
```bash
node scripts/golden.mjs --write
python3 - <<'PY'
import json; g = json.load(open('tests/golden.json'))
print(json.dumps(g['scenarios']['right_sized']['econ'], indent=1))  # copy changed literals into tests/golden.rs
PY
```

## Commit style
`area: imperative summary` (e.g. `sim: cap genset output at permit hours`). Squash-merge only.
