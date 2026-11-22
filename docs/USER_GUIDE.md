# User guide: compliance audit, dossiers and the war room (v0.4)

This guide covers the compliance features: the CLI `audit` command and the **Compliance & Bankability**
tab of the web workbench. For the rules and their legal basis see [COMPLIANCE.md](COMPLIANCE.md). For
the equations see [MODEL.md](MODEL.md) §8.

> Screening tool, not legal advice. Every result names the provision it relies on. Have counsel
> confirm current text before you file anything.

## 1. Five-minute tour (web)

1. Open `web/index.html` in any modern browser. It works offline and loads no third-party code.
2. Click **Compliance & Bankability**. Click **Explain this screen** (top right) for four numbered
   callouts: *1. Pick law*, *2. Read verdict*, *3. Fix blockers*, *4. Export proof*.
3. Under **Sample campus**, click **Illegal revenue stack (demo blocker)**. The hero banner turns red:
   **STATUTORY BLOCKER**.
4. Switch the **Role lens** between *Executive / CFO*, *Legal / GC* and *Grid Engineer*. The numbers stay
   the same; every takeaway sentence is rephrased for that reader.
5. In **Path to compliance**, compare Path A (price-responsive curtailment only) with Path B (ERCOT
   reliability service only). Click **Apply remediation** on the recommended card. The inputs animate to the
   compliant state and the hero turns green: **BANKABLE PASS**.
6. Drag **GPU power-cap floor**, **Batch SLA slack** or **Flexible batch share** in the left rail. The curve,
   **BESS CapEx avoided** and the curve takeaway update as you drag; the rulepack re-runs when you release.
7. Scroll to **Global site comparison** to see the same campus under all eight corridors. Click a corridor
   to open it.
8. In the trust bar: **Download signed audit dossier (.json)**, then drop the file on the verify zone. It
   shows **Verified**; edit any byte and drop it again to see **Tampered or invalid**.
9. Click **Copy proof link** and send it. Whoever opens it sees the same campus and a banner confirming that
   their browser reproduces your dossier hash.
10. Click **Print filing memo** for a two-page brief (verdict, KPIs, checks with citations, remediations,
    curve, matrix, SHA-256 stamp and the CLI command that reproduces it).

## 2. Reading the screen

| Zone | What it is for | How to read it |
|---|---|---|
| 1. Context bar | Pick the law, a sample campus and who you are | Four primary rulepacks plus **More corridors** (Virginia, GB, Ireland, Spain, Singapore); two sample campuses; the role lens |
| 2. Hero verdict | The bankability answer in five seconds | One badge, three KPIs (mandatory security trapped, certified 1-hr shed, BESS CapEx avoided) and a live executive deduction |
| 3a. Left rail | The levers, in two groups | *Site & regulatory envelope* (size, storage, backup, the declarations this rulepack checks) and *Flexible AI workload levers*. Each control shows its value, a one-line caption of what it means physically, and a live line saying what it does to the rules right now |
| 3b. Checklist | Every rule, with observed value vs limit | Click a row for the legal citation, why the rule exists and what you can deduce right now |
| 3c. Path to compliance | Appears only when a rule fails | Side-by-side fixes with value, CapEx and whether each one fully resolves the rule |
| 3d. Shedding curve | MW sheddable without breaking any SLA, by event length | Tab or hover a point for its shadow price and the BESS energy it displaces; the callout states the binding horizon, CapEx displaced and whether the flexibility requirement is met |
| 3e. Global site comparison | One campus, every corridor, ranked | Verdict, failed rules and compliance drag per corridor, with `matrix_sha256` |
| 4. Trust bar | Evidence for filings and data rooms | `dossier_sha256`, `scenario_sha256`, the reproduce command, download, verify, proof link and print |

Keyboard: every control is a native button, input, select or `details` row, the curve points are
focusable, and the verify zone opens the file picker on Enter or Space. The layout stacks to one column
below 980 px.

## 3. CLI

```bash
# Screen a scenario (the rulepack comes from --jurisdiction or the file's `jurisdiction` metadata)
firmgap audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --out dossier.json
firmgap audit --scenario scenarios/ercot-sb6-100mw-compliant.toml --jurisdiction es-rd --format markdown

# Verify a dossier, optionally against a hash recorded elsewhere
firmgap audit --verify dossier.json
firmgap audit --verify dossier.json --expect ec6bf71a694acbd4e75a1022c5ff052558ab41fa2231bd39107466c0f6c1662e

# Reproduce a web proof link (fails with exit 1 if it does not reproduce its embedded hash)
firmgap audit --share 'https://kakashi3lite.github.io/firmgap/#s=eyJ...'

# Rank one campus under every corridor
firmgap compare-jurisdictions --scenario scenarios/ercot-sb6-100mw-compliant.toml --format markdown

# Sweep a portfolio directory (all rulepacks, one rulepack, or each file's declared rulepack)
firmgap batch-audit --dir scenarios --jurisdiction all --format markdown --out portfolio.json
firmgap batch-audit --dir scenarios --jurisdiction declared --fail-on-blocker

# Registry and overrides
firmgap rulepacks
firmgap audit --scenario scenarios/ercot-sb6-illegal-stack.toml --set participates_in_reliability_service=false
```

Jurisdiction ids: `ercot-sb6`, `ferc-rm26-4`, `va-sb253`, `uk-neso`, `ie-cru`, `es-rd`, `sg-gdc`, `eu-eed`.
Formats: `--format text|json|markdown` (`--json` is an alias for json). `--out` writes the JSON document
whatever the display format. `--timings` prints elapsed wall time to stderr.

Exit codes: 0 ok; 1 dossier or proof-link integrity failure; 2 usage or input error; 3 compliance gate
failure (`--fail-on-blocker`: an undeclared StatutoryBlocker, a verdict that differs from
`expected_verdict`, or a batch file that could not be audited).

### Text report sections
`VERDICT`, `CHECKS` (each with `cite:`), `PATH TO COMPLIANCE`, `FINANCIAL DRAG` (including investor NPV
before and after security carry and expected forfeiture), `SHEDDING CAPABILITY CURVE`, `DOSSIER`.

## 4. Scenario files

Scenario files are flat `key = value` TOML. Simulation keys (see `firmgap params`), compliance site
keys and two metadata keys (`jurisdiction`, `expected_verdict`; see [CI_COMPLIANCE_GATE.md](CI_COMPLIANCE_GATE.md))
can be mixed in one file. Booleans are `true` / `false`. Every value is range-checked and an
unknown key is an error. Annotated examples:

| File | Expected verdict |
|---|---|
| `scenarios/ercot-sb6-100mw-compliant.toml` | `BankablePass` under `ercot-sb6` |
| `scenarios/ercot-sb6-illegal-stack.toml` | `StatutoryBlocker` under `ercot-sb6` (Path A vs Path B) |
| `scenarios/ferc-rm26-4-colocated.toml` | `BankablePass` under `ferc-rm26-4` |

Site keys and their defaults (`ASSUMPTION` marks values with no source):

| Key | Default | Meaning |
|---|---|---|
| `gross_peak_mw` | `target_mw` | Requested peak demand, not netted for BESS or backup |
| `study_fee_usd` | 100,000 | SB 6 screening-study fee (statutory floor) |
| `security_usd_per_mw` | 50,000 | 16 TAC §25.194 financial security |
| `forfeiture_share` | 0.2 | Security retained on withdrawal after allocation |
| `release_tranche_share` | 0.2 | Security returned at energization and at each milestone |
| `months_to_energization` | 24 | ASSUMPTION |
| `milestone_interval_months` | 12 | ASSUMPTION |
| `collateral_yield` | 0.04 | ASSUMPTION (rule floor: 1-month T-bill) |
| `withdraw_probability` | 0.1 | ASSUMPTION |
| `sla_ld_usd_per_mwh` | 500 | ASSUMPTION |
| `reliability_service_mw` | 0 | MW committed to the PURA §39.170(b) service |
| `reliability_service_usd_per_mw_year` | 40,000 | ASSUMPTION (no market price yet) |
| `computational_share` | 0.9 | Share of load that is power-electronic compute |
| `backup_gen_mw` | 0 | Non-exporting backup generation |
| `backup_deploy_hours_per_year` | 20 | ASSUMPTION |
| `disconnect_test_minutes` | 30 | Planning stress-test length |
| `u_min`, `beta`, `sla_slack_hours` | 0.4, 0.75, 2 | Workload power-cap floor, speed exponent, batch SLA slack |
| `flexible_share` | `defer_share` | Flexible batch + training share |
| `training_share_of_flexible`, `training_slack_multiplier` | 0.5, 3 | Training split and slack multiple |
| `batch_value_usd_per_mwh`, `training_value_usd_per_mwh`, `critical_value_usd_per_mwh` | 250, 150, 2000 | ASSUMPTION |
| `required_shed_mw` | gross − effective ceiling | Shed the site must deliver |
| `bess_degradation_usd_per_mwh` | 20 | ASSUMPTION |
| `poi_import_limit_mw`, `max_ramp_mw_per_min`, `power_factor`, `pf_floor`, `milestone_energize_month` | gross, 10, 0.98, 0.95, interim + lead | FERC rulepack inputs |
| `it_power_kw`, `pue`, `pue_target`, `renewable_share`, `flex_readiness_target` | gross × 1000, 1.3, 1.3, 0.5, 0.1 | EED rulepack inputs |
| `va_rate_uplift_share` | 0.158 | ASSUMPTION: Virginia high-load rate uplift if the SCC approves the SB 253 shift |
| `uk_commitment_fee_usd_per_mw` | 20,000 | ASSUMPTION: proposed GB commitment fee (not yet set) |
| `ie_renewable_derated_mw`, `ie_storage_derate`, `ie_gen_derate` | 0, 0.5, 0.9 | Irish renewable credit and CRM de-rating factors (ASSUMPTION for the factors) |
| `wue_m3_per_mwh`, `hourly_renewable_match`, `es_hourly_target` | 0.1, 0.6, 0.8 | Spain RD 1032/2026 inputs |
| `green_energy_share` | 0.5 | Singapore DC-CFA2 green-energy share |
| flags | see `src/compliance.rs` `SITE_SPECS` | `participates_in_reliability_service`, `price_responsive_lmp_curtailment`, `firm_shed_telemetry`, `interconnected_after_2025`, `ride_through_certified`, `backup_disclosed`, `eed_metering_ready`, `renewable_disclosed`, `va_dominion_territory`, `backup_tier4`, `uk_gate2_ready`, `uk_end_user_evidence`, `uk_long_lead_procured`, `uk_capability_evidence`, `es_interruptible_opt_in` |

## 5. Verifying a dossier without firmgap

The file is canonical JSON. Remove `dossier_sha256`, re-serialize with sorted keys and no whitespace,
and hash the result. In Node.js:

```js
const d = JSON.parse(require('fs').readFileSync('dossier.json', 'utf8')), h = d.dossier_sha256; delete d.dossier_sha256;
const c = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? '[' + v.map(c).join(',') + ']' : '{' + Object.keys(v).sort().map(k => JSON.stringify(k) + ':' + c(v[k])).join(',') + '}';
console.log(require('crypto').createHash('sha256').update(c(d)).digest('hex') === h);
```

The hash shows the file is unchanged since it was generated. It does **not** show who generated it.
For filings, record the hash out of band or sign the file (see [THREAT_MODEL.md](THREAT_MODEL.md) T2a).

## 6. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `error: audit needs --jurisdiction ...` (exit 2) | Missing flag | Add `--jurisdiction ercot-sb6` (or another id) |
| `dossier integrity check failed: hash mismatch` (exit 1) | The file changed after it was generated | Regenerate it, or find out who edited it |
| `dossier is self-consistent but its hash ... does not match the expected` (exit 1) | The file was regenerated with different inputs, or replaced | Compare `scenario_sha256` and the inputs with the recorded run |
| `unknown parameter 'xyz'` | Typo in a scenario key | `firmgap params` lists simulation keys; site keys are in section 4 |
| Web dossier hash differs from the CLI | The web scenario was not loaded from the same file and path | Use **Download scenario (.toml)**, save it at the path shown, and run the printed command; or use the proof link with `audit --share` |
| `invalid share token: ...` (exit 2) | The link was truncated, edited or is larger than 16 KiB | Ask the sender for a fresh **Copy proof link** |
| `share token claims dossier ... but its inputs reproduce ...` (exit 1) | The inputs were changed, or a different engine version is installed | Compare `firmgap --version` with the sender's engine version |
| `COMPLIANCE GATE: FAIL` (exit 3) | A blocker the scenario did not declare, or a verdict change | Fix the campus, or declare `expected_verdict` for a deliberate fixture |
