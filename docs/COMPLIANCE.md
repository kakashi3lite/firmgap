# Compliance & bankability (model 0.4)

`firmgap audit`, `compare-jurisdictions`, `batch-audit` and the **Compliance & Bankability** tab of
`web/index.html` evaluate a campus against eight versioned jurisdiction rulepacks. They propose the cheapest counterfactual path to compliance,
certify how much load the campus can shed for each event duration, and write a canonical-JSON dossier
with a SHA-256 hash.

> **Screening tool, not legal advice.** Each check states its citation and its legal basis
> (statute, adopted rule, approved protocol, permit, contract, or planning assumption). Verify the
> current text with counsel before relying on it. Sources were checked on **22 Nov 2026**.

```bash
firmgap audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --jurisdiction ercot-sb6 --out dossier.json
firmgap audit --verify dossier.json                       # exit 0 = intact, 1 = tampered or malformed
firmgap audit --verify dossier.json --expect <sha256>     # also pin a hash recorded out of band
firmgap audit --scenario scenarios/ercot-sb6-illegal-stack.toml --jurisdiction ercot-sb6   # StatutoryBlocker + Path A / Path B
firmgap compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml --format markdown
firmgap batch-audit --dir scenarios --jurisdiction all --format markdown
firmgap rulepacks                                          # registry: ids, legal status, rules, citations
```

## 1. Rulepacks

| Rulepack id | `--jurisdiction` | Legal status |
|---|---|---|
| `TX-SB6-16TAC25.194-2026.10` | `ercot-sb6` | Statute (PURA as amended by SB 6, 2025) and adopted rule (16 TAC §25.194, PUCT Project 58481, adoption order filed 18 Sep 2026), plus ERCOT NOGRR 282 (approved 9 Jul 2026, effective 1 Aug 2026) |
| `US-FERC-RM26-4-2026.01` | `ferc-rm26-4` | **No final rule.** Docket RM26-4 (DOE large-load ANOPR) and the 18 Jun 2026 show-cause orders. The checks model *anticipated* interconnection-agreement terms |
| `US-VA-SB253-2026.06` | `va-sb253` | Statute: Va. SB 253 (2026; Governor's amendments accepted 2026-04-22), HB 507 (Tier IV backup permits), HB 284 / SB 371 (signed 2026-06-30). Cost shifts need SCC approval |
| `GB-NESO-OFGEM-DC-2026.09` | `uk-neso` | **Proposed.** NESO Gate 2 (approved methodology) plus Ofgem's data-centre commitment fee and queue milestones (consultation closed 2026-09-17, awaiting decision) |
| `IE-CRU-2025-236-2026.05` | `ie-cru` | Adopted policy: CRU/2025/236 Large Energy Users connection policy; EirGrid process v3 (May 2026) |
| `ES-RD-1032-2026.11` | `es-rd` | Statute: Real Decreto 1032/2026, in force 2026-11-22 (figures from press reports of the BOE text; verify the BOE before filing) |
| `SG-IMDA-GDCR-DCCFA2-2026.03` | `sg-gdc` | Adopted policy: Green Data Centre Roadmap (2024) and DC-CFA2 (closed 2026-03-31) |
| `EU-EED-ART12-2026.01` | `eu-eed` | Directive (EU) 2023/1791 Art. 12 and Annex VII; Delegated Regulation (EU) 2024/1364. This is a reporting obligation, not a performance standard |

### 1.1 ERCOT / Texas SB 6

| Rule id | Severity if failed | What is checked | Basis |
|---|---|---|---|
| `SB6_APPLICABILITY` | Info | Requested peak demand of 75 MW or more, **gross**: BESS and backup generation do not reduce it | PURA §37.0561(c) |
| `SB6_FINANCIAL_SECURITY` | Info (drives $) | Security of $50,000 per MW plus a study fee of at least $100,000. If a request is withdrawn after allocation, the security tied to allocated capacity (20%) is retained. 20% is returned at energization, then the rest in 20% steps at each SLLIA milestone | PURA §37.0561(f); 16 TAC §25.194 |
| `SB6_ILLEGAL_REVENUE_STACK` | **Blocker** | Cannot enroll in the large-load reliability service **and** curtail on wholesale prices | PURA §39.170(b)(3) |
| `SB6_FIRM_SHED_TELEMETRY` | **Blocker** | Remote curtailment equipment for firm load shed, installed before interconnection (loads interconnected after 2025) | PURA §39.170(a); PGRR 145 |
| `SB6_DISCONNECT_30M` | Warning | The campus survives a full disconnect of N minutes (default 30) using certified shedding, backup generation and BESS | **Planning stress test.** The PUCT declined to codify a 30-minute window |
| `ERCOT_NOGRR282` | Warning | Ride-through applies to Large Electronic Loads (at least 75 MW, at least 50% computational load) | NOGRR 282; NPRR 1308 definition |
| `SB6_BACKUP_DISCLOSURE` | **Blocker** | Backup generation able to serve at least 50% of on-site demand must be disclosed | PURA §37.0561 |
| `TCEQ_AIR_PERMIT_CAP` | **Blocker** | Curtailment genset hours plus directed-backup hours must stay within the permit's run-hour cap | Site permit; SB 6 does not override emissions limits; RICE NESHAP 50 h/yr non-emergency |

Two points where the spec this release was built from differed from the adopted text:
* **The study fee is a statutory floor.** "$100,000 at least" is in the statute; the amount for a particular utility can be higher (law-firm summaries quote $100k to $300k). Set `study_fee_usd`.
* **The financial security is $50,000/MW, and it is posted on requested peak demand.** The proposed rule had $100,000/MW. The PUCT declined to net co-located generation out of the demand figure. firmgap therefore keeps security invariant to BESS size; a test asserts this.

### 1.2 FERC RM26-4 (anticipated)
POI import ceiling, from the hourly flex trace (Blocker); ramp rate, the larger of the event onset and
the hourly step (Warning); power-factor floor (Warning); staged-energization milestone (Warning).

### 1.3 EU EED Art. 12
Applicability at 500 kW or more of installed IT power. Then: KPI metering readiness, PUE against a
**national or contractual** target (the Directive sets no PUE ceiling), renewable-share disclosure, and
a grid-flexibility readiness score. The score is a firmgap metric (certified 1-hour shed / gross peak),
not a legal requirement. All of these are Warnings.

### 1.4 Virginia SB 253 (+ HB 507, HB 284)
| Rule id | Severity | Check |
|---|---|---|
| `VA_APPLICABILITY` | Info | 25 MW or more in Dominion territory (threshold as reported in bill summaries) |
| `VA_COST_ALLOCATION` | Info (drives $) | Energy cost × `va_rate_uplift_share` (assumed 15.8%, the SCC staff estimate reported in press) if the SCC approves the shift |
| `VA_TIER4_BACKUP` | **Blocker** | Backup generators must be Tier IV or equivalent for permits submitted after July 2026 |
| `VA_DEMAND_FLEX` | Info | Certified 1-hour shed to offer into the voluntary demand-flexibility programs |

### 1.5 GB NESO / Ofgem (proposed)
| Rule id | Severity | Check |
|---|---|---|
| `UK_GATE2` | **Blocker** | Gate 2 readiness and strategic-alignment criteria |
| `UK_END_USER`, `UK_LONG_LEAD`, `UK_CAPABILITY` | Warning | Proposed queue milestones: credible end user, long-lead equipment procured, financial and technical capability |
| `UK_COMMITMENT_FEE` | Info (drives $) | Proposed fee secured from offer acceptance and not returned if milestones are missed. The rate (`uk_commitment_fee_usd_per_mw`, default $20k/MW) is an assumption: Ofgem has not set it. It is counted as 100% forfeiture exposure |

### 1.6 Ireland CRU/2025/236
| Rule id | Severity | Check |
|---|---|---|
| `IE_APPLICABILITY` | Info | MIC = gross MW / power factor. Below 1 MVA out of scope; 1 to 10 MVA autoproducer; 10 MVA or more dispatchable generation and/or storage |
| `IE_DISPATCHABLE_MATCH` | **Blocker** | De-rated capacity (backup × `ie_gen_derate` + usable BESS MW × `ie_storage_derate`) ≥ MIC − min(renewable credit, 80% of MIC). De-rating factors are assumptions; use the latest CRM T-4 pack |
| `IE_RUN_HOURS` | Warning | Generation counted toward MIC must not be run-hour limited (firmgap threshold 8,000 h/yr) |
| `IE_RENEWABLE_80` | Warning | Additional Irish renewables ≥ 80% of annual demand (six-year glide path) |

### 1.7 Spain Real Decreto 1032/2026
| Rule id | Severity | Check |
|---|---|---|
| `ES_APPLICABILITY` | Info | 10 MW or more of grid access capacity |
| `ES_PUE` | **Blocker** | PUE ≤ 1.25 until the EU rating label |
| `ES_WUE` | **Blocker** | WUE ≤ 0.1 m³/MWh until the EU label |
| `ES_HOURLY_RENEWABLE` | **Blocker** (Warning on the interruptible path) | Hourly new-renewable match ≥ `es_hourly_target` (80% end state; transitional steps before 2032 as described by the minister) |
| `ES_REPORTING` | Info | MITECO reporting above 500 kW of IT power; EU Code of Conduct declaration above 1 MW |

### 1.8 Singapore GDCR / DC-CFA2
| Rule id | Severity | Check |
|---|---|---|
| `SG_PUE_GDCR` | **Blocker** | PUE ≤ 1.3 (roadmap target for new builds) |
| `SG_PUE_CFA` | Warning | PUE ≤ 1.25 (bar reported for capacity awards) |
| `SG_GREEN_50` | **Blocker** | At least 50% of power from approved green-energy pathways |
| `SG_CFA_WINDOW` | Info | DC-CFA2 closed 2026-03-31; new capacity needs a future call |

### Verdict
`StatutoryBlocker` if any Blocker fails; otherwise `ConditionalWarning` if any Warning fails; otherwise `BankablePass`.

### Registry
`JurisdictionRulepack::ALL` lists the eight rulepacks in a fixed order (also the tie-break order of the
matrix). Each variant has an `id()`, `status()` and `rules()`. Adding a corridor means adding a variant,
its rule ids and citations, and an evaluation function, with matching code in the web
`INTL_REPORTS` block. The simulator, dossier, matrix and batch code do not change.

## 2. Counterfactual path-to-compliance solver

| Failing rule | Options returned | How |
|---|---|---|
| `SB6_ILLEGAL_REVENUE_STACK` | **Path A** (LMP curtailment only) vs **Path B** (reliability service only) | A = (throttled + BESS MWh) × (stress − base price). B = committed MW × `reliability_service_usd_per_mw_year`. Returns the annual value, 5-year PV at WACC and the delta; the higher value is recommended |
| `SB6_DISCONNECT_30M` | Add BESS **or** more SLA slack | Exact minimum BESS MW from the derate and SoC window, rounded up to 0.1 MW, with CapEx. The minimum SLA slack comes from a 64-step bisection, rounded up to 0.01 h. The cheaper option is recommended |
| `TCEQ_AIR_PERMIT_CAP` | Add BESS **or** amend the permit | A 64-step bisection over BESS MW, re-simulating the year each time. If directed backup alone exceeds the cap, the result is "amend permit" (storage cannot fix it) |
| `VA_TIER4_BACKUP` | Specify Tier IV backup | A flag change before the air-permit filing |
| `UK_GATE2` / milestones | File the missing evidence | Lists exactly which milestone flags are open |
| `IE_DISPATCHABLE_MATCH` | Add de-rated storage | Minimum BESS MW = (requirement − de-rated generation) / (storage de-rate × availability), rounded up to 0.1 MW, with CapEx |
| `ES_HOURLY_RENEWABLE` | Contract hourly renewables **or** take the interruptible path | The second option downgrades the blocker to a warning for up to five years and does not resolve it |
| `SG_GREEN_50` | Contract green-energy pathways to 50% | — |

BESS proposals are capped at 60 MW, the workbench input range. Every remediation lists the parameter
`changes` that implement it. Applying them (CLI `--set`, or **Apply remediation** in the web tab)
re-runs every rule. The golden tests confirm the rule turns green.

> **Assumption, not market data:** `reliability_service_usd_per_mw_year` defaults to $40k/MW-yr. ERCOT
> had not finished designing the PURA §39.170(b) service when this was written. Change it, and Path A
> vs Path B can flip.

## 3. Shedding capability curve (`src/shed_curve.rs`)

Workload classes: *inference* (critical, not sheddable), *batch* and *training* (the flexible share,
split by `training_share_of_flexible`; training slack = batch slack × `training_slack_multiplier`).
GPU power-capping to fraction `u` runs a job at speed `u^β`. For an event of `dt` hours, the minimum
cap that preserves the SLA slack is

```
u_bar(dt) = min(1, max(u_min, max(0, 1 − slack/dt)^(1/β)))
```

Certified shed = Σ nominal × (1 − u_bar). It is non-increasing in `dt`; a test sweeps 420
combinations to check this. For a target `δ` = min(required, certified), the allocation that loses the
least value solves the KKT conditions with one dual price λ:
`u_k(λ) = clip((λ / (v_k·dt·β_k))^(1/(β_k−1)), u_bar_k, 1)`, found by a fixed 64-step bisection.
The reported shadow price is λ/dt in $/MWh.

**BESS CapEx avoided.** Without shedding, the BESS must cover the required shed for 4 h. With it, BESS
power covers the residual at the 30-minute binding horizon and BESS energy covers the worst residual
over 15 min to 4 h. Both use the same two-part cost as `economics::bess_capex`.

Grounding: Mishra, Omidi & Almassalkhi, *Job-Level Workload Allocation for Data Centers Under
Flexible Interconnections*, arXiv:2611.19073 (job-level power/speed response, value-based allocation,
a planner-facing "how much and for how long" measure). Liu, Shin & Deka, *Watts vs. Bytes*,
arXiv:2605.16190 (storage and compute flexibility are complements; sizing is driven by capital and
cycling cost). firmgap's closed-form allocator is a simplification of both and does not reproduce
their case studies.

## 4. Dossier and integrity

Canonical JSON: keys sorted, no whitespace, numbers in the ECMAScript `Number#toString` format after
quantisation to 1e-4 (round half up), and ASCII-only strings. `dossier_sha256` = SHA-256 of the
canonical body without that field. Rust (`src/canon.rs`, `src/audit_bundle.rs`, a FIPS 180-4
implementation tested on the NIST vectors) and the browser produce **byte-identical** files.
`tests/golden_audit.rs` compares 8 full dossiers byte for byte. `scripts/golden.mjs` cross-checks the
browser's SHA-256 against `node:crypto`, and the web tab also cross-checks against WebCrypto.

**What the hash does and does not prove.** It detects any accidental or naive edit. It is **not a
signature**: anyone who edits the file can recompute the hash. For tamper evidence against a
motivated party, record the hash out of band (IC minutes, a signed git tag, an email), then verify
with `--expect <sha256>`. Alternatively, sign the file (Sigstore `cosign sign-blob`, minisign or GPG).

## 5. Global bankability matrix, batch audits and proof links

* **Matrix** (`compare-jurisdictions`): one simulation, every rulepack, ranked by verdict, then failed blockers,
  failed warnings, total compliance drag (quantised to 1e-4) and registry order. The canonical JSON carries
  `scenario_sha256` and `matrix_sha256`, byte-identical to the browser's matrix.
* **Batch** (`batch-audit`): every `*.toml` directly inside `--dir` (no recursion, no symlinks, names
  `[A-Za-z0-9._-]`, at most 1,000 files, 64 KiB each), sorted by name. Each result carries the same
  `dossier_sha256` as `firmgap audit --scenario <dir>/<file> --jurisdiction <id>`. `--jurisdiction declared` uses each
  file's metadata. A result is *unexpected* if it is a StatutoryBlocker the file did not declare in
  `expected_verdict`, or if it differs from `expected_verdict`.
* **Proof link** (`#s=<base64url>`): base64url of canonical JSON `{v:1, f:<path>, j:<rulepack>, p:<all
  params>, s:<resolved site>, h:<dossier_sha256>}`, at most 16 KiB. The browser clamps inputs and the CLI
  rejects anything out of range, unknown or mistyped. `firmgap audit --share <url>` rebuilds the dossier and
  exits 1 if it does not reproduce `h`.

## 6. Scenario metadata

```toml
jurisdiction = "ercot-sb6"           # default --jurisdiction for audit and batch-audit --jurisdiction declared
expected_verdict = "StatutoryBlocker" # deliberate fixtures: the gate passes only if this verdict is reproduced
```
Neither key enters `inputs`, so neither changes any hash.

Schemas: `schemas/compliance_dossier.schema.json`, `schemas/compliance_rulepack.schema.json` (one entry of
`firmgap rulepacks --json`), `schemas/compare_matrix.schema.json`, `schemas/batch_audit.schema.json`.
