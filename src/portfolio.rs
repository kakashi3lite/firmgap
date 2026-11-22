// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Portfolio tools (model 0.4): the global bankability matrix (`compare-jurisdictions`), deterministic
//! multi-site sweeps (`batch-audit`) and the shareable proof token (`#s=<base64url>` / `audit --share`).
//!
//! Determinism: rulepacks are evaluated in [`JurisdictionRulepack::ALL`] order, files in byte order of
//! their names, and every document is canonical JSON with its own SHA-256. Bounds (hostile input):
//! at most [`MAX_BATCH_FILES`] scenario files per batch, each capped by `scenario::MAX_BYTES`, and at
//! most [`SHARE_MAX_TOKEN`] bytes of share token.

use crate::audit_bundle::{build_core, canonical_inputs, printable, sha256_hex};
use crate::canon::{canonical, num, obj, parse, quantize, qz, s, Value};
use crate::compliance::{compliance_report, site_spec, ComplianceVerdict, JurisdictionRulepack, Severity, SiteInputs, SiteKind};
use crate::error::Error;
use crate::params::{spec, Params};
use crate::scenario;
use crate::sim::{run, Run};
use std::cmp::Ordering;

/// Maximum scenario files in one batch.
pub const MAX_BATCH_FILES: usize = 1000;
/// Maximum share-token length (bytes of base64url text).
pub const SHARE_MAX_TOKEN: usize = 16384;

fn verdict_rank(v: ComplianceVerdict) -> u8 {
    match v {
        ComplianceVerdict::BankablePass => 0,
        ComplianceVerdict::ConditionalWarning => 1,
        ComplianceVerdict::StatutoryBlocker => 2,
    }
}

fn order_index(rp: JurisdictionRulepack) -> usize {
    JurisdictionRulepack::ALL.iter().position(|x| *x == rp).unwrap_or(usize::MAX)
}

/// One row of the bankability matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct CompareRow {
    /// Rulepack.
    pub rulepack: JurisdictionRulepack,
    /// Verdict.
    pub verdict: ComplianceVerdict,
    /// Failed blockers.
    pub blockers: usize,
    /// Failed warnings.
    pub warnings: usize,
    /// Failed rule codes (blockers first, then warnings, each in evaluation order).
    pub failed_rules: Vec<&'static str>,
    /// Security posted ($).
    pub security_locked_usd: f64,
    /// Total financial drag ($).
    pub total_drag_usd: f64,
    /// 1 = most bankable.
    pub rank: usize,
}

/// The global bankability matrix.
#[derive(Debug, Clone)]
pub struct Compare {
    /// Ranked rows.
    pub rows: Vec<CompareRow>,
    /// `scenario_sha256` of the inputs.
    pub scenario_sha256: String,
    /// Canonical value including `matrix_sha256`.
    pub value: Value,
    /// File text (canonical JSON + newline).
    pub text: String,
    /// `matrix_sha256`.
    pub sha256: String,
}

/// Rank every rulepack for one scenario, reusing one simulation run.
pub fn compare_with_run(p: &Params, site: &SiteInputs, scenario_path: &str, r: &Run) -> Compare {
    let st = site.resolve(p);
    let mut rows: Vec<CompareRow> = JurisdictionRulepack::ALL
        .iter()
        .map(|rp| {
            let rep = compliance_report(*rp, &st, p, r);
            let fb: Vec<&'static str> =
                rep.checks.iter().filter(|c| !c.passed && c.severity == Severity::Blocker).map(|c| c.rule_id.code()).collect();
            let fw: Vec<&'static str> =
                rep.checks.iter().filter(|c| !c.passed && c.severity == Severity::Warning).map(|c| c.rule_id.code()).collect();
            let (blockers, warnings) = (fb.len(), fw.len());
            let mut failed_rules = fb;
            failed_rules.extend(fw);
            CompareRow {
                rulepack: *rp,
                verdict: rep.verdict,
                blockers,
                warnings,
                failed_rules,
                security_locked_usd: rep.financial_drag.security_locked_usd,
                total_drag_usd: rep.financial_drag.total_drag_usd,
                rank: 0,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        verdict_rank(a.verdict)
            .cmp(&verdict_rank(b.verdict))
            .then(a.blockers.cmp(&b.blockers))
            .then(a.warnings.cmp(&b.warnings))
            .then(qz(a.total_drag_usd).unwrap_or(0.0).partial_cmp(&qz(b.total_drag_usd).unwrap_or(0.0)).unwrap_or(Ordering::Equal))
            .then(order_index(a.rulepack).cmp(&order_index(b.rulepack)))
    });
    for (i, row) in rows.iter_mut().enumerate() {
        row.rank = i + 1;
    }
    let scenario_sha256 = sha256_hex(canonical(&canonical_inputs(p, &st)).as_bytes());
    let row_values = rows
        .iter()
        .map(|x| {
            obj(vec![
                ("jurisdiction", s(x.rulepack.cli_id())),
                ("rulepack_id", s(x.rulepack.id())),
                ("verdict", s(x.verdict.as_str())),
                ("blockers", num(x.blockers as f64)),
                ("warnings", num(x.warnings as f64)),
                ("failed_rules", Value::Arr(x.failed_rules.iter().copied().map(s).collect())),
                ("security_locked_usd", num(x.security_locked_usd)),
                ("total_drag_usd", num(x.total_drag_usd)),
                ("rank", num(x.rank as f64)),
            ])
        })
        .collect();
    let body = quantize(&obj(vec![
        ("schema", s("firmgap.compare.v1")),
        ("engine_version", s(crate::VERSION)),
        ("scenario_sha256", Value::Str(scenario_sha256.clone())),
        ("repro_command", Value::Str(format!("firmgap compare-jurisdictions --scenario {} --format json", printable(scenario_path)))),
        ("rows", Value::Arr(row_values)),
    ]));
    let sha256 = sha256_hex(canonical(&body).as_bytes());
    let mut value = body;
    if let Value::Obj(m) = &mut value {
        m.insert("matrix_sha256".into(), Value::Str(sha256.clone()));
    }
    let text = canonical(&value) + "\n";
    Compare { rows, scenario_sha256, value, text, sha256 }
}

/// Rank every rulepack for one scenario.
pub fn compare_jurisdictions(p: &Params, site: &SiteInputs, scenario_path: &str) -> Result<Compare, Error> {
    let r = run(p, true)?;
    Ok(compare_with_run(p, site, scenario_path, &r))
}

/// Markdown rendering of the matrix.
pub fn compare_markdown(c: &Compare, scenario_path: &str) -> String {
    let mut o = String::new();
    o.push_str(&format!("## Global bankability matrix: `{}`\n\n", printable(scenario_path)));
    o.push_str("| Rank | Jurisdiction | Rulepack | Verdict | Blockers | Warnings | Failed rules | Security posted | Total drag |\n");
    o.push_str("|---:|---|---|---|---:|---:|---|---:|---:|\n");
    for r in &c.rows {
        o.push_str(&format!(
            "| {} | {} | `{}` | **{}** | {} | {} | {} | {} | {} |\n",
            r.rank,
            r.rulepack.name(),
            r.rulepack.id(),
            r.verdict.as_str(),
            r.blockers,
            r.warnings,
            if r.failed_rules.is_empty() { "-".to_string() } else { r.failed_rules.join(", ") },
            crate::report::money(r.security_locked_usd),
            crate::report::money(r.total_drag_usd)
        ));
    }
    o.push_str(&format!("\n`scenario_sha256` {}  \n`matrix_sha256` {}\n", c.scenario_sha256, c.sha256));
    o
}

/// Which rulepacks a batch evaluates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchSelection {
    /// Every rulepack.
    All,
    /// One rulepack.
    One(JurisdictionRulepack),
    /// The rulepack declared in each file's `jurisdiction` metadata.
    Declared,
}

impl BatchSelection {
    /// Parse `all`, `declared` or a rulepack id.
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim().to_ascii_lowercase().as_str() {
            "all" => Some(Self::All),
            "declared" => Some(Self::Declared),
            other => JurisdictionRulepack::parse(other).map(Self::One),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Declared => "declared",
            Self::One(rp) => rp.cli_id(),
        }
    }
}

/// One rulepack result for one site.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchResult {
    /// Rulepack.
    pub rulepack: JurisdictionRulepack,
    /// Verdict.
    pub verdict: ComplianceVerdict,
    /// Failed blockers.
    pub blockers: usize,
    /// Failed warnings.
    pub warnings: usize,
    /// `dossier_sha256` (identical to `firmgap audit --scenario <file> --jurisdiction <id>`).
    pub dossier_sha256: String,
    /// The result contradicts the file's `expected_verdict`, or is an undeclared blocker.
    pub unexpected: bool,
}

/// One site of a batch.
#[derive(Debug, Clone, PartialEq)]
pub struct BatchSite {
    /// Path as recorded in dossiers (`<dir>/<file>`).
    pub file: String,
    /// `scenario_sha256` (empty on error).
    pub scenario_sha256: String,
    /// Declared metadata.
    pub meta: scenario::ScenarioMeta,
    /// Results in rulepack order.
    pub results: Vec<BatchResult>,
    /// Error text if the file could not be audited.
    pub error: Option<String>,
}

/// A batch audit.
#[derive(Debug, Clone)]
pub struct Batch {
    /// Selection.
    pub selection: BatchSelection,
    /// Sites in file-name order.
    pub sites: Vec<BatchSite>,
    /// Canonical value including `batch_sha256`.
    pub value: Value,
    /// File text.
    pub text: String,
    /// `batch_sha256`.
    pub sha256: String,
}

impl Batch {
    /// Files that failed to load or audit.
    pub fn errors(&self) -> usize {
        self.sites.iter().filter(|s| s.error.is_some()).count()
    }

    /// Results that are undeclared blockers or contradict `expected_verdict`.
    pub fn unexpected(&self) -> usize {
        self.sites.iter().flat_map(|s| s.results.iter()).filter(|r| r.unexpected).count()
    }

    fn count(&self, v: ComplianceVerdict) -> usize {
        self.sites.iter().flat_map(|s| s.results.iter()).filter(|r| r.verdict == v).count()
    }
}

fn valid_name(name: &str) -> bool {
    name.len() <= 120
        && name.ends_with(".toml")
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && !name.starts_with('.')
}

fn audit_one(path: &str, display: &str, sel: BatchSelection) -> Result<BatchSite, Error> {
    let text = scenario::read_file(path)?;
    let pairs = scenario::parse(&text)?;
    let meta = scenario::metadata(&pairs)?;
    let mut p = Params::default();
    let mut site = SiteInputs::default();
    scenario::apply_with_site(&mut p, &mut site, &pairs)?;
    p.validate()?;
    let rps: Vec<JurisdictionRulepack> = match sel {
        BatchSelection::All => JurisdictionRulepack::ALL.to_vec(),
        BatchSelection::One(rp) => vec![rp],
        BatchSelection::Declared => {
            vec![meta.jurisdiction.ok_or_else(|| Error::Usage("scenario has no `jurisdiction` metadata".into()))?]
        }
    };
    let r = run(&p, true)?;
    let mut results = Vec::with_capacity(rps.len());
    let mut scenario_sha256 = String::new();
    for rp in rps {
        let core = build_core(rp, &p, &site, display, &r);
        scenario_sha256 = core.scenario_sha256;
        let blockers = core.report.checks.iter().filter(|c| !c.passed && c.severity == Severity::Blocker).count();
        let warnings = core.report.checks.iter().filter(|c| !c.passed && c.severity == Severity::Warning).count();
        let declared = meta.jurisdiction == Some(rp);
        let verdict = core.report.verdict;
        let mismatch = declared && meta.expected_verdict.is_some_and(|e| e != verdict);
        let undeclared_blocker = verdict == ComplianceVerdict::StatutoryBlocker
            && !(declared && meta.expected_verdict == Some(ComplianceVerdict::StatutoryBlocker));
        results.push(BatchResult {
            rulepack: rp,
            verdict,
            blockers,
            warnings,
            dossier_sha256: core.sha256,
            unexpected: mismatch || undeclared_blocker,
        });
    }
    Ok(BatchSite { file: display.to_string(), scenario_sha256, meta, results, error: None })
}

/// Audit every `*.toml` file directly inside `dir` (not recursive; symlinks and other names skipped).
pub fn batch_audit(dir: &str, sel: BatchSelection) -> Result<Batch, Error> {
    let entries = std::fs::read_dir(dir).map_err(|e| Error::Io(format!("cannot read directory: {}", e.kind())))?;
    let mut names: Vec<String> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| Error::Io(format!("cannot read directory entry: {}", e.kind())))?;
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        let Ok(name) = entry.file_name().into_string() else { continue };
        if !is_file || !valid_name(&name) {
            continue;
        }
        if names.len() >= MAX_BATCH_FILES {
            return Err(Error::Usage(format!("more than {MAX_BATCH_FILES} scenario files in one batch")));
        }
        names.push(name);
    }
    names.sort();
    let base = dir.trim_end_matches(&['/', '\\'][..]);
    let base = if base.is_empty() { "." } else { base };
    let mut sites = Vec::with_capacity(names.len());
    for name in &names {
        let display = format!("{}/{}", printable(base), name);
        let fs_path = std::path::Path::new(dir).join(name);
        let fs_path = fs_path.to_string_lossy();
        match audit_one(&fs_path, &display, sel) {
            Ok(site) => sites.push(site),
            Err(e) => sites.push(BatchSite {
                file: display,
                scenario_sha256: String::new(),
                meta: scenario::ScenarioMeta::default(),
                results: vec![],
                error: Some(e.to_string()),
            }),
        }
    }
    let mut batch = Batch { selection: sel, sites, value: Value::Null, text: String::new(), sha256: String::new() };
    let site_values = batch
        .sites
        .iter()
        .map(|st| {
            obj(vec![
                ("file", Value::Str(st.file.clone())),
                ("scenario_sha256", if st.scenario_sha256.is_empty() { Value::Null } else { Value::Str(st.scenario_sha256.clone()) }),
                ("declared_jurisdiction", st.meta.jurisdiction.map_or(Value::Null, |j| s(j.cli_id()))),
                ("expected_verdict", st.meta.expected_verdict.map_or(Value::Null, |v| s(v.as_str()))),
                ("error", st.error.as_ref().map_or(Value::Null, |e| Value::Str(scenario::sanitize(e)))),
                (
                    "results",
                    Value::Arr(
                        st.results
                            .iter()
                            .map(|r| {
                                obj(vec![
                                    ("jurisdiction", s(r.rulepack.cli_id())),
                                    ("rulepack_id", s(r.rulepack.id())),
                                    ("verdict", s(r.verdict.as_str())),
                                    ("blockers", num(r.blockers as f64)),
                                    ("warnings", num(r.warnings as f64)),
                                    ("dossier_sha256", Value::Str(r.dossier_sha256.clone())),
                                    ("unexpected", Value::Bool(r.unexpected)),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let totals = obj(vec![
        ("sites", num(batch.sites.len() as f64)),
        ("errors", num(batch.errors() as f64)),
        ("bankable_pass", num(batch.count(ComplianceVerdict::BankablePass) as f64)),
        ("conditional_warning", num(batch.count(ComplianceVerdict::ConditionalWarning) as f64)),
        ("statutory_blocker", num(batch.count(ComplianceVerdict::StatutoryBlocker) as f64)),
        ("unexpected", num(batch.unexpected() as f64)),
    ]);
    let body = quantize(&obj(vec![
        ("schema", s("firmgap.batch.v1")),
        ("engine_version", s(crate::VERSION)),
        ("selection", s(sel.label())),
        ("sites", Value::Arr(site_values)),
        ("totals", totals),
    ]));
    let sha = sha256_hex(canonical(&body).as_bytes());
    let mut value = body;
    if let Value::Obj(m) = &mut value {
        m.insert("batch_sha256".into(), Value::Str(sha.clone()));
    }
    batch.text = canonical(&value) + "\n";
    batch.value = value;
    batch.sha256 = sha;
    Ok(batch)
}

/// Markdown rendering of a batch.
pub fn batch_markdown(b: &Batch) -> String {
    let mut o = String::new();
    o.push_str(&format!("## firmgap batch audit ({} sites, rulepacks: {})\n\n", b.sites.len(), b.selection.label()));
    o.push_str("| File | Jurisdiction | Verdict | Blockers | Warnings | Expected | dossier_sha256 |\n|---|---|---|---:|---:|---|---|\n");
    for st in &b.sites {
        if let Some(e) = &st.error {
            o.push_str(&format!("| `{}` | - | **ERROR** | - | - | - | {} |\n", st.file, scenario::sanitize(e)));
            continue;
        }
        for r in &st.results {
            let expected =
                if st.meta.jurisdiction == Some(r.rulepack) { st.meta.expected_verdict.map_or("-", |v| v.as_str()) } else { "-" };
            o.push_str(&format!(
                "| `{}` | {} | **{}**{} | {} | {} | {} | `{}` |\n",
                st.file,
                r.rulepack.cli_id(),
                r.verdict.as_str(),
                if r.unexpected { " (unexpected)" } else { "" },
                r.blockers,
                r.warnings,
                expected,
                &r.dossier_sha256[..16.min(r.dossier_sha256.len())]
            ));
        }
    }
    o.push_str(&format!(
        "\nTotals: {} pass, {} conditional, {} blocker, {} unexpected, {} errors. `batch_sha256` {}\n",
        b.count(ComplianceVerdict::BankablePass),
        b.count(ComplianceVerdict::ConditionalWarning),
        b.count(ComplianceVerdict::StatutoryBlocker),
        b.unexpected(),
        b.errors(),
        b.sha256
    ));
    o
}

// ---------------- shareable proof token ----------------

const B64U: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// base64url encoding without padding.
pub fn b64u_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).map_or(0, |b| u32::from(*b));
        let b2 = chunk.get(2).map_or(0, |b| u32::from(*b));
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(char::from(B64U[((n >> 18) & 63) as usize]));
        out.push(char::from(B64U[((n >> 12) & 63) as usize]));
        if chunk.len() > 1 {
            out.push(char::from(B64U[((n >> 6) & 63) as usize]));
        }
        if chunk.len() > 2 {
            out.push(char::from(B64U[(n & 63) as usize]));
        }
    }
    out
}

fn b64u_val(c: u8) -> Option<u32> {
    B64U.iter().position(|x| *x == c).map(|i| i as u32)
}

/// base64url decoding without padding; `None` on any invalid input.
pub fn b64u_decode(tok: &str) -> Option<Vec<u8>> {
    let b = tok.as_bytes();
    if b.len() > SHARE_MAX_TOKEN || b.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 4 * 3 + 2);
    for chunk in b.chunks(4) {
        let mut v = [0u32; 4];
        for (i, c) in chunk.iter().enumerate() {
            v[i] = b64u_val(*c)?;
        }
        let n = (v[0] << 18) | (v[1] << 12) | (v[2] << 6) | v[3];
        out.push(((n >> 16) & 255) as u8);
        if chunk.len() > 2 {
            out.push(((n >> 8) & 255) as u8);
        }
        if chunk.len() > 3 {
            out.push((n & 255) as u8);
        }
    }
    Some(out)
}

/// A decoded share token.
#[derive(Debug, Clone, PartialEq)]
pub struct SharedScenario {
    /// Scenario path recorded in the dossier's reproduction command.
    pub path: String,
    /// Rulepack.
    pub rulepack: JurisdictionRulepack,
    /// Simulation parameters (validated).
    pub params: Params,
    /// Site inputs (validated).
    pub site: SiteInputs,
    /// `dossier_sha256` the sender saw, if included.
    pub expected_sha256: Option<String>,
}

fn bad(reason: &str) -> Error {
    Error::Usage(format!("invalid share token: {reason}"))
}

/// Safe relative path: `[A-Za-z0-9._/-]`, 1-120 chars, no `..`, not absolute.
pub fn safe_share_path(f: &str) -> bool {
    !f.is_empty()
        && f.len() <= 120
        && !f.contains("..")
        && !f.starts_with('/')
        && f.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'/' || b == b'-')
}

/// Decode a share token, `#s=<token>` fragment or full URL. Strict: unknown keys, wrong types and
/// out-of-range values are errors (the browser clamps before encoding, so its tokens always pass).
pub fn share_decode(input: &str) -> Result<SharedScenario, Error> {
    let t = input.trim();
    let tok = match t.find("#s=") {
        Some(i) => &t[i + 3..],
        None => t.strip_prefix("s=").unwrap_or(t),
    };
    if tok.is_empty() {
        return Err(bad("empty token"));
    }
    if tok.len() > SHARE_MAX_TOKEN {
        return Err(bad("token larger than 16 KiB"));
    }
    let bytes = b64u_decode(tok).ok_or_else(|| bad("not valid base64url"))?;
    if bytes.iter().any(|b| !(32..=126).contains(b)) {
        return Err(bad("payload is not printable ASCII"));
    }
    let json = String::from_utf8(bytes).map_err(|_| bad("payload is not UTF-8"))?;
    let Value::Obj(top) = parse(&json).map_err(|e| bad(&format!("not valid JSON ({e})")))? else {
        return Err(bad("not an object"));
    };
    for k in top.keys() {
        if !["v", "f", "j", "p", "s", "h"].contains(&k.as_str()) {
            return Err(bad(&format!("unknown key {}", scenario::sanitize(k))));
        }
    }
    if top.get("v") != Some(&Value::Num(1.0)) {
        return Err(bad("unsupported token version"));
    }
    let path = match top.get("f") {
        Some(Value::Str(f)) if safe_share_path(f) => f.clone(),
        _ => return Err(bad("invalid scenario path")),
    };
    let rulepack = match top.get("j") {
        Some(Value::Str(j)) => {
            JurisdictionRulepack::ALL.iter().copied().find(|rp| rp.cli_id() == j.as_str()).ok_or_else(|| bad("unknown jurisdiction"))?
        }
        _ => return Err(bad("unknown jurisdiction")),
    };
    let expected_sha256 = match top.get("h") {
        None => None,
        Some(Value::Str(h)) if h.len() == 64 && h.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) => Some(h.clone()),
        Some(_) => return Err(bad("malformed hash")),
    };
    let (Some(Value::Obj(pm)), Some(Value::Obj(sm))) = (top.get("p"), top.get("s")) else {
        return Err(bad("missing inputs"));
    };
    let mut params = Params::default();
    for (k, v) in pm {
        match (k.as_str(), v) {
            ("strategy", Value::Str(st)) if st == "throttle" || st == "bess" => params.set("strategy", st)?,
            ("strategy", _) => return Err(bad("invalid strategy")),
            ("seed", Value::Num(n)) if n.fract() == 0.0 && (0.0..=4_294_967_295.0).contains(n) => params.set("seed", &format!("{n:.0}"))?,
            ("seed", _) => return Err(bad("invalid seed")),
            (key, Value::Num(n)) if spec(key).is_some() => params.set_f64(key, *n)?,
            (key, _) if spec(key).is_some() => return Err(bad(&format!("non-numeric parameter {key}"))),
            (key, _) => return Err(bad(&format!("unknown parameter {}", scenario::sanitize(key)))),
        }
    }
    let mut site = SiteInputs::default();
    for (k, v) in sm {
        let sp = site_spec(k).ok_or_else(|| bad(&format!("unknown site key {}", scenario::sanitize(k))))?;
        match (sp.kind, v) {
            (SiteKind::Bool, Value::Bool(b)) => site.set_value(sp.key, if *b { 1.0 } else { 0.0 })?,
            (SiteKind::Num, Value::Num(n)) => site.set_value(sp.key, *n)?,
            _ => return Err(bad(&format!("wrong type for {}", sp.key))),
        }
    }
    params.validate()?;
    Ok(SharedScenario { path, rulepack, params, site, expected_sha256 })
}

/// Encode a share token for a scenario (identical bytes to the browser's `shareToken`).
pub fn share_encode(rp: JurisdictionRulepack, p: &Params, site: &SiteInputs, path: &str) -> Result<(String, String), Error> {
    if !safe_share_path(path) {
        return Err(Error::Usage("share path must be a relative path of [A-Za-z0-9._/-]".into()));
    }
    let r = run(p, true)?;
    let core = build_core(rp, p, site, path, &r);
    let inputs = core.value.get("inputs").cloned().unwrap_or(Value::Null);
    let body = quantize(&obj(vec![
        ("v", num(1.0)),
        ("f", s(path)),
        ("j", s(rp.cli_id())),
        ("p", inputs.get("params").cloned().unwrap_or(Value::Null)),
        ("s", inputs.get("site").cloned().unwrap_or(Value::Null)),
        ("h", Value::Str(core.sha256.clone())),
    ]));
    Ok((b64u_encode(canonical(&body).as_bytes()), core.sha256))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64url_roundtrip_and_rejects() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(b64u_decode(&b64u_encode(&data)), Some(data));
        }
        assert_eq!(b64u_encode(b"abc"), "YWJj");
        assert_eq!(b64u_encode(b"ab"), "YWI");
        assert!(b64u_decode("A").is_none());
        assert!(b64u_decode("ab+/").is_none());
        assert!(b64u_decode(&"A".repeat(SHARE_MAX_TOKEN + 4)).is_none());
    }

    #[test]
    fn hostile_tokens_fail_cleanly() {
        let enc = |j: &str| format!("#s={}", b64u_encode(j.as_bytes()));
        for t in [
            String::new(),
            "#s=".into(),
            "#s=@@@".into(),
            enc("[]"),
            enc("{\"v\":1,\"__proto__\":{}}"),
            enc("{\"v\":2}"),
            enc("{\"v\":1,\"f\":\"../x\",\"j\":\"ercot-sb6\",\"p\":{},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"/abs.toml\",\"j\":\"ercot-sb6\",\"p\":{},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"mars\",\"p\":{},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{\"bess_mw\":\"1\"},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{\"bess_mw\":1e9},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{\"evil\":1},\"s\":{}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{},\"s\":{\"firm_shed_telemetry\":1}}"),
            enc("{\"v\":1,\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{},\"s\":{},\"h\":\"XYZ\"}"),
            format!("#s={}", "A".repeat(SHARE_MAX_TOKEN + 8)),
        ] {
            assert!(share_decode(&t).is_err(), "accepted {t:?}");
        }
        let ok = enc("{\"f\":\"a.toml\",\"j\":\"ercot-sb6\",\"p\":{\"bess_mw\":20},\"s\":{\"gross_peak_mw\":80},\"v\":1}");
        let sc = share_decode(&format!("https://example.invalid/web/index.html{ok}"));
        assert!(matches!(sc, Ok(ref x) if x.params.bess_mw == 20.0 && x.rulepack == JurisdictionRulepack::ErcotSb6_2026));
    }

    #[test]
    fn batch_names_are_restricted() {
        assert!(valid_name("site-01.toml"));
        assert!(!valid_name(".hidden.toml"));
        assert!(!valid_name("a b.toml"));
        assert!(!valid_name("x.txt"));
    }
}
