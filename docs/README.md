# Documentation index

Start with the [README](../README.md). Then pick by role:

| If you are… | Read |
|---|---|
| An investor or IC member | [INVESTOR_DILIGENCE.md](INVESTOR_DILIGENCE.md), then [COMPLIANCE.md](COMPLIANCE.md) |
| General counsel or regulatory | [COMPLIANCE.md](COMPLIANCE.md), [JURISDICTIONS_2026.md](JURISDICTIONS_2026.md) |
| A grid or interconnection engineer | [MODEL.md](MODEL.md), [COMPLIANCE.md](COMPLIANCE.md) §3 (shedding curve) |
| A user of the CLI or web workbench | [USER_GUIDE.md](USER_GUIDE.md), [GLOSSARY.md](GLOSSARY.md) |
| Running the CI compliance gate | [CI_COMPLIANCE_GATE.md](CI_COMPLIANCE_GATE.md) |
| A contributor or AI coding agent | [../AGENTS.md](../AGENTS.md), [CONTEXT_MAP.md](CONTEXT_MAP.md), [ARCHITECTURE.md](ARCHITECTURE.md) |
| Security or an auditor | [THREAT_MODEL.md](THREAT_MODEL.md), [REPO_HARDENING.md](REPO_HARDENING.md), [VERIFICATION.md](VERIFICATION.md), [../SECURITY.md](../SECURITY.md) |
| The maintainer, publishing a release | [PUBLISHING.md](PUBLISHING.md) |
| Planning the production system | [PRODUCTION_SPEC.md](PRODUCTION_SPEC.md) |

| Document | Contents |
|---|---|
| [MODEL.md](MODEL.md) | Authoritative equations: synthetic year, dispatch, economics, investor view, Monte Carlo, compliance (§8) |
| [COMPLIANCE.md](COMPLIANCE.md) | Eight rulepacks, each check's citation and legal basis, the remediation solver, the shedding curve, dossier integrity, matrix, batch and proof links |
| [USER_GUIDE.md](USER_GUIDE.md) | War-room tour, CLI (`audit`, `compare-jurisdictions`, `batch-audit`, `--share`), scenario keys, exit codes, troubleshooting |
| [JURISDICTIONS_2026.md](JURISDICTIONS_2026.md) | Sources and implementation choices for the Virginia, GB, Ireland, Spain and Singapore rulepacks |
| [CI_COMPLIANCE_GATE.md](CI_COMPLIANCE_GATE.md) | The pull-request gate: what it runs, exit codes, scenario metadata, integrity controls |
| [INVESTOR_DILIGENCE.md](INVESTOR_DILIGENCE.md) | Corrections to v0.1, results, break-evens, prototype-to-production gap list |
| [VERIFICATION.md](VERIFICATION.md) | What was checked, how, against which sources, and what is still unverified |
| [ARCHITECTURE.md](ARCHITECTURE.md) | Module graph and data flow |
| [CONTEXT_MAP.md](CONTEXT_MAP.md) | Concept → code → test → doc, plus the decision log |
| [THREAT_MODEL.md](THREAT_MODEL.md) | STRIDE threats, mitigations and residual risk |
| [REPO_HARDENING.md](REPO_HARDENING.md) | GitHub settings, rulesets, releases |
| [PRODUCTION_SPEC.md](PRODUCTION_SPEC.md) | Target production architecture, data contracts, operations |
| [PUBLISHING.md](PUBLISHING.md) | First push and shipping updates through a PR |
| [GLOSSARY.md](GLOSSARY.md) | Terms |

Schemas live in [`../schemas/`](../schemas/). Example scenarios live in [`../scenarios/`](../scenarios/).
