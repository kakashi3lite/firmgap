// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Tamper-evident compliance dossier.
//!
//! * [`Sha256`]: a pure-`std` FIPS 180-4 SHA-256 (tested against the NIST vectors).
//! * [`build_dossier`]: inputs, shedding curve and compliance report in one canonical JSON body.
//! * `dossier_sha256 = SHA-256(canonical JSON of the body without dossier_sha256)`.
//!
//! Verification in three lines of Node.js (numbers are already canonical in the file):
//!
//! ```text
//! const d = JSON.parse(require('fs').readFileSync('dossier.json', 'utf8')), h = d.dossier_sha256; delete d.dossier_sha256;
//! const c = (v) => v === null || typeof v !== 'object' ? JSON.stringify(v) : Array.isArray(v) ? '[' + v.map(c).join(',') + ']' : '{' + Object.keys(v).sort().map(k => JSON.stringify(k) + ':' + c(v[k])).join(',') + '}';
//! console.log(require('crypto').createHash('sha256').update(c(d)).digest('hex') === h);
//! ```

use crate::canon::{canonical, num, obj, parse, quantize, s, Value, MAX_JSON_BYTES};
use crate::compliance::{compliance_report, ComplianceReport, JurisdictionRulepack, Site, SiteInputs};
use crate::error::Error;
use crate::params::{Params, SPECS};
use crate::shed_curve::{shed_curve, SheddingCapabilityCurve};
use crate::sim::{run, Run};
use std::io::Read;

/// Dossier schema version.
pub const SCHEMA_VERSION: &str = "1.0.0";

/// Incremental SHA-256 (FIPS 180-4).
#[derive(Debug, Clone)]
pub struct Sha256 {
    h: [u32; 8],
    buf: [u8; 64],
    len: usize,
    total: u64,
}

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
    0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
    0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
    0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
    0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sha256 {
    /// New hasher.
    pub fn new() -> Self {
        Self {
            h: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
            buf: [0; 64],
            len: 0,
            total: 0,
        }
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (x, v) in self.h.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *x = x.wrapping_add(v);
        }
    }

    /// Absorb bytes.
    pub fn update(&mut self, data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        for &byte in data {
            self.buf[self.len] = byte;
            self.len += 1;
            if self.len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.len = 0;
            }
        }
    }

    /// Finish and return the 32-byte digest.
    pub fn finalize(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        while self.len != 56 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (i, word) in self.h.iter().enumerate() {
            out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
        }
        out
    }
}

/// Lowercase hex encoding.
pub fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(HEX[usize::from(b >> 4)]));
        out.push(char::from(HEX[usize::from(b & 0x0f)]));
    }
    out
}

/// Lowercase hex SHA-256 of bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    to_hex(&h.finalize())
}

/// Canonical inputs: every simulation parameter plus every resolved site field.
pub fn canonical_inputs(p: &Params, st: &Site) -> Value {
    let mut params = Vec::new();
    for sp in SPECS {
        params.push((sp.key, num(p.get(sp.key).unwrap_or(f64::NAN))));
    }
    params.push(("seed", num(f64::from(p.seed))));
    params.push(("strategy", s(p.strategy.as_str())));
    quantize(&obj(vec![("params", obj(params)), ("site", st.to_value())]))
}

/// The compliance dossier (alias used in the documentation and schema).
pub type ComplianceDossier = Dossier;

/// A built dossier.
#[derive(Debug, Clone)]
pub struct Dossier {
    /// Canonical dossier value (including `dossier_sha256`).
    pub value: Value,
    /// File text: canonical JSON + newline.
    pub text: String,
    /// `dossier_sha256`.
    pub sha256: String,
    /// `scenario_sha256`.
    pub scenario_sha256: String,
    /// Compliance report.
    pub report: ComplianceReport,
    /// Shedding capability curve.
    pub curve: SheddingCapabilityCurve,
    /// Resolved site.
    pub site: Site,
    /// Simulation run.
    pub run: Run,
}

/// Replace every non-printable-ASCII character by '?' per UTF-16 code unit (identical to the
/// JavaScript reference `asciiOnly`).
pub(crate) fn printable(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if (' '..='~').contains(&c) {
            out.push(c);
        } else {
            for _ in 0..c.len_utf16() {
                out.push('?');
            }
        }
    }
    out
}

/// Build a dossier for a rulepack. `scenario_path` is recorded verbatim (non-printable characters
/// replaced) in the reproduction command.
pub fn build_dossier(rp: JurisdictionRulepack, p: &Params, site: &SiteInputs, scenario_path: &str) -> Result<Dossier, Error> {
    let r = run(p, true)?;
    Ok(build_dossier_with_run(rp, p, site, scenario_path, r))
}

/// Build a dossier from a completed run (`run(p, true)`).
pub fn build_dossier_with_run(rp: JurisdictionRulepack, p: &Params, site: &SiteInputs, scenario_path: &str, r: Run) -> Dossier {
    let c = build_core(rp, p, site, scenario_path, &r);
    Dossier {
        value: c.value,
        text: c.text,
        sha256: c.sha256,
        scenario_sha256: c.scenario_sha256,
        report: c.report,
        curve: c.curve,
        site: c.site,
        run: r,
    }
}

/// A dossier without the simulation run attached (cheap to build many times from one run).
#[derive(Debug, Clone)]
pub struct DossierCore {
    /// Canonical dossier value (including `dossier_sha256`).
    pub value: Value,
    /// File text: canonical JSON + newline.
    pub text: String,
    /// `dossier_sha256`.
    pub sha256: String,
    /// `scenario_sha256`.
    pub scenario_sha256: String,
    /// Compliance report.
    pub report: ComplianceReport,
    /// Shedding capability curve.
    pub curve: SheddingCapabilityCurve,
    /// Resolved site.
    pub site: Site,
}

/// Build a dossier body and hash from a borrowed run, so one simulation can back several rulepacks
/// (`compare-jurisdictions`, `batch-audit`).
pub fn build_core(rp: JurisdictionRulepack, p: &Params, site: &SiteInputs, scenario_path: &str, r: &Run) -> DossierCore {
    let st = site.resolve(p);
    let curve = shed_curve(&st, p);
    let report = compliance_report(rp, &st, p, r);
    let inputs = canonical_inputs(p, &st);
    let scenario_sha256 = sha256_hex(canonical(&inputs).as_bytes());
    let body = quantize(&obj(vec![
        ("schema_version", s(SCHEMA_VERSION)),
        ("engine_version", s(crate::VERSION)),
        ("rulepack_id", s(report.rulepack_id)),
        ("jurisdiction", s(rp.cli_id())),
        ("scenario_sha256", Value::Str(scenario_sha256.clone())),
        ("inputs", inputs),
        ("shedding_curve", curve.to_value()),
        ("compliance", report.to_value()),
        ("repro_command", Value::Str(format!("firmgap audit --scenario {} --jurisdiction {}", printable(scenario_path), rp.cli_id()))),
        ("seed", num(f64::from(p.seed))),
    ]));
    let sha256 = sha256_hex(canonical(&body).as_bytes());
    let mut value = body;
    if let Value::Obj(m) = &mut value {
        m.insert("dossier_sha256".into(), Value::Str(sha256.clone()));
    }
    let text = canonical(&value) + "\n";
    DossierCore { value, text, sha256, scenario_sha256, report, curve, site: st }
}

/// Outcome of verifying a dossier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    /// Hash in the file.
    pub claimed: String,
    /// Hash recomputed from the canonical body.
    pub actual: String,
    /// `claimed == actual`.
    pub ok: bool,
    /// Engine version recorded in the dossier.
    pub engine_version: String,
    /// Rulepack recorded in the dossier.
    pub rulepack_id: String,
}

/// Verify dossier text: recompute the canonical SHA-256 of the body without `dossier_sha256`.
pub fn verify_text(text: &str) -> Result<Verification, Error> {
    let v = parse(text).map_err(|e| Error::Integrity(format!("not a valid dossier JSON document: {e}")))?;
    let Value::Obj(mut m) = v else {
        return Err(Error::Integrity("dossier must be a JSON object".into()));
    };
    let claimed = match m.remove("dossier_sha256") {
        Some(Value::Str(h)) if h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) => h,
        _ => return Err(Error::Integrity("missing or malformed dossier_sha256".into())),
    };
    let engine_version = m.get("engine_version").and_then(Value::as_str).unwrap_or("unknown").to_string();
    let rulepack_id = m.get("rulepack_id").and_then(Value::as_str).unwrap_or("unknown").to_string();
    let actual = sha256_hex(canonical(&Value::Obj(m)).as_bytes());
    Ok(Verification { ok: actual == claimed, claimed, actual, engine_version, rulepack_id })
}

/// Read a dossier file with a hard size cap.
pub fn read_dossier(path: &str) -> Result<String, Error> {
    let f = std::fs::File::open(path).map_err(|e| Error::Io(format!("cannot open dossier: {}", e.kind())))?;
    let mut buf = String::new();
    f.take(MAX_JSON_BYTES as u64 + 1)
        .read_to_string(&mut buf)
        .map_err(|e| Error::Io(format!("cannot read dossier (must be UTF-8 JSON): {}", e.kind())))?;
    if buf.len() > MAX_JSON_BYTES {
        return Err(Error::Integrity(format!("dossier exceeds {MAX_JSON_BYTES} bytes")));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nist_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        let million = vec![b'a'; 1_000_000];
        assert_eq!(sha256_hex(&million), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
    }

    #[test]
    fn incremental_equals_one_shot() {
        let data: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        for split in [0, 1, 55, 56, 63, 64, 65, 999] {
            let mut h = Sha256::new();
            h.update(&data[..split]);
            h.update(&data[split..]);
            assert_eq!(to_hex(&h.finalize()), sha256_hex(&data));
        }
    }

    #[test]
    fn verify_rejects_garbage() {
        assert!(verify_text("[]").is_err());
        assert!(verify_text("{\"dossier_sha256\":\"xyz\"}").is_err());
        let ok = verify_text("{\"a\":1,\"dossier_sha256\":\"0000000000000000000000000000000000000000000000000000000000000000\"}");
        assert!(matches!(ok, Ok(v) if !v.ok));
    }
}
