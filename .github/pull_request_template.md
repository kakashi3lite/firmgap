## What and why

## Checklist
- [ ] `./scripts/verify.sh` passes locally
- [ ] Commits are signed off (`git commit -s`, DCO) and, if possible, signed (`-S`)
- [ ] No new runtime dependency (or justified in the PR and approved in `deny.toml`)
- [ ] Model change? Web model + Rust kernel updated together, `node scripts/golden.mjs --write`, `tests/golden.rs` updated, CHANGELOG entry
- [ ] Docs updated (`docs/CONTEXT_MAP.md` if a concept moved)
- [ ] No workflow change that widens `permissions:` or adds an unpinned action
