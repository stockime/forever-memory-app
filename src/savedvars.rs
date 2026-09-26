//! Parses WoW SavedVariables files: `name = <lua value>` assignments where
//! values are nil, booleans, numbers, strings or tables. Tables whose keys are
//! exactly 1..n become arrays, all others objects with string keys. Empty
//! tables become null, since Lua can't tell an empty list from an empty map.

use serde_json::{Map, Number, Value};

/// Every top-level assignment in the file.
pub fn parse(src: &[u8]) -> Result<Map<String, Value>, String> {
    let mut p = Parser { s: src, pos: 0 };
    let mut out = Map::new();
    loop {
        p.space();
        if p.eof() {
            return Ok(out);
        }
        let name = p.ident();
        if name.is_empty() {
            return Err(p.error("expected a variable name"));
        }
        p.space();
        if !p.eat(b'=') {
            return Err(p.error(&format!("expected '=' after {name}")));
        }
        let v = p.value()?;
        out.insert(name, v);
    }
}

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn eof(&self) -> bool {
        self.pos >= self.s.len()
    }

    fn peek(&self) -> u8 {
        self.s[self.pos]
    }

    fn error(&self, msg: &str) -> String {
        let line = self.s[..self.pos.min(self.s.len())]
            .iter()
            .filter(|&&c| c == b'\n')
            .count()
            + 1;
        format!("savedvars line {line}: {msg}")
    }

    /// Skips whitespace and -- comments.
    fn space(&mut self) {
        while !self.eof() {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                b'-' if self.s[self.pos..].starts_with(b"--") => {
                    match self.s[self.pos..].iter().position(|&c| c == b'\n') {
                        Some(i) => self.pos += i + 1,
                        None => self.pos = self.s.len(),
                    }
                }
                _ => return,
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        if !self.eof() && self.peek() == c {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> String {
        let start = self.pos;
        while !self.eof() {
            let c = self.peek();
            if c == b'_' || c.is_ascii_alphabetic() || (self.pos > start && c.is_ascii_digit()) {
                self.pos += 1;
            } else {
                break;
            }
        }
        String::from_utf8_lossy(&self.s[start..self.pos]).into_owned()
    }

    fn value(&mut self) -> Result<Value, String> {
        self.space();
        if self.eof() {
            return Err(self.error("unexpected end of file"));
        }
        match self.peek() {
            b'{' => return self.table(),
            b'"' | b'\'' => return self.string().map(Value::String),
            b'-' | b'.' | b'0'..=b'9' => return self.number(),
            _ => {}
        }
        match self.ident().as_str() {
            "nil" => Ok(Value::Null),
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            id => Err(self.error(&format!("unexpected {id:?}"))),
        }
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        while !self.eof() && b"+-.0123456789eExXabcdefABCDEF".contains(&self.peek()) {
            self.pos += 1;
        }
        let lit = std::str::from_utf8(&self.s[start..self.pos]).unwrap_or("");
        if let Some(n) = parse_int(lit) {
            return Ok(Value::Number(n.into()));
        }
        lit.parse::<f64>()
            .ok()
            .and_then(Number::from_f64)
            .map(Value::Number)
            .ok_or_else(|| self.error(&format!("bad number {lit:?}")))
    }

    fn string(&mut self) -> Result<String, String> {
        let quote = self.peek();
        self.pos += 1;
        let mut b: Vec<u8> = vec![];
        while !self.eof() {
            let c = self.peek();
            self.pos += 1;
            if c == quote {
                return Ok(String::from_utf8_lossy(&b).into_owned());
            }
            if c != b'\\' {
                b.push(c);
                continue;
            }
            if self.eof() {
                break;
            }
            let e = self.peek();
            self.pos += 1;
            match e {
                b'n' => b.push(b'\n'),
                b't' => b.push(b'\t'),
                b'r' => b.push(b'\r'),
                b'\n' => b.push(b'\n'),
                b'0'..=b'9' => {
                    let mut n = (e - b'0') as u32;
                    for _ in 0..2 {
                        if self.eof() || !self.peek().is_ascii_digit() {
                            break;
                        }
                        n = n * 10 + (self.peek() - b'0') as u32;
                        self.pos += 1;
                    }
                    b.push(n as u8);
                }
                other => b.push(other), // \\ \" \'
            }
        }
        Err(self.error("unterminated string"))
    }

    fn table(&mut self) -> Result<Value, String> {
        self.pos += 1; // {
        let mut m = Map::new();
        let mut next: i64 = 1;
        loop {
            self.space();
            if self.eat(b'}') {
                break;
            }
            
            let key = if self.eat(b'[') {
                let k = self.value()?;
                self.space();
                if !self.eat(b']') {
                    return Err(self.error("expected ']'"));
                }
                self.space();
                if !self.eat(b'=') {
                    return Err(self.error("expected '='"));
                }
                match k {
                    Value::String(s) => s,
                    other => other.to_string(),
                }
            } else {
                let save = self.pos;
                let id = self.ident();
                let mut named = None;
                if !id.is_empty() && !matches!(id.as_str(), "true" | "false" | "nil") {
                    self.space();
                    if self.eat(b'=') {
                        named = Some(id);
                    }
                }
                match named {
                    Some(k) => k,
                    None => {
                        self.pos = save; // a positional value
                        next.to_string()
                    }
                }
            };
            let v = self.value()?;
            let positional = key == next.to_string();
            if !v.is_null() {
                m.insert(key, v);
            }
            if positional {
                next += 1;
            }
            self.space();
            if !self.eat(b',') && !self.eat(b';') {
                self.space();
                if !self.eat(b'}') {
                    return Err(self.error("expected ',' or '}'"));
                }
                break;
            }
        }
        if m.is_empty() {
            return Ok(Value::Null);
        }
        // A table whose keys are exactly 1..n is a list.
        if m.len() as i64 == next - 1 {
            return Ok(Value::Array(
                (1..next)
                    .map(|i| m.remove(&i.to_string()).unwrap_or(Value::Null))
                    .collect(),
            ));
        }
        Ok(Value::Object(m))
    }
}

/// Integers the way Go's strconv.ParseInt(s, 0, 64) reads them.
fn parse_int(s: &str) -> Option<i64> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if body.chars().all(|c| c.is_ascii_digit()) && !body.is_empty() {
        body.parse().ok()?
    } else {
        return None;
    };
    Some(if neg { -v } else { v })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn values() {
        let src = br#"
armory_db = {
    ["characters"] = {
        ["Player-1"] = { ["name"] = "Dead", ["level"] = 4, ["xp"] = 0.5, ["ok"] = true },
    },
    ["list"] = { "a", "b\"c", 3, },
    ["empty"] = {},
    ["mixed"] = { [1] = 1, [3] = 3 },
}
-- a comment
armory_memory = nil
"#;
        let v = super::parse(src).unwrap();
        assert_eq!(
            v["armory_db"]["characters"]["Player-1"],
            json!({"name": "Dead", "level": 4, "xp": 0.5, "ok": true})
        );
        assert_eq!(v["armory_db"]["list"], json!(["a", "b\"c", 3]));
        assert!(v["armory_db"].get("empty").is_none());
        assert_eq!(v["armory_db"]["mixed"], json!({"1": 1, "3": 3}));
        assert_eq!(v["armory_memory"], json!(null));
    }

    #[test]
    fn escapes() {
        let v = super::parse(b"x = \"a\\nb\\065\\\\\"").unwrap();
        assert_eq!(v["x"], json!("a\nbA\\"));
    }
}
