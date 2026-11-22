# Glossary

| Term | Meaning in firmgap |
|---|---|
| **Firm service** | Interconnection that the utility must serve at all times. In constrained zones it often follows a multi-year cluster study and network upgrades. |
| **Interim non-firm service** | Early, partial interconnection that the utility may curtail during system or transmission stress. |
| **Curtailment signal** | Utility instruction to keep grid draw at or below the **ceiling** (`ceiling_mw`). |
| **Firm gap** | The period between interim energization and firm service. It's the economic object this tool prices. |
| **Gap** | `load − ceiling` during a signal, in MW (power) or MWh (energy). |
| **Shortfall** | Gap not covered by throttle, BESS or genset; drawn from the grid in breach of the ceiling and billed at `penalty`. |
| **First-minutes test / 5-minute test** | Whether the gap is covered before scheduler preemption completes (`ramp_min`). Only GPU power-capping, BESS and genset count. |
| **Certification** | All events pass the first-minutes test, so the utility signs the expansion. |
| **Throttle / deferral** | Pausing or slowing deferrable batch / training work (`defer_share`) and replaying it later. |
| **Throttle hold limit** | Max hours into an event during which new work may be deferred (`max_throttle_h`). |
| **SLA window / breach** | Deferred work must be replayed within `sla_window` hours; late work is a breach, as a share of annual compute energy. |
| **BESS** | Battery energy storage system: power (MW) × duration (h) = energy (MWh). |
| **RTE** | Round-trip efficiency of the battery. |
| **Power block / energy block** | BESS cost split between $/kW (inverters, interconnect) and $/kWh (cells, enclosures). |
| **CRF** | Capital recovery factor; turns CapEx into an equivalent annual cost at `wacc` over `life` years. |
| **LMP** | Locational marginal price: the nodal wholesale energy price. Modelled as `base_price` or `stress_price`. |
| **Queue-delay carry** | Monthly cost of capacity that is built but not energized: financing, lease burn and lost margin. |
| **Pathway A / B / C** | Wait for firm / interim-only / interim + non-firm flex bridge. |
| **Breakeven carry** | Monthly carry at which pathway C is exactly 25% cheaper per MWh than A. |
| **Arbitrage value** | Post-firm saving from discharging the BESS during stress hours. |
| **RICE NESHAP** | US EPA rule for stationary reciprocating engines, which limits non-emergency hours for emergency gensets. |

## Compliance terms (v0.3 and v0.4)

| Term | Meaning |
|---|---|
| Rulepack | A versioned set of checks for one jurisdiction, e.g. `ES-RD-1032-2026.11`; the suffix is the month of the source text |
| Blocker / Warning / Info | A failed Blocker makes the campus not bankable; a failed Warning is a diligence item; Info rows carry context and money |
| Path to compliance | The cheapest parameter change that turns a failing rule green, computed by the engine |
| Certified shed | MW the campus can shed by GPU power-capping for a given event length without breaking any SLA |
| MIC | Maximum Import Capacity (Ireland), the contracted import limit in MVA; firmgap uses gross MW / power factor |
| De-rated capacity | Nameplate MW × the capacity-market de-rating factor for that technology and duration |
| Gate 2 | GB connection-reform stage that gives a firm queue position to ready, strategically aligned projects |
| PUE / WUE | Power usage effectiveness (total energy / IT energy); water usage effectiveness (m³ of water per MWh of IT energy) |
| Hourly matching | Covering each hour's consumption with renewable output in the same hour, not an annual total |
| Tier IV | The strictest US EPA emission tier for non-road diesel engines |
| Dossier | Canonical JSON of inputs, curve and compliance report with `dossier_sha256` |
| Proof link | A URL fragment `#s=<base64url>` carrying the scenario and its dossier hash, reproducible in any browser or with `firmgap audit --share` |
| Bankability matrix | One campus ranked under every rulepack (`compare-jurisdictions`) |
