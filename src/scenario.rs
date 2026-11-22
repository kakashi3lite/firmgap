// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Scenario files: a flat, TOML-compatible `key = value` format.
//!
//! ```toml
//! # Right-sized hybrid bridge
//! preset = "case"          # optional, applied first
//! bess_mw = 13.5
//! bess_h = 2
//! max_throttle_h = 12
//! strategy = "throttle"
//! ```
//!
//! Hardening (scenario files are untrusted input):
//! * hard limits on file size, line count and line length;
//! * only `[a-z0-9_]` keys, each at most once;
//! * every value goes through [`Params::set`], which enforces type, finiteness
//!   and range; the final set is checked by [`Params::validate`];
//! * no includes, no environment expansion, no paths, no code.
//! * compliance site keys (booleans `true` / `false` and ranged numbers) are accepted alongside
//!   simulation parameters; see `firmgap audit`.

use crate::compliance::{ComplianceVerdict, JurisdictionRulepack, SiteInputs};
use crate::error::Error;
use crate::params::Params;
use crate::presets;
use std::io::Read;

/// Maximum scenario file size in bytes.
pub const MAX_BYTES: usize = 64 * 1024;
/// Maximum number of lines.
pub const MAX_LINES: usize = 512;
/// Maximum characters per line.
pub const MAX_LINE_LEN: usize = 512;

/// Parse scenario text into ordered `(key, value)` pairs without applying them.
pub fn parse(text: &str) -> Result<Vec<(String, String)>, Error> {
    if text.len() > MAX_BYTES {
        return Err(Error::Parse { line: 0, message: format!("file exceeds {MAX_BYTES} bytes") });
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        if line_no > MAX_LINES {
            return Err(Error::Parse { line: line_no, message: format!("more than {MAX_LINES} lines") });
        }
        if raw.chars().count() > MAX_LINE_LEN {
            return Err(Error::Parse { line: line_no, message: format!("line longer than {MAX_LINE_LEN} characters") });
        }
        let line = strip_comment(raw).trim();
        if line.is_empty() || line == "[scenario]" {
            continue;
        }
        let (k, v) = line.split_once('=').ok_or_else(|| Error::Parse { line: line_no, message: "expected `key = value`".into() })?;
        let k = k.trim();
        let v = v.trim();
        if k.is_empty() || !k.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_') {
            return Err(Error::Parse { line: line_no, message: format!("invalid key '{}'", sanitize(k)) });
        }
        if v.is_empty() {
            return Err(Error::Parse { line: line_no, message: format!("missing value for '{k}'") });
        }
        if out.iter().any(|(ek, _)| ek == k) {
            return Err(Error::Parse { line: line_no, message: format!("duplicate key '{k}'") });
        }
        out.push((k.to_string(), v.to_string()));
    }
    Ok(out)
}

/// Remove a `#` comment that is not inside a quoted string.
fn strip_comment(line: &str) -> &str {
    let mut in_q: Option<char> = None;
    for (i, ch) in line.char_indices() {
        match (in_q, ch) {
            (None, '"') | (None, '\'') => in_q = Some(ch),
            (Some(q), c) if c == q => in_q = None,
            (None, '#') => return &line[..i],
            _ => {}
        }
    }
    line
}

/// Make arbitrary text safe to echo in an error: printable ASCII only,
/// at most 160 characters.
pub fn sanitize(s: &str) -> String {
    s.chars().take(160).map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '?' }).collect()
}

/// Apply parsed pairs onto `p`. A `preset` key is applied before all others. Compliance site keys
/// (see [`crate::compliance::SITE_SPECS`]) are validated and then ignored; use [`apply_with_site`]
/// to keep them.
pub fn apply(p: &mut Params, pairs: &[(String, String)]) -> Result<(), Error> {
    let mut site = SiteInputs::default();
    apply_with_site(p, &mut site, pairs)
}

/// Scenario metadata keys: validated, never part of the simulation inputs or the dossier hash.
pub const META_KEYS: [&str; 2] = ["jurisdiction", "expected_verdict"];

/// Optional scenario metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScenarioMeta {
    /// Default rulepack for `firmgap audit` (`jurisdiction = "ercot-sb6"`).
    pub jurisdiction: Option<JurisdictionRulepack>,
    /// Verdict the file is expected to produce under that rulepack (fixtures and demo blockers).
    pub expected_verdict: Option<ComplianceVerdict>,
}

/// Read and validate the metadata keys.
pub fn metadata(pairs: &[(String, String)]) -> Result<ScenarioMeta, Error> {
    let mut m = ScenarioMeta::default();
    for (k, v) in pairs {
        let value = v.trim().trim_matches('"').trim_matches('\'');
        match k.as_str() {
            "jurisdiction" => {
                m.jurisdiction = Some(JurisdictionRulepack::parse(value).ok_or_else(|| Error::InvalidValue {
                    key: k.clone(),
                    value: value.to_string(),
                    reason: "unknown jurisdiction (run `firmgap rulepacks`)",
                })?);
            }
            "expected_verdict" => {
                m.expected_verdict = Some(ComplianceVerdict::parse(value).ok_or_else(|| Error::InvalidValue {
                    key: k.clone(),
                    value: value.to_string(),
                    reason: "expected BankablePass, ConditionalWarning or StatutoryBlocker",
                })?);
            }
            _ => {}
        }
    }
    Ok(m)
}

/// Apply parsed pairs onto simulation parameters and compliance site inputs. Metadata keys are
/// validated and skipped.
pub fn apply_with_site(p: &mut Params, site: &mut SiteInputs, pairs: &[(String, String)]) -> Result<(), Error> {
    metadata(pairs)?;
    if let Some((_, v)) = pairs.iter().find(|(k, _)| k == "preset") {
        let name = v.trim().trim_matches('"').trim_matches('\'');
        presets::apply(p, name)?;
    }
    for (k, v) in pairs.iter().filter(|(k, _)| k != "preset" && !META_KEYS.contains(&k.as_str())) {
        if SiteInputs::is_site_key(k) {
            site.set(k, v)?;
        } else {
            p.set(k, v)?;
        }
    }
    Ok(())
}

/// Read a scenario file with a hard size cap (never reads more than `MAX_BYTES + 1`).
pub fn read_file(path: &str) -> Result<String, Error> {
    let f = std::fs::File::open(path).map_err(|e| Error::Io(format!("cannot open scenario file: {}", e.kind())))?;
    let mut buf = String::new();
    f.take(MAX_BYTES as u64 + 1)
        .read_to_string(&mut buf)
        .map_err(|e| Error::Io(format!("cannot read scenario file (must be UTF-8 text): {}", e.kind())))?;
    if buf.len() > MAX_BYTES {
        return Err(Error::Parse { line: 0, message: format!("file exceeds {MAX_BYTES} bytes") });
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_applies() {
        let txt = "# comment\n[scenario]\npreset = \"right-sized\"\nseed = 7 # inline\nstrategy = 'bess'\n";
        let pairs = parse(txt).unwrap();
        let mut p = Params::default();
        apply(&mut p, &pairs).unwrap();
        assert_eq!(p.bess_mw, 13.5);
        assert_eq!(p.seed, 7);
        p.validate().unwrap();
    }

    #[test]
    fn metadata_is_validated_and_kept_out_of_inputs() {
        let pairs = parse("jurisdiction = \"es-rd\"  # meta\nexpected_verdict = \"StatutoryBlocker\"\nbess_mw = 20\n").unwrap_or_default();
        let m = metadata(&pairs);
        assert!(matches!(
            m,
            Ok(ScenarioMeta {
                jurisdiction: Some(JurisdictionRulepack::SpainRd2026),
                expected_verdict: Some(ComplianceVerdict::StatutoryBlocker)
            })
        ));
        let mut p = Params::default();
        let mut site = SiteInputs::default();
        assert!(apply_with_site(&mut p, &mut site, &pairs).is_ok());
        assert_eq!(p.bess_mw, 20.0);
        let bad = parse("jurisdiction = \"mars\"\n").unwrap_or_default();
        assert!(apply_with_site(&mut p, &mut site, &bad).is_err());
        let bad = parse("expected_verdict = \"Fine\"\n").unwrap_or_default();
        assert!(metadata(&bad).is_err());
    }

    #[test]
    fn rejects_hostile_input() {
        assert!(parse("bess_mw").is_err());
        assert!(parse("BESS_MW = 1").is_err());
        assert!(parse("bess_mw = 1\nbess_mw = 2").is_err());
        assert!(parse("bess_mw =").is_err());
        assert!(parse(&"a".repeat(MAX_BYTES + 1)).is_err());
        assert!(parse(&format!("x = {}", "9".repeat(MAX_LINE_LEN))).is_err());
        let mut p = Params::default();
        assert!(apply(&mut p, &parse("bess_mw = 1e400").unwrap()).is_err());
        assert!(apply(&mut p, &parse("rm_rf = 1").unwrap()).is_err());
    }

    #[test]
    fn hash_inside_quotes_is_kept() {
        assert_eq!(strip_comment("a = \"x#y\" # c"), "a = \"x#y\" ");
    }
}
