// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Canonical JSON: a small, dependency-free value model, a hardened parser and a serializer that is
//! byte-identical to JavaScript's `JSON.stringify` for the values firmgap emits.
//!
//! Canonical form (used for every hash in an audit dossier):
//! * objects are emitted with keys in byte order ([`BTreeMap`]), with no whitespace;
//! * numbers use the ECMAScript `Number::toString` algorithm (shortest round-trip digits, the same
//!   fixed / exponent switch points), `-0` becomes `0` and non-finite values become `null`;
//! * strings escape `"`, `\`, `\b \f \n \r \t` and other control characters as `\u00xx`
//!   (lowercase), exactly like `JSON.stringify`;
//! * numbers placed into a dossier are first quantised to 1e-4 by [`qz`] (round half up), which
//!   absorbs last-ulp differences between platform math libraries.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A JSON value. Objects are ordered by key bytes, so serialization is deterministic.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// `null`.
    Null,
    /// `true` / `false`.
    Bool(bool),
    /// A number (always f64, as in JavaScript).
    Num(f64),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<Value>),
    /// An object with byte-ordered keys.
    Obj(BTreeMap<String, Value>),
}

impl Value {
    /// Object member lookup.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(m) => m.get(key),
            _ => None,
        }
    }

    /// String content, if this is a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Builder for objects: `obj(vec![("a", Value::Num(1.0))])`.
pub fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = BTreeMap::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Obj(m)
}

/// Number value, `null` when not finite.
pub fn num(v: f64) -> Value {
    if v.is_finite() {
        Value::Num(v)
    } else {
        Value::Null
    }
}

/// Optional number value.
pub fn opt(v: Option<f64>) -> Value {
    v.map_or(Value::Null, num)
}

/// String value.
pub fn s(v: &str) -> Value {
    Value::Str(v.to_string())
}

/// Round half up (toward +infinity), identical to the JavaScript reference `roundHalfUp`.
pub fn round_half_up(q: f64) -> f64 {
    let f = q.floor();
    if q - f >= 0.5 {
        f + 1.0
    } else {
        f
    }
}

/// Quantise to 1e-4 (round half up); `None` for non-finite input. Values whose scaled magnitude
/// exceeds 4.5e15 are returned unchanged (they are already beyond 1e-4 resolution).
pub fn qz(x: f64) -> Option<f64> {
    if !x.is_finite() {
        return None;
    }
    let q = x * 1e4;
    if q.abs() >= 4.5e15 {
        return Some(if x == 0.0 { 0.0 } else { x });
    }
    let v = round_half_up(q) / 1e4;
    Some(if v == 0.0 { 0.0 } else { v })
}

/// Quantise every number in a value tree (the JavaScript `canonValue`).
pub fn quantize(v: &Value) -> Value {
    match v {
        Value::Num(x) => opt(qz(*x)),
        Value::Arr(a) => Value::Arr(a.iter().map(quantize).collect()),
        Value::Obj(m) => Value::Obj(m.iter().map(|(k, v)| (k.clone(), quantize(v))).collect()),
        other => other.clone(),
    }
}

/// ECMAScript `Number.prototype.toString()` for finite values; `null` otherwise.
pub fn js_number(v: f64) -> String {
    if !v.is_finite() {
        return "null".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    let neg = v < 0.0;
    // Rust's `{:e}` prints the shortest round-trip digits, the same digit string ECMAScript uses.
    let e_repr = format!("{:e}", v.abs());
    let (mant, exp) = match e_repr.split_once('e') {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (e_repr.as_str(), 0),
    };
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let body = if k <= n && n <= 21 {
        let mut s = digits.clone();
        for _ in 0..(n - k) {
            s.push('0');
        }
        s
    } else if 0 < n && n <= 21 {
        let (a, b) = digits.split_at(n as usize);
        format!("{a}.{b}")
    } else if -6 < n && n <= 0 {
        let mut s = String::from("0.");
        for _ in 0..(-n) {
            s.push('0');
        }
        s.push_str(&digits);
        s
    } else {
        let e = n - 1;
        let sign = if e < 0 { '-' } else { '+' };
        if k == 1 {
            format!("{digits}e{sign}{}", e.abs())
        } else {
            let (a, b) = digits.split_at(1);
            format!("{a}.{b}e{sign}{}", e.abs())
        }
    };
    if neg {
        format!("-{body}")
    } else {
        body
    }
}

fn push_js_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Canonical (compact, key-sorted, `JSON.stringify`-compatible) serialization.
pub fn canonical(v: &Value) -> String {
    let mut out = String::new();
    write_canonical(&mut out, v);
    out
}

fn write_canonical(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Num(x) => out.push_str(&js_number(*x)),
        Value::Str(s) => push_js_string(out, s),
        Value::Arr(a) => {
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(out, x);
            }
            out.push(']');
        }
        Value::Obj(m) => {
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                push_js_string(out, k);
                out.push(':');
                write_canonical(out, x);
            }
            out.push('}');
        }
    }
}

/// Maximum accepted JSON document size (bytes).
pub const MAX_JSON_BYTES: usize = 4 * 1024 * 1024;
/// Maximum nesting depth.
pub const MAX_DEPTH: usize = 64;

/// Parse a JSON document (RFC 8259) with size and depth limits. Duplicate object keys are rejected.
pub fn parse(text: &str) -> Result<Value, String> {
    if text.len() > MAX_JSON_BYTES {
        return Err(format!("document exceeds {MAX_JSON_BYTES} bytes"));
    }
    let mut p = Parser { b: text.as_bytes(), i: 0 };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.b.len() {
        return Err(format!("trailing characters at byte {}", p.i));
    }
    Ok(v)
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }

    fn literal(&mut self, lit: &[u8]) -> Result<(), String> {
        if self.b[self.i..].starts_with(lit) {
            self.i += lit.len();
            Ok(())
        } else {
            Err(format!("invalid literal at byte {}", self.i))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err("nesting too deep".into());
        }
        match self.peek() {
            Some(b'n') => self.literal(b"null").map(|_| Value::Null),
            Some(b't') => self.literal(b"true").map(|_| Value::Bool(true)),
            Some(b'f') => self.literal(b"false").map(|_| Value::Bool(false)),
            Some(b'"') => self.string().map(Value::Str),
            Some(b'[') => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                    return Ok(Value::Arr(a));
                }
                loop {
                    self.ws();
                    a.push(self.value(depth + 1)?);
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(Value::Arr(a));
                        }
                        _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                    }
                }
            }
            Some(b'{') => {
                self.i += 1;
                let mut m = BTreeMap::new();
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                    return Ok(Value::Obj(m));
                }
                loop {
                    self.ws();
                    if self.peek() != Some(b'"') {
                        return Err(format!("expected object key at byte {}", self.i));
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.peek() != Some(b':') {
                        return Err(format!("expected ':' at byte {}", self.i));
                    }
                    self.i += 1;
                    self.ws();
                    let v = self.value(depth + 1)?;
                    if m.insert(k, v).is_some() {
                        return Err("duplicate object key".into());
                    }
                    self.ws();
                    match self.peek() {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(Value::Obj(m));
                        }
                        _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                    }
                }
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(format!("unexpected character at byte {}", self.i)),
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        match self.peek() {
            Some(b'0') => self.i += 1,
            Some(c) if c.is_ascii_digit() => {
                while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            _ => return Err(format!("invalid number at byte {start}")),
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            let d0 = self.i;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
            if self.i == d0 {
                return Err(format!("invalid number at byte {start}"));
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.i += 1;
            }
            let d0 = self.i;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
            if self.i == d0 {
                return Err(format!("invalid number at byte {start}"));
            }
        }
        let txt = std::str::from_utf8(&self.b[start..self.i]).map_err(|_| "invalid number".to_string())?;
        let v: f64 = txt.parse().map_err(|_| format!("invalid number at byte {start}"))?;
        if !v.is_finite() {
            return Err(format!("number out of range at byte {start}"));
        }
        Ok(Value::Num(v))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        if self.i + 4 > self.b.len() {
            return Err("truncated \\u escape".into());
        }
        let h = std::str::from_utf8(&self.b[self.i..self.i + 4]).map_err(|_| "bad \\u escape".to_string())?;
        let v = u32::from_str_radix(h, 16).map_err(|_| "bad \\u escape".to_string())?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, String> {
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = self.peek().ok_or_else(|| "unterminated string".to_string())?;
            match c {
                b'"' => {
                    self.i += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.i += 1;
                    let e = self.peek().ok_or_else(|| "unterminated escape".to_string())?;
                    self.i += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let cp = if (0xd800..0xdc00).contains(&hi) {
                                if self.b.get(self.i) == Some(&b'\\') && self.b.get(self.i + 1) == Some(&b'u') {
                                    self.i += 2;
                                    let lo = self.hex4()?;
                                    if !(0xdc00..0xe000).contains(&lo) {
                                        return Err("invalid surrogate pair".into());
                                    }
                                    0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00)
                                } else {
                                    return Err("lone surrogate".into());
                                }
                            } else if (0xdc00..0xe000).contains(&hi) {
                                return Err("lone surrogate".into());
                            } else {
                                hi
                            };
                            out.push(char::from_u32(cp).ok_or_else(|| "invalid code point".to_string())?);
                        }
                        _ => return Err("invalid escape".into()),
                    }
                }
                c if c < 0x20 => return Err("control character in string".into()),
                _ => {
                    // copy one UTF-8 scalar (input came from &str, so it is valid; decode only this char)
                    let len = match c {
                        0x00..=0x7f => 1,
                        0xc0..=0xdf => 2,
                        0xe0..=0xef => 3,
                        _ => 4,
                    };
                    let end = (self.i + len).min(self.b.len());
                    let ch = std::str::from_utf8(&self.b[self.i..end]).map_err(|_| "invalid UTF-8".to_string())?;
                    out.push_str(ch);
                    self.i = end;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_number_matches_ecmascript() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-2.5, "-2.5"),
            (100.0, "100"),
            (0.1, "0.1"),
            (0.30000000000000004, "0.30000000000000004"),
            (123456789.0, "123456789"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (1e20, "100000000000000000000"),
            (0.000001, "0.000001"),
            (1e-7, "1e-7"),
            (1.25e-7, "1.25e-7"),
            (66649148.347, "66649148.347"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
        ];
        for (v, want) in cases {
            assert_eq!(js_number(*v), *want, "{v:e}");
        }
        assert_eq!(js_number(f64::NAN), "null");
    }

    #[test]
    fn quantize_rounds_half_up() {
        // expected values come from the JavaScript reference
        assert_eq!(qz(1.00005), Some(1.0001));
        assert_eq!(qz(-1.00005), Some(-1.0001));
        assert_eq!(qz(1.23456), Some(1.2346));
        assert_eq!(qz(-1.23455), Some(-1.2345));
        assert_eq!(qz(0.00004999), Some(0.0));
        assert_eq!(qz(-0.00001), Some(0.0));
        assert_eq!(qz(2.0), Some(2.0));
        assert_eq!(qz(f64::INFINITY), None);
        assert_eq!(round_half_up(2.5), 3.0);
        assert_eq!(round_half_up(-2.5), -2.0);
    }

    #[test]
    fn canonical_roundtrip_and_escaping() {
        let v = obj(vec![("b", Value::Arr(vec![num(1.5), Value::Null, Value::Bool(true)])), ("a", s("x\"y\\z\n\u{1}\u{2028}é"))]);
        let c = canonical(&v);
        assert_eq!(c, "{\"a\":\"x\\\"y\\\\z\\n\\u0001\u{2028}é\",\"b\":[1.5,null,true]}");
        assert_eq!(parse(&c), Ok(v));
    }

    #[test]
    fn parser_rejects_hostile_input() {
        for bad in ["", "{", "[1,]", "{\"a\":1,\"a\":2}", "01", "1.", "\"\\ud800\"", "nul", "[1] x", "1e999", "\"a\u{1}\""] {
            assert!(parse(bad).is_err(), "accepted {bad:?}");
        }
        let deep = "[".repeat(MAX_DEPTH + 2) + &"]".repeat(MAX_DEPTH + 2);
        assert!(parse(&deep).is_err());
        let v = parse(" {\"k\" : [ -0.5e1 , \"\\u00e9\\ud83d\\ude80\" ] } ");
        assert_eq!(v.ok().and_then(|v| v.get("k").cloned()), Some(Value::Arr(vec![num(-5.0), s("é🚀")])));
    }
}
