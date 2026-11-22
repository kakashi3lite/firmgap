# Jurisdiction research, 2026 (source notes for the v0.4 rulepacks)

Status: **shipped in v0.4.** Each corridor below is implemented as a rulepack (`src/rulepacks_intl.rs`,
mirrored in the web `INTL_REPORTS` block); the rule tables are in [COMPLIANCE.md](COMPLIANCE.md) §1.4 to
§1.8. The facts were checked against public sources on **22 Nov 2026**. Where only a press or law-firm
summary was available, the row says *Reported*; read the primary text before filing.

Reading guide: *Adopted* = in force or signed. *Proposed* = consultation or draft. *Reported* = taken
from a law-firm or press summary, not yet checked against the primary text.

## United States: Virginia

| Item | Finding | Status |
|---|---|---|
| SB 253 (2026, Sen. Lucas) | Allows the State Corporation Commission to assign PJM capacity-auction costs and the financing of distribution lines and substations that mainly serve data centers to the high-load class. Every shift needs SCC approval | Passed; the legislature accepted the Governor's amendments on 2026-04-22 |
| Threshold | 25 MW or more, as summarised in bill coverage | Reported |
| Size of impact | SCC staff estimated −3.4% for residential rates and +15.8% for data centers, effective 2027-01-01 (reported for the introduced bill) | Reported; may differ after amendment |
| HB 507 (2026) | DEQ must deny data-center generator air permits submitted after July 2026 unless the units are Tier IV or equivalent | Passed the legislature. The Governor's office says it is setting stricter backup-generator emission standards; check the signed text |
| HB 284 / SB 371 (2026) | Dominion and Appalachian Power must develop voluntary demand-flexibility programs for high-demand customers | Signed 2026-06-30 |
| SB 94 / HB 153 (2026) | Rezoning for high-load users above 100 MW requires a site-assessment report | Passed |

**Design implication.** SB 253 does not create an interconnection gate. A Virginia rulepack would price
cost-allocation exposure (Info) and add a Tier IV backup check (Blocker for new air permits). It would
also connect the certified shedding curve to the voluntary flexibility programs.

## United Kingdom: NESO and Ofgem

| Item | Finding | Status |
|---|---|---|
| NESO Connections Reform | Gate 2 to Whole Queue. A Gate 2 offer requires the readiness and strategic-alignment criteria | Methodologies approved by Ofgem in 2025 |
| Ofgem demand-connections reform | Three pillars: Curate, Plan and Connect | Call for input, 2026-02-12 |
| Proposed data-centre reforms (Curate) | A commitment fee secured from offer acceptance until ready to connect, not returned if the project does not progress; queue milestones (credible end user, long-lead equipment procured, financial and technical capability) | Consultation 2026-07-29 to 2026-09-17, **awaiting decision**. Fee amount not set |
| Government statement | Stronger conditions for joining and staying in the demand queue | 2026-03-11 |

**Design implication.** Gate 2 readiness is a Blocker. The milestones are Warnings until Ofgem decides.
The commitment fee is fully at risk, so it is 100% forfeiture exposure, at an assumed rate.

## Ireland: CRU

| Item | Finding | Status |
|---|---|---|
| CRU/2025/236 Large Energy Users Connection Policy | Applies only to data centres seeking a new connection. Supersedes CRU/21/124 | Decision published Dec 2025 |
| MIC of 10 MVA or more | Dispatchable on-site or proximate generation and/or storage matching MIC on a de-rated (CRM T-4) basis, participating in the SEM. The site may not ramp to full MIC until that capacity is delivered | Adopted |
| MIC of 1 to 10 MVA | An autoproducer unit meeting 100% of MIC (de-rated), trading in the SEM | Adopted |
| Below 1 MVA | Outside the policy; location still considered | Adopted |
| Run hours | The units must not be bound by limited run hours outside routine maintenance. The SOs reduce MIC if availability falls short | Adopted |
| Renewables | At least 80% of annual demand from additional Irish renewables. Six-year glide path. Up to 80% of the de-rated renewable match may count toward the dispatchable requirement | Adopted |
| Process | EirGrid *Data Centre Connection Offer Process and Policy* v3 | May 2026 |

**Design implication.** This is the strictest corridor for BESS sizing. It requires a de-rated capacity
match, not curtailment. Run-hour-capped diesel may not count.

## Spain: Real Decreto 1032/2026

| Item | Finding | Status |
|---|---|---|
| Scope | Data centres with 10 MW or more of grid access capacity, including same-owner groups on one site | **In force 2026-11-22** (published in the BOE 2026-11-21, per press reports) |
| PUE | 1.25 or less until the EU rating label (expected Aug 2027); enforced two years after start | Adopted (reported) |
| WUE | 0.1 m³/MWh or less (about 100 litres per MWh of IT energy); enforced one year after start | Adopted (reported) |
| Hourly renewables | 80% of consumption matched hour by hour with *new* renewables (plants connected at most 36 months before start) until renewables exceed 90% of the mix. The minister described a 60% → 70% → 80% ramp, reaching 80% by 2032 | Adopted (reported) |
| Transitional path | For five years, sites that accept being first to disconnect when the grid needs it | Adopted (reported) |
| Enforcement | Automatic compensation charges; sanctions can suspend connection rights for up to three years | Adopted (reported) |
| Reporting | More than 500 kW of IT power: annual data to MITECO. More than 1 MW: EU Code of Conduct declaration | Adopted (reported) |

**Design implication.** Hourly matching needs an hourly renewable profile, which the model does not
have yet. Until then the share is an input, and the interruptible path ties directly to the certified
shedding curve. Check the BOE text before building, since the figures above come from press reports.

## Singapore: IMDA / EDB

| Item | Finding | Status |
|---|---|---|
| Green Data Centre Roadmap | PUE target of 1.3 or lower for new data centres | Published 2024-05-30 |
| DC-CFA2 | At least 200 MW allocated. Requires best-in-class efficiency and at least 50% of power from approved green-energy pathways | Opened 2025-12-01, **closed 2026-03-31** |
| Competitive bar | 1.25 PUE reported for earlier allocations | Reported |

**Design implication.** Singapore is a capacity-allocation contest, not an interconnection rule. A
rulepack would score readiness for the next call.

## Related research (verified)

| Paper | Use in firmgap |
|---|---|
| Mishra, Omidi, Almassalkhi, *Job-Level Workload Allocation for Data Centers Under Flexible Interconnections*, arXiv:2611.19073 (Nov 2026) | Basis for the shedding-curve allocator (`src/shed_curve.rs`) |
| Liu, Shin, Deka, *Watts vs. Bytes: Turning Data Centers into Grid Assets via Storage Compute Co-Optimization*, arXiv:2605.16190 v2 (Aug 2026) | Storage and compute flexibility as complements; BESS sizing driven by capital and cycling cost |

## Implementation choices made in v0.4

| Corridor | Choice | Why |
|---|---|---|
| Virginia | Cost allocation is Info (priced at an assumed 15.8% uplift); Tier IV backup is the only Blocker | SB 253 shifts costs only with SCC approval; HB 507 is the hard permit rule |
| GB | Gate 2 is a Blocker; the Ofgem milestones are Warnings; the commitment fee is 100% forfeitable at an assumed $20k/MW | The milestones and fee are proposals awaiting decision |
| Ireland | MIC = gross MW / power factor; de-rating factors are inputs (defaults 0.5 storage, 0.9 generation); 8,000 h/yr run-hour threshold | CRU sizes on de-rated CRM T-4 factors, which change with each auction pack |
| Spain | Hourly matching is an input share with a target; the interruptible path downgrades the blocker to a warning | firmgap has no hourly renewable profile yet |
| Singapore | 1.3 PUE is a Blocker; 1.25 is a Warning; 50% green energy is a Blocker | 1.25 is a competitive bar reported for awards, not a published rule |

## Adding or changing a corridor

1. Read the primary text (statute, decision paper or BOE), not only summaries. Record the exact section.
2. Classify every check as statute, rule, protocol, permit, contract or planning assumption.
3. Add the rule to `src/compliance.rs` **and** the web `complianceReport` block, with identical text.
4. Add golden cases (`scripts/golden.mjs --write`) and a scenario that passes and one that fails.
5. Bump the rulepack id's `YYYY.MM` suffix whenever the underlying text changes.
