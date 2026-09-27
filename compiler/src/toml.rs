// Mały podzbiór TOML, który wystarcza na sowa.toml: sekcje [a] i [a.b], klucze z tekstem,
// liczbą, true/false, listą w jednej linii [a, b] albo tabelą w jednej linii { k = v, ... },
// komentarze `#`.

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Value {
    Str(String),
    Int(i64),
    Bool(bool),
    List(Vec<Value>),
    Table(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Table(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key) {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub key: String,
    pub value: Value,
    pub line: usize,
}

// Sekcja → wpisy w kolejności z pliku.
pub type Doc = BTreeMap<String, Vec<Entry>>;

pub fn parse(src: &str) -> Result<Doc, (usize, String)> {
    let mut doc: Doc = BTreeMap::new();
    let mut section = String::new();
    doc.insert(section.clone(), vec![]);
    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        let l = strip_comment(raw).trim().to_string();
        if l.is_empty() {
            continue;
        }
        if l.starts_with('[') {
            if !l.ends_with(']') {
                return Err((line, "niezamknięta nazwa sekcji".into()));
            }
            section = l[1..l.len() - 1].trim().to_string();
            doc.entry(section.clone()).or_default();
            continue;
        }
        let Some(eq) = l.find('=') else {
            return Err((line, "oczekiwano `klucz = wartość`".into()));
        };
        let key = l[..eq].trim().to_string();
        let mut p = P {
            c: l[eq + 1..].chars().collect(),
            i: 0,
        };
        let value = p.value().map_err(|m| (line, m))?;
        p.ws();
        if p.i < p.c.len() {
            return Err((line, "nadmiarowy tekst po wartości".into()));
        }
        doc.get_mut(&section).unwrap().push(Entry { key, value, line });
    }
    Ok(doc)
}

fn strip_comment(l: &str) -> String {
    let mut out = String::new();
    let mut in_str = false;
    for c in l.chars() {
        if c == '"' {
            in_str = !in_str;
        }
        if c == '#' && !in_str {
            break;
        }
        out.push(c);
    }
    out
}

struct P {
    c: Vec<char>,
    i: usize,
}

impl P {
    fn ws(&mut self) {
        while self.i < self.c.len() && self.c[self.i] == ' ' {
            self.i += 1;
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.c.get(self.i) {
            Some('"') => {
                self.i += 1;
                let mut s = String::new();
                while let Some(&c) = self.c.get(self.i) {
                    self.i += 1;
                    if c == '"' {
                        return Ok(Value::Str(s));
                    }
                    if c == '\\' {
                        if let Some(&d) = self.c.get(self.i) {
                            self.i += 1;
                            s.push(if d == 'n' { '\n' } else { d });
                        }
                        continue;
                    }
                    s.push(c);
                }
                Err("niezamknięty tekst".into())
            }
            Some('[') => {
                self.i += 1;
                let mut items = vec![];
                loop {
                    self.ws();
                    if self.c.get(self.i) == Some(&']') {
                        self.i += 1;
                        return Ok(Value::List(items));
                    }
                    items.push(self.value()?);
                    self.ws();
                    match self.c.get(self.i) {
                        Some(',') => self.i += 1,
                        Some(']') => {}
                        _ => return Err("oczekiwano `,` albo `]` w liście".into()),
                    }
                }
            }
            Some('{') => {
                self.i += 1;
                let mut items = vec![];
                loop {
                    self.ws();
                    if self.c.get(self.i) == Some(&'}') {
                        self.i += 1;
                        return Ok(Value::Table(items));
                    }
                    let mut k = String::new();
                    while let Some(&c) = self.c.get(self.i) {
                        if c == '=' || c == ' ' {
                            break;
                        }
                        k.push(c);
                        self.i += 1;
                    }
                    self.ws();
                    if self.c.get(self.i) != Some(&'=') {
                        return Err("oczekiwano `=` w tabeli".into());
                    }
                    self.i += 1;
                    let v = self.value()?;
                    items.push((k, v));
                    self.ws();
                    if self.c.get(self.i) == Some(&',') {
                        self.i += 1;
                    }
                }
            }
            Some(_) => {
                let mut s = String::new();
                while let Some(&c) = self.c.get(self.i) {
                    if c == ',' || c == '}' || c == ']' || c == ' ' {
                        break;
                    }
                    s.push(c);
                    self.i += 1;
                }
                match s.as_str() {
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    _ => s.parse().map(Value::Int).map_err(|_| format!("nieznana wartość `{}`", s)),
                }
            }
            None => Err("brak wartości".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists() {
        let d = parse("[review]\napprovers = [\"@anna\", \"piotr@firma.pl\"]\nempty = []\nmixed = [1, true, \"x\"]\n").unwrap();
        let es = &d["review"];
        let Value::List(xs) = &es[0].value else { panic!() };
        assert!(matches!(&xs[..], [Value::Str(a), Value::Str(b)] if a == "@anna" && b == "piotr@firma.pl"));
        assert!(matches!(&es[1].value, Value::List(xs) if xs.is_empty()));
        assert!(matches!(&es[2].value, Value::List(xs) if xs.len() == 3));
        assert!(parse("[a]\nk = [1 2]\n").is_err());
    }
}
