#!/usr/bin/env node
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade
//
// Two-sided parity guard between the reference JavaScript model embedded in
// web/index.html and the golden values the Rust tests assert:
//   tests/golden.json     model 0.1 kernel (case study, v0.1 optimiser)
//   tests/golden_v2.json  model 0.2 (derates, investor NPV, Monte Carlo, guardrails, robust optimiser, stress)
//
//   node scripts/golden.mjs --check   # CI: fail if the web model drifted from either file
//   node scripts/golden.mjs --write   # after an intentional model change, then:
//   python3 scripts/gen_golden_v2.py  # regenerate tests/golden_v2.rs (and update tests/golden.rs by hand)
//
// The script only reads web/index.html and reads/writes the two JSON files.
// It executes the model block in an isolated vm context with no require, no
// process and no file-system access.

import { readFileSync, writeFileSync } from 'node:fs';
import vm from 'node:vm';

const root = new URL('..', import.meta.url);
const html = readFileSync(new URL('web/index.html', root), 'utf8');
const m = html.match(/\/\*FIRMGAP-CORE-BEGIN\*\/([\s\S]*?)\/\*FIRMGAP-CORE-END\*\//);
if (!m) {
  console.error('FIRMGAP-CORE markers not found in web/index.html');
  process.exit(2);
}
const sandbox = Object.create(null);
vm.createContext(sandbox, { codeGeneration: { strings: false, wasm: false } });
vm.runInContext(`${m[1]}\n;globalThis.__api = { mulberry32, runAll, optimise, sanitizeParams, CASE_STATED, WORLDS, DESIGNS, worldParams, designParams, guardrails, verdictOf, isCompliant, monteCarlo, seedList, robustOptimise, stressSuite, MODEL_VERSION };`, sandbox, { timeout: 600000 });
const { mulberry32, runAll, optimise, sanitizeParams, CASE_STATED, WORLDS, DESIGNS, worldParams, designParams, guardrails, verdictOf, isCompliant, monteCarlo, seedList, robustOptimise, stressSuite, MODEL_VERSION } = sandbox.__api;

const CASE = {
  queueMonths: 38, interimMW: 22, interimMonth: 4, bessLead: 6, horizon: 60, ceilingMW: 22,
  targetMW: 45, util: 0.78, deferShare: 0.35, fastCap: 0.10, rampMin: 4, slaWindow: 4, maxThrottleH: 4,
  stressHours: 180, avgEventH: 3.5, basePrice: 68, stressPrice: 310, penalty: 450,
  bessMW: 23, bessH: 4, bessCost: 285, bessPowerCost: 250, rte: 0.88,
  genMW: 0, genCapH: 50, genCost: 290, genRental: 65,
  carry: 145000, wacc: 0.09, life: 10, margin: 250, strategy: 'throttle', seed: 2026, slaTarget: 0.015,
};
const SCENARIOS = {
  case_eng: {},
  right_sized: { bessMW: 13.5, bessH: 2, maxThrottleH: 12 },
  throttle_only: { bessMW: 0, maxThrottleH: 12 },
  genset_bandaid: { bessMW: 0, genMW: 17, genCapH: 50 },
  bess_first: { bessMW: 13.5, bessH: 2, maxThrottleH: 12, strategy: 'bess' },
  deep_stress: { bessMW: 13.5, bessH: 2, maxThrottleH: 12, ceilingMW: 15, stressHours: 260 },
  high_carry: { bessMW: 13.5, bessH: 2, maxThrottleH: 12, carry: 453000 },
};

function generate() {
  const r = mulberry32(2026);
  const out = { rng: [r(), r(), r(), r(), r()], scenarios: {} };
  for (const [k, patch] of Object.entries(SCENARIOS)) {
    const p = { ...CASE, ...patch };
    const R = runAll(p);
    const f = R.sims.flex, e = R.econ;
    const pw = (s, extra) => Object.assign({ mwh: s.mwh, total: s.total, perMWh: s.perMWh, carry: s.carry }, extra ? { energy: s.energy, bess: s.bess, gen: s.gen, penalty: s.penalty } : {});
    out.scenarios[k] = {
      patch,
      events: R.Y.events.length,
      first_events: R.Y.events.slice(0, 4).map((x) => [x.start, x.dur]),
      flex: { demand: f.demand, gapE: f.gapE, thrE: f.thrE, besE: f.besE, genE: f.genE, shortE: f.shortE, breach: f.breach, energyCost: f.energyCost, maxGap: f.maxGap, failed5: f.failed5, genHrs: f.genHrs, catchE: f.catchE },
      econ: { F: e.F, certified: e.certified, capex: e.capex, bessAnn: e.bessAnn, pull: e.pullForward, reduction: e.reduction, breakevenCarry: e.breakevenCarry, A: pw(e.S.A), B: pw(e.S.B), C: pw(e.S.C, true) },
      firm_arb: R.sims.firm.energyCost - R.sims.firmB.energyCost,
    };
  }
  const p0 = { ...CASE, optimisePolicy: true };
  const R0 = runAll(p0);
  const o = optimise(p0, R0.Y, R0.sims.firm, R0.sims.interim);
  out.optimise = o.best;
  out.optimise_evals = o.results.length;
  return out;
}

function compare(a, b, path, errs) {
  if (typeof b === 'number' || b === null) {
    const want = b === null ? Infinity : b;
    const got = a === null ? Infinity : a;
    if (!Number.isFinite(want)) { if (Number.isFinite(got)) errs.push(`${path}: got ${got}, want ${want}`); return; }
    if (Math.abs(got - want) > 1e-9 * Math.max(1, Math.abs(want))) errs.push(`${path}: got ${got}, want ${want}`);
  } else if (Array.isArray(b)) {
    if (!Array.isArray(a) || a.length !== b.length) { errs.push(`${path}: array length`); return; }
    b.forEach((v, i) => compare(a[i], v, `${path}[${i}]`, errs));
  } else if (b && typeof b === 'object') {
    for (const k of Object.keys(b)) compare(a?.[k], b[k], `${path}.${k}`, errs);
  } else if (a !== b) {
    errs.push(`${path}: got ${a}, want ${b}`);
  }
}

const SNAKE = { queueMonths: 'queue_months', interimMonth: 'interim_month', interimMW: 'interim_mw', ceilingMW: 'ceiling_mw', bessLead: 'bess_lead', horizon: 'horizon', targetMW: 'target_mw', util: 'util', deferShare: 'defer_share', fastCap: 'fast_cap', rampMin: 'ramp_min', slaWindow: 'sla_window', maxThrottleH: 'max_throttle_h', stressHours: 'stress_hours', avgEventH: 'avg_event_h', basePrice: 'base_price', stressPrice: 'stress_price', penalty: 'penalty', bessMW: 'bess_mw', bessH: 'bess_h', bessCost: 'bess_cost', bessPowerCost: 'bess_power_cost', rte: 'rte', genMW: 'gen_mw', genCapH: 'gen_cap_h', genCost: 'gen_cost', genRental: 'gen_rental', carry: 'carry', wacc: 'wacc', life: 'life', slaTarget: 'sla_target', safetyMargin: 'safety_margin', bessDerate: 'bess_derate', socWindow: 'soc_window', deferCost: 'defer_cost', slaPenalty: 'sla_penalty', salvage: 'salvage', marginPreEnergy: 'margin_pre_energy', swSetup: 'sw_setup', swPriceMWMonth: 'sw_price_mw_month', mcSeeds: 'mc_seeds', seed: 'seed', strategy: 'strategy' };

function generateV2() {
  const W = id => WORLDS.find(w => w.id === id), D = id => DESIGNS.find(d => d.id === id);
  const start = sanitizeParams(CASE_STATED).p;
  const mk = (w, d) => worldParams(designParams(start, D(d)), W(w));
  const toSnake = p => { const o = {}; for (const k of Object.keys(SNAKE)) o[SNAKE[k]] = p[k]; return o; };
  const num = v => (v === null || v === undefined) ? null : (v === Infinity ? 'inf' : v);
  const inv = i => ({ npv: i.npv, npvFail: i.npvFail, payback: num(i.payback), irr: num(i.irr), bridgeMonthly: i.bridgeMonthly, arbMonthly: i.arbMonthly, disposition: i.disposition, failDisposition: i.failDisposition, postPV: i.postPV, costPerMWMonth: num(i.costPerMWMonth), breakEvenMargin: num(i.breakEvenMargin), breakEvenDeferCost: num(i.breakEvenDeferCost), breakEvenSlaPenalty: num(i.breakEvenSlaPenalty), swPV: i.swPV, swShare: num(i.swShare), swPaybackDays: num(i.swPaybackDays), capexPV: i.capexPV, pvFactor: i.pvFactor, bridgeMonths: i.bridgeMonths, mwMonths: i.mwMonths });
  const out = { schema: 'firmgap.golden.v2', model: MODEL_VERSION, snake: SNAKE, worlds_designs: {}, seeds: seedList(2026, 5) };
  const combos = [['base', 'robust'], ['base', 'edge'], ['base', 'eng'], ['base', 'sw'], ['base', 'diesel'], ['harsh', 'robust'], ['fasttrack', 'robust'], ['crunch', 'robust'], ['tariff', 'robust'], ['workload', 'robust'], ['stated', 'eng'], ['stated', 'edge']];
  for (const [w, d] of combos) {
    const p = mk(w, d), r = runAll(p), f = r.sims.flex, e = r.econ, g = guardrails(p, r, null, []);
    out.worlds_designs[w + '__' + d] = { world: w, design: d, params: toSnake(p),
      flex: { shortE: f.shortE, breachPct: f.breachPct, failed5: f.failed5, thrE: f.thrE, energyCost: f.energyCost, genHrs: f.genHrs },
      econ: { certified: e.certified, F: e.F, capex: e.capex, reduction: e.reduction, Ctotal: e.S.C.total },
      inv: inv(r.inv), compliant: isCompliant(r), guard: g.map(x => x.sev + ':' + x.code), verdictNoMc: verdictOf(g, null).tag };
  }
  for (const [w, d] of [['base', 'robust'], ['base', 'eng'], ['base', 'edge']]) {
    const p = mk(w, d), mc = monteCarlo(p, 24), r = runAll(p), g = guardrails(p, r, mc, []);
    out['mc__' + w + '__' + d] = { n: 24, pCompliant: mc.pCompliant, pCertified: mc.pCertified, npvP10: mc.npvP10, npvP50: mc.npvP50, npvP90: mc.npvP90, shortP50: mc.shortP50, shortP90: mc.shortP90, penaltyMean: mc.penaltyMean, guard: g.map(x => x.sev + ':' + x.code), verdict: verdictOf(g, mc).tag };
  }
  const pb = mk('base', 'robust');
  const o = robustOptimise(pb);
  out.robust = { verified: o.verified, best: o.best, evals: o.evals, holds: o.holds };
  const s = stressSuite(pb);
  out.stress = { matrix: s.matrix.map(r => ({ design: r.design.id, cells: r.cells.map(c => ({ world: c.world, npv: c.npv, compliant: c.compliant, certified: c.certified, short: c.short, capex: c.capex })) })),
    tornado: s.tornado.map(t => ({ k: SNAKE[t.k], lo: t.lo, hi: t.hi, swing: t.swing })), be: s.be.map(b => ({ id: b.id, value: b.value, cur: b.cur })), intensity: s.intensity, baseNpv: s.baseNpv, baseOk: s.baseOk };
  return out;
}

const mode = process.argv[2] || '--check';
const targets = [['tests/golden.json', generate], ['tests/golden_v2.json', generateV2]];
let failed = false;
for (const [file, gen] of targets) {
  const url = new URL(file, root);
  const fresh = JSON.parse(JSON.stringify(gen()));
  if (mode === '--write') {
    writeFileSync(url, JSON.stringify(fresh, null, 1) + '\n');
    console.log(file + ' updated');
    continue;
  }
  const golden = JSON.parse(readFileSync(url, 'utf8'));
  const errs = [];
  compare(fresh, golden, file, errs);
  if (errs.length) {
    failed = true;
    console.error(`web model drifted from ${file} (${errs.length} differences):`);
    errs.slice(0, 20).forEach((e) => console.error('  ' + e));
  } else {
    console.log(`parity OK: web/index.html reproduces ${file}`);
  }
}
if (mode === '--write') console.log('now run: python3 scripts/gen_golden_v2.py, update tests/golden.rs if golden.json changed, and add a CHANGELOG entry');
if (failed) process.exit(1);
