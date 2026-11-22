// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Swanand Tanavade

//! Error type. Every fallible public function returns [`Error`]; the library
//! never panics on user input.

use std::fmt;

/// All recoverable failures surfaced by the library and CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// A parameter name that is not in [`crate::params::SPECS`].
    UnknownKey(String),
    /// A value that could not be parsed or is not finite.
    InvalidValue {
        /// Parameter name.
        key: String,
        /// Raw text supplied by the user.
        value: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// A value outside the supported domain of the model.
    OutOfRange {
        /// Parameter name.
        key: String,
        /// Supplied value.
        value: f64,
        /// Inclusive lower bound.
        min: f64,
        /// Inclusive upper bound.
        max: f64,
    },
    /// Parameters that are individually valid but inconsistent together.
    Inconsistent(String),
    /// A malformed scenario file.
    Parse {
        /// 1-based line number.
        line: usize,
        /// Human-readable description.
        message: String,
    },
    /// File-system failure (message only; paths are not echoed beyond the one supplied).
    Io(String),
    /// Bad command-line usage.
    Usage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Every user-controlled string is sanitized before display so hostile
        // input cannot inject terminal escape sequences into logs or terminals.
        use crate::scenario::sanitize as s;
        match self {
            Error::UnknownKey(k) => write!(f, "unknown parameter '{}' (run `firmgap params` for the list)", s(k)),
            Error::InvalidValue { key, value, reason } => {
                write!(f, "invalid value '{}' for '{}': {}", s(value), s(key), reason)
            }
            Error::OutOfRange { key, value, min, max } => {
                write!(f, "'{}' = {} is outside the supported range [{}, {}]", s(key), value, min, max)
            }
            Error::Inconsistent(m) => write!(f, "inconsistent parameters: {}", s(m)),
            Error::Parse { line, message } => write!(f, "scenario line {}: {}", line, s(message)),
            Error::Io(m) => write!(f, "i/o error: {}", s(m)),
            Error::Usage(m) => write!(f, "{}", m),
        }
    }
}

impl std::error::Error for Error {}
