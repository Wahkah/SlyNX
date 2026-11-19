//! A minimal JSON reader and writer, enough for performance reports and baselines.
//!
//! Objects keep their key order. Numbers are `f64`; non-finite numbers are written as `null`.

use std::fmt::{self, Write as _};

/// A JSON value.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// A parse error with the byte offset where it happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsonError {
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON error at byte {}: {}", self.offset, self.message)
    }
}

impl std::error::Error for JsonError {}

impl Value {
    /// Looks up a key in an object.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The number, if this is one.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The string, if this is one.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The elements, if this is an array.
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The fields, if this is an object.
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(o) => Some(o),
            _ => None,
        }
    }

    /// Parses a complete JSON document.
    pub fn parse(text: &str) -> Result<Value, JsonError> {
        let mut p = Parser { s: text.as_bytes(), i: 0, depth: 0 };
        p.ws();
        let v = p.value()?;
        p.ws();
        if p.i != p.s.len() {
            return Err(p.err("trailing characters"));
        }
        Ok(v)
    }

    /// Writes the value with two-space indentation.
    pub fn to_pretty(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, true);
        out.push('\n');
        out
    }

    /// Writes the value on one line.
    pub fn to_compact(&self) -> String {
        let mut out = String::new();
        self.write(&mut out, 0, false);
        out
    }

    fn write(&self, out: &mut String, indent: usize, pretty: bool) {
        let nl = |out: &mut String, level: usize| {
            if pretty {
                out.push('\n');
                for _ in 0..level {
                    out.push_str("  ");
                }
            }
        };
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => write_number(out, *n),
            Value::String(s) => write_string(out, s),
            Value::Array(items) => {
                // Arrays of plain numbers stay on one line: per-frame series would otherwise be thousands of lines.
                let flat = items.iter().all(|v| matches!(v, Value::Number(_) | Value::Null | Value::Bool(_)));
                out.push('[');
                for (n, item) in items.iter().enumerate() {
                    if n > 0 {
                        out.push(',');
                        if flat && pretty {
                            out.push(' ');
                        }
                    }
                    if !flat {
                        nl(out, indent + 1);
                    }
                    item.write(out, indent + 1, pretty);
                }
                if !flat && !items.is_empty() {
                    nl(out, indent);
                }
                out.push(']');
            }
            Value::Object(fields) => {
                out.push('{');
                for (n, (k, v)) in fields.iter().enumerate() {
                    if n > 0 {
                        out.push(',');
                    }
                    nl(out, indent + 1);
                    write_string(out, k);
                    out.push(':');
                    if pretty {
                        out.push(' ');
                    }
                    v.write(out, indent + 1, pretty);
                }
                if !fields.is_empty() {
                    nl(out, indent);
                }
                out.push('}');
            }
        }
    }
}

fn write_number(out: &mut String, n: f64) {
    if !n.is_finite() {
        out.push_str("null");
    } else if n == n.trunc() && n.abs() < 1e15 {
        let _ = write!(out, "{}", n as i64);
    } else {
        let _ = write!(out, "{n}");
    }
}

fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
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

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
    depth: usize,
}

impl Parser<'_> {
    fn err(&self, message: &str) -> JsonError {
        JsonError { offset: self.i, message: message.to_string() }
    }

    fn ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.s.get(self.i) {
            self.i += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Value, JsonError> {
        match self.s.get(self.i) {
            None => Err(self.err("unexpected end")),
            Some(b'{') => self.nested(Self::object),
            Some(b'[') => self.nested(Self::array),
            Some(b'"') => Ok(Value::String(self.string()?)),
            Some(b't') if self.eat("true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Value::Bool(false)),
            Some(b'n') if self.eat("null") => Ok(Value::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn nested(&mut self, f: fn(&mut Self) -> Result<Value, JsonError>) -> Result<Value, JsonError> {
        self.depth += 1;
        if self.depth > 128 {
            return Err(self.err("nesting too deep"));
        }
        let v = f(self);
        self.depth -= 1;
        v
    }

    fn object(&mut self) -> Result<Value, JsonError> {
        self.i += 1;
        let mut fields = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Value::Object(fields));
        }
        loop {
            self.ws();
            if self.s.get(self.i) != Some(&b'"') {
                return Err(self.err("expected a key"));
            }
            let k = self.string()?;
            self.ws();
            if !self.eat(":") {
                return Err(self.err("expected `:`"));
            }
            self.ws();
            let v = self.value()?;
            fields.push((k, v));
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("}") {
                return Ok(Value::Object(fields));
            }
            return Err(self.err("expected `,` or `}`"));
        }
    }

    fn array(&mut self) -> Result<Value, JsonError> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Value::Array(items));
        }
        loop {
            self.ws();
            items.push(self.value()?);
            self.ws();
            if self.eat(",") {
                continue;
            }
            if self.eat("]") {
                return Ok(Value::Array(items));
            }
            return Err(self.err("expected `,` or `]`"));
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let h = self.s.get(self.i..self.i + 4).ok_or_else(|| self.err("short \\u escape"))?;
        let h = std::str::from_utf8(h).map_err(|_| self.err("bad \\u escape"))?;
        let v = u32::from_str_radix(h, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(v)
    }

    fn string(&mut self) -> Result<String, JsonError> {
        self.i += 1;
        let mut out = Vec::new();
        loop {
            let Some(&b) = self.s.get(self.i) else {
                return Err(self.err("unterminated string"));
            };
            self.i += 1;
            match b {
                b'"' => break,
                b'\\' => {
                    let Some(&e) = self.s.get(self.i) else {
                        return Err(self.err("unterminated escape"));
                    };
                    self.i += 1;
                    let c = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            let code = if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return Err(self.err("lone surrogate"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("bad surrogate pair"));
                                }
                                0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                            } else {
                                hi
                            };
                            char::from_u32(code).ok_or_else(|| self.err("bad code point"))?
                        }
                        _ => return Err(self.err("bad escape")),
                    };
                    let mut buf = [0; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                b if b < 0x20 => return Err(self.err("control character in string")),
                b => out.push(b),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("invalid UTF-8"))
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.s.get(self.i) {
            self.i += 1;
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).unwrap_or("");
        text.parse::<f64>().map(Value::Number).map_err(|_| JsonError { offset: start, message: "bad number".into() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let v = Value::Object(vec![
            ("name".into(), Value::String("a \"q\" \\ \n é".into())),
            ("n".into(), Value::Number(1.5)),
            ("i".into(), Value::Number(42.0)),
            ("neg".into(), Value::Number(-0.001)),
            ("list".into(), Value::Array(vec![Value::Number(1.0), Value::Null, Value::Bool(true)])),
            ("nested".into(), Value::Array(vec![Value::Object(vec![]), Value::Array(vec![])])),
        ]);
        assert_eq!(Value::parse(&v.to_pretty()).unwrap(), v);
        assert_eq!(Value::parse(&v.to_compact()).unwrap(), v);
    }

    #[test]
    fn parses_escapes_and_whitespace() {
        let v = Value::parse(" { \"a\" : [ 1 , 2e3 , -4.5E-1 ] , \"b\":\"\\u00e9\\ud83d\\ude00\\/\" } ").unwrap();
        assert_eq!(v.get("a").unwrap().as_array().unwrap()[1].as_f64(), Some(2000.0));
        assert_eq!(v.get("b").unwrap().as_str(), Some("é\u{1F600}/"));
    }

    #[test]
    fn rejects_bad_input() {
        for bad in ["", "{", "[1,]", "{\"a\" 1}", "tru", "\"abc", "1 2", "{\"a\":1,}", "\"\\x\"", "-"] {
            assert!(Value::parse(bad).is_err(), "{bad:?}");
        }
        assert!(Value::parse(&"[".repeat(200)).is_err());
    }

    #[test]
    fn non_finite_is_null() {
        assert_eq!(Value::Number(f64::INFINITY).to_compact(), "null");
    }
}
