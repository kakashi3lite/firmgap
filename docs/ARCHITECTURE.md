# Architecture

This document maps how the code fits together. Diagrams are Mermaid, which GitHub renders inline.

## 1. Module dependency graph

```mermaid
graph TD
  main[src/main.rs<br/>CLI] --> presets
  main --> scenario
  main --> report
  main --> sim
  main --> optimise
  scenario[scenario.rs<br/>untrusted file parser] --> params
  scenario --> presets
  presets[presets.rs] --> params
  params[params.rs<br/>single source of inputs] --> error
  year[year.rs<br/>8,760 h year] --> rng[rng.rs<br/>Mulberry32]
  year --> params
  sim[sim.rs<br/>dispatch engine] --> year
  sim --> economics
  economics[economics.rs<br/>A/B/C pathways] --> params
  optimise[optimise.rs] --> sim
  optimise --> economics
  report[report.rs] --> json[json.rs<br/>escaping writer]
  report --> calendar[calendar.rs]
  report --> sim
  error[error.rs<br/>sanitized Display]
  main --> audit[audit_bundle.rs<br/>SHA-256, dossier]
  audit --> canon[canon.rs<br/>canonical JSON + parser]
  audit --> compliance[compliance.rs<br/>rulepacks + remediation]
  audit --> shed[shed_curve.rs<br/>KKT shedding curve]
  compliance --> sim
  compliance --> shed
  shed --> economics
  main --> portfolio[portfolio.rs<br/>matrix, batch, proof links]
  portfolio --> audit
  portfolio --> compliance
  compliance --> intl[rulepacks_intl.rs<br/>VA, GB, IE, ES, SG]
```

There are no cycles, and every arrow points toward leaf modules with no I/O. Only `main.rs` and
`scenario::read_file` touch the operating system.

## 2. Data flow of `firmgap run`

```mermaid
sequenceDiagram
  participant U as User
  participant CLI as main.rs
  participant P as Params
  participant Y as year
  participant S as sim
  participant E as economics
  participant R as report
  U->>CLI: args (UTF-8 checked, ≤256)
  CLI->>P: defaults → preset → config file → --set (each validated)
  CLI->>P: validate() cross-checks
  CLI->>S: run(p)
  S->>Y: build_year(p)
  S->>S: simulate × 4 (interim, flex, firm, firm_bess)
  S->>E: economics(p, sims)
  E-->>S: Economics
  S-->>CLI: Run
  CLI->>R: run_text / run_json
  R-->>U: stdout (exit 0) or sanitized error on stderr (exit 2)
```

## 3. Dispatch decision flow (one hour, compliance mode)

```mermaid
flowchart TD
  A[hour t] --> B{expired backlog?}
  B -- yes --> B1[count SLA breach<br/>bill at base price] --> C
  B -- no --> C{curtailment signal?}
  C -- no --> N1[replay backlog into cap headroom] --> N2[recharge BESS at RTE] --> Z
  C -- yes --> D{gap = L − ceiling > 0?}
  D -- no --> D1[replay backlog into ceiling headroom] --> Z
  D -- yes --> E{event hour 1?}
  E -- yes --> F[ramp segment ramp_min:<br/>GPU power-cap + BESS + genset]
  F --> F1{uncovered?}
  F1 -- yes --> F2[event fails first-minutes test]
  F1 -- no --> G
  F2 --> G
  E -- no --> G[full segment:<br/>throttle ≤ hold limit, BESS, genset<br/>in strategy order]
  G --> H[remainder = shortfall → penalty]
  H --> I[queue throttled MWh<br/>deadline t + SLA + 1]
  I --> Z[accumulate energy & cost]
```

## 4. Event and certification state machine

```mermaid
stateDiagram-v2
  [*] --> NoGap: load ≤ ceiling all event
  [*] --> Covered: gap fully covered incl. first minutes
  [*] --> RampFail: first-minutes gap uncovered
  Covered --> Shortfall: later hour uncovered
  Covered --> SlaBreach: deferred work missed deadline
  RampFail --> NotCertified
  NotCertified --> PathwayC_equals_B
  Covered --> Compliant
  Compliant --> [*]
```

A single `RampFail` anywhere in the year makes `certified = false`. Pathway C then collapses to B
(interim-only). This models a utility witness test.

## 5. Pathway phasing

```mermaid
gantt
  dateFormat  X
  axisFormat  M%s
  section A wait
  0 MW            :a1, 0, 38
  45 MW firm      :a2, 38, 60
  section B interim
  0 MW            :b0, 0, 4
  22 MW non-firm  :b1, 4, 38
  45 MW firm      :b2, 38, 60
  section C bridge
  0 MW            :c0, 0, 4
  22 MW interim   :c1, 4, 10
  45 MW non-firm + BESS :crit, c2, 10, 38
  45 MW firm + arbitrage :c3, 38, 60
```

## 6. Two implementations, one model

```mermaid
flowchart LR
  W[web/index.html<br/>FIRMGAP-CORE block] -- node scripts/golden.mjs --check --> G[(tests/golden.json)]
  G -- values embedded in --> T[tests/golden.rs]
  T -- cargo test --> RS[src/*.rs]
```

CI fails if either side drifts. See [CONTEXT_MAP.md](CONTEXT_MAP.md#changing-the-model) for the
change procedure.
