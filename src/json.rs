// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Minimal, dependency-free JSON writer (output only; firmgap never parses JSON).
//!
//! Non-finite numbers are written as `null`; strings are fully escaped, so
//! user-supplied text can never break out of a string literal.

use std::fmt::Write as _;

/// Streaming JSON builder with automatic comma handling.
#[derive(Debug, Default)]
pub struct Json {
    buf: String,
    first: Vec<bool>,
    after_key: bool,
}

impl Json {
    /// Empty writer.
    pub fn new() -> Self {
        Self::default()
    }

    fn pre_value(&mut self) {
        if self.after_key {
            self.after_key = false;
            return;
        }
        if let Some(first) = self.first.last_mut() {
            if *first {
                *first = false;
            } else {
                self.buf.push(',');
            }
        }
    }

    fn push_str_lit(&mut self, s: &str) {
        self.buf.push('"');
        for ch in s.chars() {
            match ch {
                '"' => self.buf.push_str("\\\""),
                '\\' => self.buf.push_str("\\\\"),
                '\n' => self.buf.push_str("\\n"),
                '\r' => self.buf.push_str("\\r"),
                '\t' => self.buf.push_str("\\t"),
                c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                    let _ = write!(self.buf, "\\u{:04x}", c as u32);
                }
                c => self.buf.push(c),
            }
        }
        self.buf.push('"');
    }

    /// Open an object.
    pub fn begin_obj(&mut self) -> &mut Self {
        self.pre_value();
        self.buf.push('{');
        self.first.push(true);
        self
    }

    /// Close the current object.
    pub fn end_obj(&mut self) -> &mut Self {
        self.first.pop();
        self.buf.push('}');
        self
    }

    /// Open an array.
    pub fn begin_arr(&mut self) -> &mut Self {
        self.pre_value();
        self.buf.push('[');
        self.first.push(true);
        self
    }

    /// Close the current array.
    pub fn end_arr(&mut self) -> &mut Self {
        self.first.pop();
        self.buf.push(']');
        self
    }

    /// Write an object key; the next call writes its value.
    pub fn key(&mut self, k: &str) -> &mut Self {
        self.pre_value();
        self.push_str_lit(k);
        self.buf.push(':');
        self.after_key = true;
        self
    }

    /// Write a number (`null` if not finite).
    pub fn num(&mut self, v: f64) -> &mut Self {
        self.pre_value();
        if v.is_finite() {
            let _ = write!(self.buf, "{v}");
        } else {
            self.buf.push_str("null");
        }
        self
    }

    /// Write an unsigned integer.
    pub fn uint(&mut self, v: u64) -> &mut Self {
        self.pre_value();
        let _ = write!(self.buf, "{v}");
        self
    }

    /// Write a boolean.
    pub fn boolean(&mut self, v: bool) -> &mut Self {
        self.pre_value();
        self.buf.push_str(if v { "true" } else { "false" });
        self
    }

    /// Write a string.
    pub fn string(&mut self, v: &str) -> &mut Self {
        self.pre_value();
        self.push_str_lit(v);
        self
    }

    /// `key: number` shorthand.
    pub fn kv_num(&mut self, k: &str, v: f64) -> &mut Self {
        self.key(k).num(v)
    }

    /// Finish and return the document.
    pub fn finish(self) -> String {
        self.buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_document() {
        let mut j = Json::new();
        j.begin_obj();
        j.kv_num("a", 1.5);
        j.key("b").begin_arr().num(f64::NAN).boolean(true).string("x\"y\n").end_arr();
        j.key("c").begin_obj().key("d").uint(7).end_obj();
        j.end_obj();
        assert_eq!(j.finish(), r#"{"a":1.5,"b":[null,true,"x\"y\n"],"c":{"d":7}}"#);
    }
}
