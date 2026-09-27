// docs.lock: które opisy w docs/ człowiek przejrzał i przy jakiej wersji kodu.
//
// Opis to sekcja .md (plik#kotwica) albo cały plik. Z symbolem łączy go `doc` albo `why` przy
// definicji w src/, `doc`/`why` w nagłówku pliku (symbolem jest wtedy ścieżka pliku) albo
// {Symbol} w tekście sekcji. Wiersz w docs.lock zapamiętuje hash symbolu z chwili przeglądu:
// sygnaturę funkcji z warunkami i uprawnieniami, definicję typu albo, dla nagłówka pliku,
// wszystkie definicje w pliku. Gdy hash się zmieni, opis trzeba przejrzeć jeszcze raz.

use crate::ast::*;
use crate::env::{Env, BUILTIN_UNIONS};
use crate::project::{MdFile, Project};
use std::collections::BTreeMap;

pub const FILE: &str = "docs.lock";

#[derive(Clone)]
pub struct Link {
    pub target: String,
    pub symbol: String,
    pub via: &'static str,
    pub hash: String,
    pub what: &'static str,
}

pub struct Entry {
    pub target: String,
    pub symbol: String,
    pub via: String,
    pub hash: String,
    pub who: String,
    pub date: String,
    pub line: usize,
}

fn fnv(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)[..6].to_string()
}

pub fn type_text(t: &TypeDecl) -> String {
    match &t.body {
        TypeBody::Rhs(ts) => {
            let a: Vec<String> = ts.iter().map(|x| x.to_string()).collect();
            format!("type {} = {}", t.name, a.join(" | "))
        }
        TypeBody::Record(fs) => {
            let a: Vec<String> = fs.iter().map(|f| format!("  {}: {}", f.name, f.ty)).collect();
            format!("type {}\n{}", t.name, a.join("\n"))
        }
    }
}

fn item_text(it: &Item) -> String {
    match it {
        Item::Type(t) => type_text(t),
        Item::Fn(d) => d.signature(),
    }
}

// Hash symbolu i to, co się zmienia, gdy hash jest inny.
fn symbol_hash(p: &Project, env: &Env, name: &str) -> (String, &'static str) {
    if let Some(f) = p.files.iter().find(|f| !f.is_impl && f.path == name) {
        let all: Vec<String> = f.items.iter().map(item_text).collect();
        return (fnv(&all.join("\n")), "zmieniły się definicje w pliku");
    }
    if let Some(info) = env.fns.get(name) {
        return (fnv(&info.decl().signature()), "zmieniła się sygnatura");
    }
    if let Some((t, _)) = env.types.get(name) {
        return (fnv(&type_text(t)), "zmienił się typ");
    }
    // Wariant: hash typu, w którym stoi.
    for (t, _) in env.types.values() {
        if let TypeBody::Rhs(ts) = &t.body {
            if ts.iter().any(|x| matches!(x, TypeExpr::Name { name: n, .. } if n == name)) {
                return (fnv(&type_text(t)), "zmienił się typ");
            }
        }
    }
    if let Some((u, vs)) = BUILTIN_UNIONS.iter().find(|(_, vs)| vs.contains(&name)) {
        return (fnv(&format!("{} = {}", u, vs.join(" | "))), "zmienił się typ");
    }
    (fnv(name), "zmienił się symbol")
}

// Sekcja, w której stoi linia: najbliższy nagłówek nad nią, a przed pierwszym nagłówkiem cały plik.
pub fn section_of(md: &MdFile, line: usize) -> String {
    match md.headings.iter().rev().find(|(_, l)| *l <= line) {
        Some((a, _)) => format!("{}#{}", md.rel, a),
        None => md.rel.clone(),
    }
}

pub fn links(p: &Project, env: &Env) -> Vec<Link> {
    let mut raw: Vec<(String, String, &'static str)> = vec![];
    for f in p.files.iter().filter(|f| !f.is_impl) {
        for d in &f.header {
            if d.kind == "doc" || d.kind == "why" {
                raw.push((d.text.trim().to_string(), f.path.clone(), if d.kind == "doc" { "doc" } else { "why" }));
            }
        }
        for it in &f.items {
            let (name, ds) = match it {
                Item::Type(t) => (&t.name, &t.directives),
                Item::Fn(d) => (&d.name, &d.directives),
            };
            for d in ds {
                if d.kind == "doc" || d.kind == "why" {
                    raw.push((d.text.trim().to_string(), name.clone(), if d.kind == "doc" { "doc" } else { "why" }));
                }
            }
        }
    }
    for md in &p.docs {
        for (name, line) in &md.refs {
            raw.push((section_of(md, *line), name.clone(), "{}"));
        }
    }
    raw.sort();
    raw.dedup();
    raw.into_iter()
        .map(|(target, symbol, via)| {
            let (hash, what) = symbol_hash(p, env, &symbol);
            Link {
                target,
                symbol,
                via,
                hash,
                what,
            }
        })
        .collect()
}

// None, gdy projekt nie ma docs.lock.
pub fn read(p: &Project) -> Option<Vec<Entry>> {
    let src = std::fs::read_to_string(p.root.join(FILE)).ok()?;
    let mut out = vec![];
    for (i, l) in src.lines().enumerate() {
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let c: Vec<&str> = t.split_whitespace().collect();
        if c.len() < 6 {
            continue;
        }
        out.push(Entry {
            target: c[0].into(),
            symbol: c[1].into(),
            via: c[2].into(),
            hash: c[3].into(),
            who: c[4].into(),
            date: c[5].into(),
            line: i + 1,
        });
    }
    Some(out)
}

pub struct Pending {
    pub target: String,
    // (symbol, powód); powód None znaczy „jeszcze nie przeglądany”
    pub symbols: Vec<(String, Option<&'static str>)>,
    pub line: usize,
}

// Sekcje do przejrzenia: nowe powiązania i zmienione hashe. None, gdy nie ma docs.lock.
pub fn pending(p: &Project, env: &Env) -> Option<Vec<Pending>> {
    let lock = read(p)?;
    let mut by: BTreeMap<String, Pending> = BTreeMap::new();
    for l in links(p, env) {
        let e = lock.iter().find(|e| e.target == l.target && e.symbol == l.symbol && e.via == l.via);
        let reason = match e {
            Some(e) if e.hash == l.hash => continue,
            Some(_) => Some(l.what),
            None => None,
        };
        let pd = by.entry(l.target.clone()).or_insert(Pending {
            target: l.target.clone(),
            symbols: vec![],
            line: 0,
        });
        if pd.line == 0 {
            pd.line = e.map(|e| e.line).unwrap_or(0);
        }
        if !pd.symbols.iter().any(|(s, _)| *s == l.symbol) {
            pd.symbols.push((l.symbol, reason));
        }
    }
    Some(by.into_values().collect())
}

// Uwagi dla `sowa check`.
pub fn warnings(p: &Project, env: &Env) -> Vec<Diag> {
    let mut out = vec![];
    for pd in pending(p, env).unwrap_or_default() {
        let changed: Vec<String> = pd
            .symbols
            .iter()
            .filter_map(|(s, r)| r.map(|r| format!("{} ({})", s, r)))
            .collect();
        let new: Vec<&str> = pd.symbols.iter().filter(|(_, r)| r.is_none()).map(|(s, _)| s.as_str()).collect();
        let msg = if changed.is_empty() {
            format!("{}/{} nie był jeszcze przeglądany (powiązania: {})", p.docs_dir, pd.target, new.join(", "))
        } else if new.is_empty() {
            format!("{}/{} nie był przeglądany od zmiany {}", p.docs_dir, pd.target, changed.join(", "))
        } else {
            format!(
                "{}/{} nie był przeglądany od zmiany {}; nowe powiązania: {}",
                p.docs_dir,
                pd.target,
                changed.join(", "),
                new.join(", ")
            )
        };
        out.push(Diag::new(FILE, pd.line, msg));
    }
    out
}

// Zapisuje docs.lock: wiersze dla sekcji z `only` (wszystkich, gdy puste) dostają bieżący hash,
// kto i datę. Pozostałe zostają jak były, a wiersze bez powiązania znikają.
// Zwraca potwierdzone sekcje.
pub fn confirm(p: &Project, env: &Env, only: &[String], who: &str, date: &str) -> Result<Vec<String>, String> {
    let old = read(p).unwrap_or_default();
    let todo: Vec<String> = pending(p, env)
        .unwrap_or_else(|| {
            // Bez docs.lock wszystko jest do przejrzenia.
            let mut t: Vec<String> = links(p, env).into_iter().map(|l| l.target).collect();
            t.dedup();
            t.into_iter()
                .map(|target| Pending {
                    target,
                    symbols: vec![],
                    line: 0,
                })
                .collect()
        })
        .into_iter()
        .map(|pd| pd.target)
        .collect();
    for o in only {
        if !links(p, env).iter().any(|l| &l.target == o) {
            return Err(format!("{}: {} nie jest powiązany z żadnym symbolem", FILE, o));
        }
    }
    let chosen: Vec<String> = if only.is_empty() { todo.clone() } else { only.to_vec() };
    let mut rows: Vec<[String; 6]> = vec![];
    for l in links(p, env) {
        let prev = old.iter().find(|e| e.target == l.target && e.symbol == l.symbol && e.via == l.via);
        let row = if chosen.contains(&l.target) {
            match prev {
                Some(e) if e.hash == l.hash => [l.target, l.symbol, l.via.into(), l.hash, e.who.clone(), e.date.clone()],
                _ => [l.target, l.symbol, l.via.into(), l.hash, who.into(), date.into()],
            }
        } else if let Some(e) = prev {
            [l.target, l.symbol, l.via.into(), e.hash.clone(), e.who.clone(), e.date.clone()]
        } else {
            continue;
        };
        rows.push(row);
    }
    let head = ["# opis", "symbol", "przez", "hash", "kto", "przejrzane"];
    let mut w = [0usize; 5];
    for r in rows.iter().cloned().chain(std::iter::once(head.map(|x| x.to_string()))) {
        for (i, c) in r.iter().take(5).enumerate() {
            w[i] = w[i].max(c.chars().count());
        }
    }
    let line = |r: &[String; 6]| {
        let mut s = String::new();
        for (i, c) in r.iter().enumerate() {
            s.push_str(c);
            if i < 5 {
                s.push_str(&" ".repeat(w[i] - c.chars().count() + 2));
            }
        }
        s.trim_end().to_string()
    };
    let mut out = String::from(
        "# Generuje `sowa review --confirm-docs`, nie edytuj ręcznie. Plik należy do właściciela w CODEOWNERS.\n\
         # przez: doc / why = odnośnik z kodu do .md, {} = odnośnik {Symbol} z .md do kodu.\n\
         # hash: sygnatura funkcji, definicja typu albo, dla nagłówka pliku, wszystkie definicje w pliku.\n",
    );
    out.push_str(&line(&head.map(|x| x.to_string())));
    out.push('\n');
    for r in &rows {
        out.push_str(&line(r));
        out.push('\n');
    }
    std::fs::write(p.root.join(FILE), out).map_err(|e| format!("{}: {}", FILE, e))?;
    let mut done: Vec<String> = chosen.into_iter().filter(|t| todo.contains(t)).collect();
    done.dedup();
    Ok(done)
}
