# Production architecture specification (target state)

This document defines the system that must exist before firmgap results can back an investment
committee decision or a utility filing.

## 1. Domain and calculation engine

**Topology.** One deterministic engine (the Rust kernel) runs:
* server-side, behind an API, for system-of-record runs;
* as WASM in the browser, for interactive what-ifs.

Both builds share one code path and one golden test suite. The browser never produces a
system-of-record number.

**Determinism and versioning:**
* Every run is the pure function `(inputs, model_version, data_snapshot_ids, seed_set) → results`.
* `model_version` follows SemVer:
  * patch: no number changes;
  * minor: new outputs, existing outputs unchanged;
  * major: any change to existing numbers, which requires a regenerated golden file and a
    CHANGELOG entry.
* A scenario snapshot ([schema](../schemas/scenario_snapshot.schema.json)) stores:
  * canonical inputs;
  * a SHA-256 of the canonical JSON;
  * the model version;
  * the data snapshot IDs (load profile, LMP, signal history);
  * the results;
  * the author;
  * an approval state machine: `draft → reviewed → approved-for-IC → superseded`.

  Snapshots are immutable; edits create a new version.

**Edge-case guards:**

| Guard | Rule |
|---|---|
| Division | Every ratio has a defined zero-denominator result (`null` or `∞` with a flag), never NaN. For example, the $/MWh of a pathway with no MWh is `null` and shown as "n/a" |
| Domain | Every input has a typed range; out-of-range values are rejected server-side (no silent clamping); the UI clamps and logs |
| Cross-field | `interim ≤ target`, `ceiling ≤ target`, `interim_month < queue`, `horizon ≥ queue + 12`, `power-block $/kW ÷ 4 ≤ $/kWh` |
| Physical | SoC ∈ [0, usable E]; discharge ≤ derated P and ≤ SoC / Δt; no charging during a signal; grid draw ≥ 0; deferred work is conserved |
| Infeasibility | If no design meets the constraints, return `infeasible` with the binding constraint and the least-violating design, never an extrapolated number |
| Numerics | f64 throughout (utilization stored as f32 for reference parity); bounded loops; time budget per run; Monte Carlo seeds derived deterministically from the base seed |

**Regression suite:**
* golden parity with the reference;
* invariants on every hour;
* metamorphic properties: NPV non-increasing in BESS cost and deferral fee, non-decreasing in margin;
  shortfall non-increasing in BESS MW and non-decreasing as the ceiling falls;
* property-based fuzzing of the input surface;
* a benchmark gate (runtime regression < 20%).

## 2. Data contracts and telemetry ingestion

| Contract | Schema | Grain | Key rules |
|---|---|---|---|
| Campus load profile | [campus_load_profile](../schemas/campus_load_profile.schema.json) | 1–60 min | UTC ISO-8601, MW at the POI meter, quality flag per point, 8,760+ h for planning |
| BESS state | [bess_telemetry](../schemas/bess_telemetry.schema.json) | ≤ 4 s | SoC %, available charge/discharge MW, C-rate limits, alarms, inverter availability |
| Batch-queue SLA tiers | [batch_queue_sla](../schemas/batch_queue_sla.schema.json) | per job class | deferrable MW, max hold, SLA window, deferral fee, priority; inference is always `deferrable=false` |
| Curtailment signal | [curtailment_signal](../schemas/curtailment_signal.schema.json) | event | OpenADR 3 compatible: event ID, start, duration, ceiling MW, notice, ack deadline, signature |
| LMP / price feed | [lmp_feed](../schemas/lmp_feed.schema.json) | 5 min / hourly | node ID, market (DA/RT), $/MWh, congestion component, publication time |

Ingestion rules:
* Accept data only through signed or authenticated channels.
* Validate against the schema and reject the whole batch on structural error.
* Quarantine points that fail range checks.
* Never interpolate across a gap longer than 2 intervals; mark it as a gap instead.
* Store raw and normalized data side by side.

## 3. Security, RBAC and secrets

| Role | Scenarios | Approve for IC | Telemetry | Live dispatch policy | Admin |
|---|---|---|---|---|---|
| Viewer (LP, board) | read approved | – | – | – | – |
| Analyst | create / edit drafts | – | read | – | – |
| IC approver | read all | ✓ (four-eyes: not the author) | read | – | – |
| Site operator | read | – | read / write own site | propose | – |
| Dispatch controller (service) | – | – | write | execute approved policy only | – |
| Admin | manage | – | – | – | ✓ (no data-edit rights) |

* **Authentication:** OIDC SSO with phishing-resistant MFA; service identities use short-lived
  workload tokens (no static keys).
* **Secrets:** stored in a managed vault, rotated every 90 days or less, never in repos, images or CI
  logs. Utility signal credentials and meter keys are per site.
* **Input sanitization:** schema validation at the API edge; size caps; no HTML is rendered from data
  (text-only escaping); CSV exports neutralize formula prefixes; strict CSP; no third-party runtime
  scripts.
* **Tenant isolation:** row-level by organization; per-tenant encryption keys for telemetry.
* **Auditability:** append-only, hash-chained audit log covering logins, edits, approvals, exports
  and dispatch-policy changes. It's exportable for utility or IC review.

## 4. Reliability, observability and fallback

**Telemetry freshness state machine** (per site, per feed):

```
FRESH --(age > 2 intervals)--> DEGRADED --(age > 30 s meter | 60 s BESS | signal channel down)--> STALE --> FAILSAFE
  ^                                                                                                   |
  +--------------------------- 5 consecutive fresh intervals + operator ack ----------------------------+
```

| State | Behaviour |
|---|---|
| FRESH | Normal dispatch under the approved policy |
| DEGRADED | Widen the control deadband by +1 MW; pre-stage BESS discharge; alert on-call |
| STALE | **Pre-emptively hold campus draw ≤ ceiling − deadband** using the local controller's last good meter reading and a conservative load estimate. Pause new batch admissions. Keep BESS SoC ≥ the reserve needed for one design event |
| FAILSAFE | Local controller enforces the ceiling independently of the cloud (hard-wired limit on the PDU/UPS path); page the incident commander; notify the utility per the agreement |

* **The fail-safe holds without the cloud.** The ceiling must be enforceable by on-site equipment
  alone. The cloud optimizes; it is never the only safety layer.
* **SLOs:**
  * signal acknowledgement < 10 s;
  * ceiling reached < 4 min (with a 1 min margin to the 5 min requirement);
  * planning API p99 < 2 s;
  * zero unreported ceiling breaches.
* **Metrics:**
  * per event: gap, coverage by resource, shortfall, rebound peak, SLA-late energy;
  * per feed: freshness;
  * model drift: actual vs. simulated shortfall and deferral per event;
  * BESS availability.
* **Alerts:** on any shortfall > 0, coverage below 100% in the first minutes, BESS availability below
  the design derate, or a stale feed.
* **Incident runbook:** detect → contain (FAILSAFE) → notify the utility within contract time →
  reconcile meter data against the event log → root cause → update the scenario snapshot if the
  physics assumptions were wrong (for example, a higher derate).
