//! json: minimal RFC 8259 JSON parsing and serialization on the Rust
//! standard library alone. Zero dependencies.
//!
//! Rust's std has no JSON support; the usual answer is serde's crate tree.
//! This crate is the other end of the trade: a small [`Value`] enum, a strict
//! spec-anchored parser with line/column errors, and compact + pretty
//! serialization. No derive, no typed mapping, no streaming. Parse, inspect,
//! serialize, done.
//!
//! ```
//! let v = json::parse(r#"{"name": "ada", "tags": [1, 2]}"#).unwrap();
//! assert_eq!(v.get("name").and_then(|v| v.as_str()), Some("ada"));
//! assert_eq!(v.to_string(), r#"{"name":"ada","tags":[1,2]}"#);
//! ```
//!
//! Design decisions, stated plainly:
//!
//! * **Objects preserve order** (a vector of pairs, not a hash map), and
//!   duplicate keys are preserved on parse/serialize; [`Value::get`] returns
//!   the *last* occurrence, matching `JSON.parse` semantics.
//! * **Numbers** are [`Number::Int`] (`i64`) when the lexeme is integral and
//!   fits, otherwise [`Number::Float`] (`f64`). Integers beyond the `i64`
//!   range parse as floats. Numbers whose value overflows `f64` (`1e999`) are
//!   a parse error, not infinity.
//! * **Strict by default:** trailing commas, comments, single quotes,
//!   unquoted keys, `NaN`/`Infinity`, leading zeros, control characters in
//!   strings, and unpaired surrogates are all rejected. One leading U+FEFF
//!   BOM is skipped. Nesting is limited to [`MAX_DEPTH`].
//! * **Float serialization** uses Rust's shortest round-trip formatting, with
//!   `.0` appended to integral floats so a float stays a float across a
//!   round trip. Non-finite floats (only constructible via the API, never by
//!   parsing) serialize as `null`, like JavaScript's `JSON.stringify`.

use std::fmt;

/// Maximum nesting depth of arrays/objects the parser accepts.
pub const MAX_DEPTH: usize = 128;

/// A parsed JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Value>),
    /// Key/value pairs in document order; duplicates are preserved.
    Object(Vec<(String, Value)>),
}

/// A JSON number: an `i64` when the lexeme is integral and fits, else `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

/// A parse failure, locating the offending byte in the input.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    /// 1-based byte column within the line.
    pub col: usize,
    /// What went wrong.
    pub msg: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}: {}", self.line, self.col, self.msg)
    }
}

impl std::error::Error for ParseError {}

impl Value {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The value as `i64`, only for [`Number::Int`]; floats return `None`.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Number(Number::Int(i)) => Some(*i),
            _ => None,
        }
    }

    /// The value as `f64`, for both int and float numbers.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(Number::Int(i)) => Some(*i as f64),
            Value::Number(Number::Float(f)) => Some(*f),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Object member lookup. With duplicate keys the *last* occurrence wins,
    /// matching `JSON.parse`. Returns `None` on non-objects.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(o) => o.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Serialize with newlines and `indent` spaces per nesting level.
    pub fn pretty(&self, indent: usize) -> String {
        let mut out = String::new();
        write_pretty(self, indent, 0, &mut out);
        out
    }
}

/// Compact serialization, no whitespace.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        write_compact(self, &mut out);
        f.write_str(&out)
    }
}

fn write_number(n: &Number, out: &mut String) {
    match n {
        Number::Int(i) => out.push_str(&i.to_string()),
        Number::Float(f) if f.is_finite() => {
            let s = f.to_string();
            let integral = !s.contains('.');
            out.push_str(&s);
            if integral {
                out.push_str(".0");
            }
        }
        // Not constructible by parsing; serialize like JSON.stringify does.
        Number::Float(_) => out.push_str("null"),
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_compact(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(n, out),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_compact(item, out);
            }
            out.push(']');
        }
        Value::Object(members) => {
            out.push('{');
            for (i, (k, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_compact(item, out);
            }
            out.push('}');
        }
    }
}

fn write_pretty(v: &Value, indent: usize, level: usize, out: &mut String) {
    match v {
        Value::Array(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                push_indent(out, indent, level + 1);
                write_pretty(item, indent, level + 1, out);
            }
            out.push('\n');
            push_indent(out, indent, level);
            out.push(']');
        }
        Value::Object(members) if !members.is_empty() => {
            out.push_str("{\n");
            for (i, (k, item)) in members.iter().enumerate() {
                if i > 0 {
                    out.push_str(",\n");
                }
                push_indent(out, indent, level + 1);
                write_string(k, out);
                out.push_str(": ");
                write_pretty(item, indent, level + 1, out);
            }
            out.push('\n');
            push_indent(out, indent, level);
            out.push('}');
        }
        other => write_compact(other, out),
    }
}

fn push_indent(out: &mut String, indent: usize, level: usize) {
    for _ in 0..indent * level {
        out.push(' ');
    }
}

/// Parse a complete JSON text into a [`Value`].
///
/// The whole input must be one JSON value plus optional whitespace (and one
/// optional leading BOM); anything after it is an error.
pub fn parse(text: &str) -> Result<Value, ParseError> {
    let mut p = Parser {
        text: text.strip_prefix('\u{feff}').unwrap_or(text),
        pos: 0,
        depth: 0,
    };
    p.skip_ws();
    let v = p.parse_value()?;
    p.skip_ws();
    if p.pos < p.text.len() {
        return Err(p.err("unexpected trailing characters"));
    }
    Ok(v)
}

struct Parser<'a> {
    text: &'a str,
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    fn bytes(&self) -> &'a [u8] {
        self.text.as_bytes()
    }

    fn peek(&self) -> Option<u8> {
        self.bytes().get(self.pos).copied()
    }

    fn err(&self, msg: &'static str) -> ParseError {
        self.err_at(self.pos, msg)
    }

    fn err_at(&self, pos: usize, msg: &'static str) -> ParseError {
        let upto = &self.bytes()[..pos.min(self.text.len())];
        let line = 1 + upto.iter().filter(|&&b| b == b'\n').count();
        let line_start = upto.iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
        ParseError {
            line,
            col: pos - line_start + 1,
            msg,
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<Value, ParseError> {
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(Value::String),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            _ => Err(self.err("expected a value")),
        }
    }

    fn literal(&mut self, lit: &'static str, v: Value) -> Result<Value, ParseError> {
        if self.bytes()[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(v)
        } else {
            Err(self.err("invalid literal"))
        }
    }

    fn parse_number(&mut self) -> Result<Value, ParseError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => self.pos += 1,
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.err_at(start, "invalid number")),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err_at(start, "invalid number"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.err_at(start, "invalid number"));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let lexeme = &self.text[start..self.pos];
        if !is_float {
            if let Ok(i) = lexeme.parse::<i64>() {
                return Ok(Value::Number(Number::Int(i)));
            }
        }
        let f: f64 = lexeme
            .parse()
            .map_err(|_| self.err_at(start, "invalid number"))?;
        if !f.is_finite() {
            return Err(self.err_at(start, "number out of range"));
        }
        Ok(Value::Number(Number::Float(f)))
    }

    fn parse_string(&mut self) -> Result<String, ParseError> {
        self.pos += 1; // opening quote, verified by the caller
        let mut start = self.pos;
        let mut out: Option<String> = None;
        loop {
            match self.peek() {
                None => return Err(self.err("unterminated string")),
                Some(b'"') => {
                    let tail = &self.text[start..self.pos];
                    self.pos += 1;
                    return Ok(match out {
                        None => tail.to_string(),
                        Some(mut s) => {
                            s.push_str(tail);
                            s
                        }
                    });
                }
                Some(b'\\') => {
                    let mut s = out.take().unwrap_or_default();
                    s.push_str(&self.text[start..self.pos]);
                    self.pos += 1;
                    self.parse_escape(&mut s)?;
                    start = self.pos;
                    out = Some(s);
                }
                Some(b) if b < 0x20 => {
                    return Err(self.err("control character in string"));
                }
                Some(_) => self.pos += 1,
            }
        }
    }

    fn parse_escape(&mut self, s: &mut String) -> Result<(), ParseError> {
        let c = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.pos += 1;
                let hi = self.hex4()?;
                let cp = if (0xD800..=0xDBFF).contains(&hi) {
                    if self.peek() == Some(b'\\') && self.bytes().get(self.pos + 1) == Some(&b'u') {
                        self.pos += 2;
                        let lo = self.hex4()?;
                        if !(0xDC00..=0xDFFF).contains(&lo) {
                            return Err(self.err("unpaired surrogate"));
                        }
                        0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                    } else {
                        return Err(self.err("unpaired surrogate"));
                    }
                } else if (0xDC00..=0xDFFF).contains(&hi) {
                    return Err(self.err("unpaired surrogate"));
                } else {
                    hi
                };
                s.push(char::from_u32(cp).expect("surrogates excluded above"));
                return Ok(());
            }
            None => return Err(self.err("unterminated string")),
            Some(_) => return Err(self.err("invalid escape")),
        };
        s.push(c);
        self.pos += 1;
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let d = match self.peek() {
                Some(b @ b'0'..=b'9') => (b - b'0') as u32,
                Some(b @ b'a'..=b'f') => (b - b'a' + 10) as u32,
                Some(b @ b'A'..=b'F') => (b - b'A' + 10) as u32,
                _ => return Err(self.err("invalid \\u escape")),
            };
            v = v * 16 + d;
            self.pos += 1;
        }
        Ok(v)
    }

    fn enter(&mut self) -> Result<(), ParseError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        Ok(())
    }

    fn parse_array(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws();
                }
                Some(b']') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.err("expected ',' or ']'")),
            }
        }
    }

    fn parse_object(&mut self) -> Result<Value, ParseError> {
        self.enter()?;
        self.pos += 1; // '{'
        let mut members = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            self.depth -= 1;
            return Ok(Value::Object(members));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a string key"));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected ':'"));
            }
            self.pos += 1;
            self.skip_ws();
            members.push((key, self.parse_value()?));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                    self.skip_ws();
                }
                Some(b'}') => {
                    self.pos += 1;
                    self.depth -= 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.err("expected ',' or '}'")),
            }
        }
    }
}
