//! Draton standard library surface and Rust-backed FFI implementations.

pub mod ffi {
    /// Crypto bindings.
    pub mod crypto;
    /// Filesystem bindings.
    pub mod fs;
    /// Console bindings.
    pub mod io;
    /// Network bindings.
    pub mod net;
    /// Operating system bindings.
    pub mod os;
    /// Time bindings.
    pub mod time;
}

use std::collections::BTreeMap;

use thiserror::Error;

/// A bundled Draton stdlib source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdlibModule {
    /// The import name of the module.
    pub name: &'static str,
    /// The Draton source text bundled with the crate.
    pub source: &'static str,
}

/// Returns the bundled Draton stdlib modules.
pub fn modules() -> &'static [StdlibModule] {
    &[
        StdlibModule {
            name: "fs",
            source: include_str!("../dt/fs.dt"),
        },
        StdlibModule {
            name: "net",
            source: include_str!("../dt/net.dt"),
        },
        StdlibModule {
            name: "io",
            source: include_str!("../dt/io.dt"),
        },
        StdlibModule {
            name: "string",
            source: include_str!("../dt/string.dt"),
        },
        StdlibModule {
            name: "os",
            source: include_str!("../dt/os.dt"),
        },
        StdlibModule {
            name: "time",
            source: include_str!("../dt/time.dt"),
        },
        StdlibModule {
            name: "json",
            source: include_str!("../dt/json.dt"),
        },
        StdlibModule {
            name: "math",
            source: include_str!("../dt/math.dt"),
        },
        StdlibModule {
            name: "collections",
            source: include_str!("../dt/collections.dt"),
        },
        StdlibModule {
            name: "crypto",
            source: include_str!("../dt/crypto.dt"),
        },
        StdlibModule {
            name: "test",
            source: include_str!("../dt/test.dt"),
        },
    ]
}

/// Returns the bundled `.dt` test sources.
pub fn test_modules() -> &'static [StdlibModule] {
    &[
        StdlibModule {
            name: "fs_tests",
            source: include_str!("../tests/fs_tests.dt"),
        },
        StdlibModule {
            name: "string_tests",
            source: include_str!("../tests/string_tests.dt"),
        },
        StdlibModule {
            name: "json_tests",
            source: include_str!("../tests/json_tests.dt"),
        },
        StdlibModule {
            name: "math_tests",
            source: include_str!("../tests/math_tests.dt"),
        },
    ]
}

/// Filesystem helpers.
pub mod fs {
    pub use crate::ffi::fs::*;
}

/// Network helpers.
pub mod net {
    pub use crate::ffi::net::*;
    pub use crate::{JsonError, JsonValue, NetError, Response};
}

/// Console I/O helpers.
pub mod io {
    pub use crate::ffi::io::*;
}

/// Operating system helpers.
pub mod os {
    pub use crate::ffi::os::*;
}

/// Time helpers.
pub mod time {
    pub use crate::ffi::time::*;
    pub use crate::{DurationValue as Duration, Timestamp};
}

/// Crypto helpers.
pub mod crypto {
    pub use crate::ffi::crypto::*;
}

/// String parsing error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct ParseError {
    message: String,
}

impl ParseError {
    /// Creates a new parse error with the provided message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Filesystem error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct FsError {
    message: String,
}

impl FsError {
    /// Creates a new filesystem error with the provided message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Network error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message}")]
pub struct NetError {
    message: String,
}

impl NetError {
    /// Creates a new network error with the provided message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// JSON parse error with source coordinates.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{message} at line {line}, col {col}")]
pub struct JsonError {
    message: String,
    line: usize,
    col: usize,
}

impl JsonError {
    /// Creates a new JSON error with a message and source position.
    pub fn new(message: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            message: message.into(),
            line,
            col,
        }
    }

    /// Returns the message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the 1-based line.
    pub fn line(&self) -> usize {
        self.line
    }

    /// Returns the 1-based column.
    pub fn col(&self) -> usize {
        self.col
    }
}

/// A JSON value exposed to Draton programs.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// JSON `null`.
    Null,
    /// A boolean value.
    Bool(bool),
    /// A signed integer value.
    Int(i64),
    /// A floating-point value.
    Float(f64),
    /// A string value.
    Str(String),
    /// An array value.
    Array(Vec<JsonValue>),
    /// An object value.
    Object(BTreeMap<String, JsonValue>),
}

impl From<serde_json::Value> for JsonValue {
    fn from(value: serde_json::Value) -> Self {
        match value {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(value) => Self::Bool(value),
            serde_json::Value::Number(number) => {
                if let Some(value) = number.as_i64() {
                    Self::Int(value)
                } else if let Some(value) = number.as_f64() {
                    Self::Float(value)
                } else {
                    Self::Float(0.0)
                }
            }
            serde_json::Value::String(value) => Self::Str(value),
            serde_json::Value::Array(values) => {
                Self::Array(values.into_iter().map(JsonValue::from).collect())
            }
            serde_json::Value::Object(values) => Self::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, JsonValue::from(value)))
                    .collect(),
            ),
        }
    }
}

impl From<JsonValue> for serde_json::Value {
    fn from(value: JsonValue) -> Self {
        match value {
            JsonValue::Null => serde_json::Value::Null,
            JsonValue::Bool(value) => serde_json::Value::Bool(value),
            JsonValue::Int(value) => serde_json::Value::Number(value.into()),
            JsonValue::Float(value) => serde_json::json!(value),
            JsonValue::Str(value) => serde_json::Value::String(value),
            JsonValue::Array(values) => {
                serde_json::Value::Array(values.into_iter().map(serde_json::Value::from).collect())
            }
            JsonValue::Object(values) => serde_json::Value::Object(
                values
                    .into_iter()
                    .map(|(key, value)| (key, serde_json::Value::from(value)))
                    .collect(),
            ),
        }
    }
}

/// A blocking HTTP response.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    status: i64,
    body: String,
    headers: BTreeMap<String, String>,
}

impl Response {
    /// Creates a response from raw parts.
    pub fn new(status: i64, body: impl Into<String>, headers: BTreeMap<String, String>) -> Self {
        Self {
            status,
            body: body.into(),
            headers,
        }
    }

    /// Returns the HTTP status code.
    pub fn status(&self) -> i64 {
        self.status
    }

    /// Returns the response body as text.
    pub fn text(&self) -> String {
        self.body.clone()
    }

    /// Parses the response body as JSON.
    pub fn json(&self) -> Result<JsonValue, JsonError> {
        crate::json::parse(self.body.clone())
    }

    /// Returns a copy of the response headers.
    pub fn headers(&self) -> BTreeMap<String, String> {
        self.headers.clone()
    }
}

/// A point in time represented as Unix epoch milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp {
    unix_ms: i64,
}

impl Timestamp {
    /// Creates a timestamp from Unix epoch milliseconds.
    pub fn from_unix_ms(unix_ms: i64) -> Self {
        Self { unix_ms }
    }

    /// Returns the Unix epoch in milliseconds.
    pub fn unix(&self) -> i64 {
        self.unix_ms
    }

    /// Formats the timestamp with a `chrono` format string.
    pub fn format(&self, fmt: impl AsRef<str>) -> String {
        use chrono::{DateTime, Utc};

        let Some(datetime) = DateTime::<Utc>::from_timestamp_millis(self.unix_ms) else {
            return self.unix_ms.to_string();
        };
        datetime.format(fmt.as_ref()).to_string()
    }
}

/// A measured duration in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DurationValue {
    ms: i64,
}

impl DurationValue {
    /// Creates a duration from milliseconds.
    pub fn from_ms(ms: i64) -> Self {
        Self { ms }
    }

    /// Returns the duration in milliseconds.
    pub fn ms(&self) -> i64 {
        self.ms
    }

    /// Returns the duration in fractional seconds.
    pub fn seconds(&self) -> f64 {
        self.ms as f64 / 1_000.0
    }
}

/// Pure string helpers mirrored by the Draton stdlib.
pub mod string {
    use crate::ParseError;

    /// Uppercases a string.
    pub fn upper(s: impl AsRef<str>) -> String {
        s.as_ref().to_uppercase()
    }

    /// Lowercases a string.
    pub fn lower(s: impl AsRef<str>) -> String {
        s.as_ref().to_lowercase()
    }

    /// Trims leading and trailing whitespace.
    pub fn trim(s: impl AsRef<str>) -> String {
        s.as_ref().trim().to_string()
    }

    /// Trims leading whitespace.
    pub fn trim_start(s: impl AsRef<str>) -> String {
        s.as_ref().trim_start().to_string()
    }

    /// Trims trailing whitespace.
    pub fn trim_end(s: impl AsRef<str>) -> String {
        s.as_ref().trim_end().to_string()
    }

    /// Splits a string by a separator.
    pub fn split(s: impl AsRef<str>, sep: impl AsRef<str>) -> Vec<String> {
        s.as_ref()
            .split(sep.as_ref())
            .map(ToString::to_string)
            .collect()
    }

    /// Joins string parts with a separator.
    pub fn join(parts: &[String], sep: impl AsRef<str>) -> String {
        parts.join(sep.as_ref())
    }

    /// Checks whether the string contains a substring.
    pub fn contains(s: impl AsRef<str>, sub: impl AsRef<str>) -> bool {
        s.as_ref().contains(sub.as_ref())
    }

    /// Checks whether the string starts with a prefix.
    pub fn starts_with(s: impl AsRef<str>, prefix: impl AsRef<str>) -> bool {
        s.as_ref().starts_with(prefix.as_ref())
    }

    /// Checks whether the string ends with a suffix.
    pub fn ends_with(s: impl AsRef<str>, suffix: impl AsRef<str>) -> bool {
        s.as_ref().ends_with(suffix.as_ref())
    }

    /// Replaces the first occurrence of a substring.
    pub fn replace(s: impl AsRef<str>, from: impl AsRef<str>, to: impl AsRef<str>) -> String {
        s.as_ref().replacen(from.as_ref(), to.as_ref(), 1)
    }

    /// Replaces all occurrences of a substring.
    pub fn replace_all(s: impl AsRef<str>, from: impl AsRef<str>, to: impl AsRef<str>) -> String {
        s.as_ref().replace(from.as_ref(), to.as_ref())
    }

    /// Returns the Unicode scalar count of the string.
    pub fn len(s: impl AsRef<str>) -> i64 {
        s.as_ref().chars().count() as i64
    }

    /// Returns the string as an array of chars.
    pub fn chars(s: impl AsRef<str>) -> Vec<char> {
        s.as_ref().chars().collect()
    }

    /// Parses an integer.
    pub fn to_int(s: impl AsRef<str>) -> Result<i64, ParseError> {
        s.as_ref()
            .trim()
            .parse::<i64>()
            .map_err(|error| ParseError::new(error.to_string()))
    }

    /// Parses a float.
    pub fn to_float(s: impl AsRef<str>) -> Result<f64, ParseError> {
        s.as_ref()
            .trim()
            .parse::<f64>()
            .map_err(|error| ParseError::new(error.to_string()))
    }

    /// Repeats a string `n` times.
    pub fn repeat(s: impl AsRef<str>, n: i64) -> String {
        if n <= 0 {
            return String::new();
        }
        s.as_ref().repeat(n as usize)
    }

    /// Returns the character index of a substring.
    pub fn index_of(s: impl AsRef<str>, sub: impl AsRef<str>) -> Option<i64> {
        let haystack = s.as_ref();
        let needle = sub.as_ref();
        haystack
            .find(needle)
            .map(|byte_idx| haystack[..byte_idx].chars().count() as i64)
    }

    /// Returns a character-based slice of the string.
    pub fn slice(s: impl AsRef<str>, start: i64, end: i64) -> String {
        let chars = s.as_ref().chars().collect::<Vec<_>>();
        let len = chars.len() as i64;
        let start = start.clamp(0, len) as usize;
        let end = end.clamp(start as i64, len) as usize;
        chars[start..end].iter().collect()
    }
}

/// JSON helpers mirrored by the Draton stdlib.
pub mod json {
    use crate::{JsonError, JsonValue};

    /// Parses JSON text.
    pub fn parse(s: impl AsRef<str>) -> Result<JsonValue, JsonError> {
        serde_json::from_str::<serde_json::Value>(s.as_ref())
            .map(JsonValue::from)
            .map_err(|error| JsonError::new(error.to_string(), error.line(), error.column()))
    }

    /// Serializes a JSON value without extra whitespace.
    pub fn stringify(value: JsonValue) -> String {
        serde_json::to_string(&serde_json::Value::from(value))
            .unwrap_or_else(|_| "null".to_string())
    }

    /// Serializes a JSON value with indentation.
    pub fn pretty(value: JsonValue) -> String {
        serde_json::to_string_pretty(&serde_json::Value::from(value))
            .unwrap_or_else(|_| "null".to_string())
    }
}

/// Math helpers mirrored by the Draton stdlib.
pub mod math {
    /// Returns the square root of a number.
    pub fn sqrt(x: f64) -> f64 {
        libm::sqrt(x)
    }

    /// Raises a number to a power.
    pub fn pow(base: f64, exp: f64) -> f64 {
        libm::pow(base, exp)
    }

    /// Returns the absolute value.
    pub fn abs(x: f64) -> f64 {
        if x < 0.0 { -x } else { x }
    }

    /// Floors a number.
    pub fn floor(x: f64) -> f64 {
        libm::floor(x)
    }

    /// Ceils a number.
    pub fn ceil(x: f64) -> f64 {
        libm::ceil(x)
    }

    /// Rounds a number.
    pub fn round(x: f64) -> f64 {
        libm::round(x)
    }

    /// Returns the sine of a number.
    pub fn sin(x: f64) -> f64 {
        libm::sin(x)
    }

    /// Returns the cosine of a number.
    pub fn cos(x: f64) -> f64 {
        libm::cos(x)
    }

    /// Returns the tangent of a number.
    pub fn tan(x: f64) -> f64 {
        libm::tan(x)
    }

    /// Returns the natural logarithm.
    pub fn log(x: f64) -> f64 {
        libm::log(x)
    }

    /// Returns the base-2 logarithm.
    pub fn log2(x: f64) -> f64 {
        libm::log2(x)
    }

    /// Returns the base-10 logarithm.
    pub fn log10(x: f64) -> f64 {
        libm::log10(x)
    }

    /// Returns the smaller number.
    pub fn min(a: f64, b: f64) -> f64 {
        a.min(b)
    }

    /// Returns the larger number.
    pub fn max(a: f64, b: f64) -> f64 {
        a.max(b)
    }

    /// Clamps a number into a range.
    pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
        x.clamp(lo, hi)
    }

    /// Returns `pi`.
    pub fn pi() -> f64 {
        std::f64::consts::PI
    }

    /// Returns Euler's number.
    pub fn e() -> f64 {
        std::f64::consts::E
    }

    /// Adds integers with overflow checking.
    pub fn checked_add(a: i64, b: i64) -> Option<i64> {
        a.checked_add(b)
    }

    /// Subtracts integers with overflow checking.
    pub fn checked_sub(a: i64, b: i64) -> Option<i64> {
        a.checked_sub(b)
    }

    /// Multiplies integers with overflow checking.
    pub fn checked_mul(a: i64, b: i64) -> Option<i64> {
        a.checked_mul(b)
    }

    /// Divides integers with overflow checking.
    pub fn checked_div(a: i64, b: i64) -> Option<i64> {
        a.checked_div(b)
    }
}
