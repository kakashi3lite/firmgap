# firmgap — compliance and bankability engine for AI data-center grid connections

[![ci](https://github.com/kakashi3lite/firmgap/actions/workflows/ci.yml/badge.svg)](https://github.com/kakashi3lite/firmgap/actions/workflows/ci.yml)
[![codeql](https://github.com/kakashi3lite/firmgap/actions/workflows/codeql.yml/badge.svg)](https://github.com/kakashi3lite/firmgap/actions/workflows/codeql.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/kakashi3lite/firmgap/badge)](https://scorecard.dev/viewer/?uri=github.com/kakashi3lite/firmgap)
[![license: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)
![runtime dependencies: 0](https://img.shields.io/badge/runtime%20deps-0-brightgreen.svg)
![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-brightgreen.svg)

**firmgap tells an AI data-center developer, lender or utility, in one command, whether a campus can
connect to the grid early, under which law it is bankable, and what the cheapest legal fix is — with a
SHA-256-stamped dossier anyone can re-verify.**

It is an hour-by-hour (8,760 h) simulator of **interim non-firm interconnection**: BESS sizing,
deferrable GPU workloads, curtailment ceilings, SLA risk and interconnection-queue carry. On top sit
**eight versioned regulatory rulepacks** with citations, a **counterfactual path-to-compliance
solver**, a **certified shedding-capability curve**, and a **tamper-evident audit dossier** that is
byte-identical between the Rust CLI and the offline web workbench.

## 30-second quickstart

```bash
cargo install --git https://github.com/kakashi3lite/firmgap --locked     # Rust >= 1.74; zero dependencies

firmgap audit --scenario scenarios/ercot-sb6-illegal-stack.toml            # StatutoryBlocker + Path A vs Path B
firmgap compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml --format markdown
firmgap batch-audit --dir scenarios --jurisdiction all --format markdown   # portfolio sweep
firmgap audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --out dossier.json && firmgap audit --verify dossier.json
```

No Rust? Open `web/index.html` (or the GitHub Pages copy published by each release) and click
**Compliance & Bankability**. It runs offline, loads no third-party code and produces the same dossier
bytes as the CLI.

## What you get

| Capability | CLI | Web workbench |
|---|---|---|
| Eight rulepacks: ERCOT / Texas SB 6, FERC RM26-4 (anticipated), Virginia SB 253, GB NESO / Ofgem (proposed), Ireland CRU/2025/236, Spain RD 1032/2026, Singapore GDCR / DC-CFA2, EU EED Art. 12 | `audit --jurisdiction <id>`, `rulepacks` | Jurisdiction bar + "More corridors" |
| Path-to-compliance solver (Path A vs B, minimum BESS vs SLA slack, permit-hour BESS, Tier IV, de-rated storage for Irish MIC, hourly renewables) | `audit` report | Remediation cards with **Apply remediation** |
| Certified shedding-capability curve (15 min to 4 h) and BESS CapEx it displaces | `audit` report | Live SVG with sliders and a takeaway callout |
| Global bankability matrix (one campus, every corridor, ranked) | `compare-jurisdictions` | Global site comparison |
| Portfolio sweeps, deterministic and bounded | `batch-audit --dir` | — |
| Tamper-evident dossier (pure-std FIPS 180-4 SHA-256) | `audit --out`, `--verify`, `--expect` | Download + verify drop zone (WebCrypto cross-check) |
| Shareable proof link `#s=<base64url>` that reproduces the sender's hash | `audit --share <url>` | **Copy proof link** |
| Compliance-as-code CI gate | `--fail-on-blocker`, `--format markdown` | — |
| Filing memo | — | **Print filing memo** (print stylesheet) |
| Role lens: CFO, General Counsel, Grid Engineer | — | Every takeaway rephrased; numbers unchanged |

### Example: one 100 MW campus, eight corridors

`firmgap compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml` (golden-tested):

| Rank | Corridor | Verdict | Why |
|---:|---|---|---|
| 1 | FERC RM26-4 (anticipated) | BankablePass | POI, ramp, power factor and staged energization all within terms |
| 2 | EU EED Art. 12 | BankablePass | Reporting-ready; PUE meets the contractual target |
| 3 | Texas SB 6 | BankablePass | $5.00M security posted on gross MW; disconnect test survived |
| 4 | Singapore GDCR | ConditionalWarning | PUE 1.30 misses the 1.25 bar reported for capacity awards |
| 5 | GB NESO / Ofgem | ConditionalWarning | End-user and long-lead milestones open (proposed rules) |
| 6 | Virginia SB 253 | StatutoryBlocker | Backup is not Tier IV (HB 507 air permits) |
| 7 | Ireland CRU | StatutoryBlocker | De-rated dispatchable capacity 57.4 MW vs 102.0 MW MIC |
| 8 | Spain RD 1032/2026 | StatutoryBlocker | PUE 1.30 > 1.25; 60% hourly renewables < 80% |

> Screening tool, not legal advice. Each check names its provision and its legal status (statute,
> adopted rule, approved protocol, proposed or anticipated term, or planning assumption). Sources were
> checked on 22 Nov 2026: [docs/COMPLIANCE.md](docs/COMPLIANCE.md), [docs/JURISDICTIONS_2026.md](docs/JURISDICTIONS_2026.md).

## Open core and commercial

| | AGPL-3.0 (free) | Commercial license |
|---|---|---|
| Rust engine, CLI, eight rulepacks, web workbench, dossier and proof links | Yes | Yes |
| Use inside proprietary products, internal SaaS or consulting deliverables without AGPL source obligations | No: network use of a modified version requires offering its source | Yes |
| Support, custom rulepacks and model extensions (ISO-specific tariffs, historical LMP replay, multi-site portfolios) | Community issues | By agreement |
| Indemnity | No | By agreement |

Terms are agreed in writing; see [COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md). Nothing in this README
is a license grant.

## Results on the case study (verified, reproducible)

v0.2 adds an investor layer (full-life NPV, downside, weather-year Monte Carlo, stress worlds) and
realistic engineering derates. The **Base Case** is the case-study inputs plus a 0.5 MW control
deadband, a 90% usable SoC window, 95% BESS availability and $25/MWh deferral fees. Gross margin is
an assumed $320/MWh before energy, with its break-even reported alongside.

| Design | BESS CapEx | Compliant weather years | NPV vs. interim-only | Downside (test fails) |
|---|---|---|---|---|
| **Robust hybrid 18.5 MW / 2 h, 12 h hold** | **$12.9M** | **100%** | **$66.6M** | −$11.1M |
| v0.1 optimum 13.5 MW / 2 h | $9.4M | 0% (fails the first-minutes test) | −$8.1M | −$8.1M |
| Engineering proposal 23 MW / 4 h, 4 h hold | $26.2M | 0% (17% with zero derates) | $54.7M with penalties + termination risk | −$22.7M |
| Same 23 MW / 4 h hardware, 12 h hold | $26.2M | 100% | $54.9M | −$22.7M |
| Software-only / diesel band-aid | $0 | not certifiable | $0 | $0 |

What the numbers show:

1. **The software policy, not the hardware, decides compliance.** The engineering proposal's battery
   passes in every weather year once batch work may stay deferred through long events.
2. **The economics are robust and the physics are knife-edge.**
   * NPV stays positive down to a $105/MWh gross margin (3× headroom), up to $2,139/kWh BESS cost
     (7.5×), and with firm service as early as month 14.
   * The least-cost design has **zero headroom** on the ceiling, deferrable share, hold limit, BESS MW
     and deadband. Approve a design with explicit margin, for example 22 MW / 4 h ($25.1M, NPV $56.0M)
     for the ceiling 2 MW lower and deferrable load 5 points lower.
3. **The thesis breaks on contracts.** It fails on workload hold rights (at 4 h no battery up to
   33.5 MW / 4 h works), utility ceiling tightening, and queue compression before month 14. Weather
   doesn't break it: it's 100% compliant at up to 600 signal h/yr.
4. **The case's ">25% cheaper per MWh" KPI needs carry of at least $505k/month** (8.9%/yr on $68M). The
   case states $145k. Measure unlocked MW-months and NPV instead.

> **Correction notice.** v0.1 recommended 13.5 MW / 2 h and reported $87.7M of net value. Both were
> wrong in ways an investor would catch: the design is knife-edge, and the value double-counted carry
> with margin. See [docs/INVESTOR_DILIGENCE.md](docs/INVESTOR_DILIGENCE.md) §0.

## Investment analysis commands

```bash
firmgap diligence --world base --design robust   # IC report: NPV, downside, Monte Carlo, guardrails, verdict
firmgap robust --world base                      # best design compliant in >= 90% of weather years
firmgap stress --world base --design robust      # design x world matrix, tornado, break-evens, cliffs
firmgap montecarlo --world harsh --design eng -n 40
firmgap run --preset right-sized --events        # v0.1 case view with per-event ledger
firmgap params                                   # every input, default, range, unit
```

> The Rust crate and the web workbench implement the same model (0.4.0). `tests/golden.json`,
> `tests/golden_v2.json` and `tests/golden_audit.json` pin both to identical numbers and identical
> dossier, matrix and proof-link bytes; CI fails if either drifts.

### Library

```rust
use firmgap::{apply_design, apply_world, guardrails, monte_carlo, run, verdict, Params};

let p = apply_design(&apply_world(&Params::default(), "base")?, "robust")?;   // Base Case + robust design
let r = run(&p, false)?;                                                    // validated inputs
let mc = monte_carlo(&p, 24)?;                                              // 24 weather years
let findings = guardrails(&p, &r, Some(&mc));
println!("NPV {:.1}M, downside {:.1}M, compliant {:.0}% of years, verdict {}",
    r.inv.npv / 1e6, r.inv.npv_fail / 1e6, mc.p_compliant * 100.0, verdict(&findings, true));
# Ok::<(), firmgap::Error>(())
```


## Documentation

The index by role is [docs/README.md](docs/README.md). Most used: [USER_GUIDE](docs/USER_GUIDE.md),
[COMPLIANCE](docs/COMPLIANCE.md), [CI_COMPLIANCE_GATE](docs/CI_COMPLIANCE_GATE.md), [MODEL](docs/MODEL.md),
[INVESTOR_DILIGENCE](docs/INVESTOR_DILIGENCE.md), [THREAT_MODEL](docs/THREAT_MODEL.md),
[VERIFICATION](docs/VERIFICATION.md).

## Security posture

* `#![forbid(unsafe_code)]`, **zero runtime dependencies**, overflow checks in release builds.
* All input is untrusted. It's size-capped, whitelisted, finiteness- and range-checked, and fuzzed
  weekly. Error messages are sanitized against terminal-escape injection.
* The binary makes no network calls, spawns no processes, reads no environment variables, reads only
  the files it is given and writes only to stdout and the `--out` file.
* The web workbench ships a strict Content-Security-Policy (`default-src 'none'`), treats proof links
  and dropped dossiers as hostile (size caps, allowlisted keys, no HTML sinks) and never makes a
  network request.
* CI uses SHA-pinned actions, read-only tokens, CodeQL (Rust, Actions, JS), OpenSSF Scorecard,
  cargo-deny, cargo-audit and Dependabot.
* Report vulnerabilities privately: [SECURITY.md](SECURITY.md).

## License

Copyright (C) 2026 Swanand Tanavade.

firmgap is free software under the **GNU Affero General Public License v3.0 only**
([LICENSE](LICENSE)). If you run a modified version as a network service, you must offer its source
to the users of that service. **Commercial licenses** without AGPL obligations are available; see
[COMMERCIAL-LICENSE.md](COMMERCIAL-LICENSE.md). "firmgap" is a trademark of the author; see
[TRADEMARKS.md](TRADEMARKS.md).

## Citing

See [CITATION.cff](CITATION.cff) (GitHub shows a "Cite this repository" button).

## Disclaimer

firmgap is a planning model, not an operational controller, and not investment, legal or
engineering advice. Validate against the actual interim agreement, ISO tariff, air permit and
equipment specifications before committing capital.
