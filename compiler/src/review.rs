// sowa review --base REF: zmiany w znaczeniu specyfikacji względem wersji z gita.
//
// Bazę kompilator wyjmuje przez `git archive` do katalogu tymczasowego i wczytuje jak zwykły
// projekt. Potem porównuje sowa.toml, typy i sygnatury z src/, przykłady, property i bloki
// sowa w docs/. Każda zmiana dostaje kategorię, a lista idzie od najbardziej ryzykownej:
// uprawnienie, osłabienie, usunięty test, rozszerzenie, zwykłe. Czy warunek jest luźniejszy,
// rozstrzyga solver (solver.rs). Kod w impl/ jest tylko liczony.

use crate::ast::*;
use crate::check::glob_match;
use crate::codegen::is_cap;
use crate::env::{Env, BUILTIN_RECORDS};
use crate::project::{self, MdFile, Project};
use crate::solver::{self, Num, Verdict};
use crate::toml::{Entry, Value};
use crate::{docslock, plural};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Cat {
    Cap,
    Weak,
    Test,
    Wide,
    Plain,
}

impl Cat {
    fn label(self) -> &'static str {
        match self {
            Cat::Cap => "uprawnienie",
            Cat::Weak => "osłabienie",
            Cat::Test => "usunięty test",
            Cat::Wide => "rozszerzenie",
            Cat::Plain => "zwykłe",
        }
    }
}

struct Change {
    cat: Cat,
    sym: String,
    loc: String,
    text: String,
    note: Option<String>,
    // Typy uprawnień, które zmiana daje funkcji.
    caps: Vec<String>,
}

// Jedna strona porównania: baza albo bieżący projekt.
struct Side<'a> {
    p: &'a Project,
    env: &'a Env<'a>,
    lines: RefCell<BTreeMap<String, Vec<String>>>,
}

impl<'a> Side<'a> {
    fn lines(&self, path: &str) -> Vec<String> {
        let mut c = self.lines.borrow_mut();
        c.entry(path.to_string())
            .or_insert_with(|| {
                std::fs::read_to_string(self.p.root.join(path))
                    .unwrap_or_default()
                    .lines()
                    .map(|l| l.to_string())
                    .collect()
            })
            .clone()
    }

    fn spec_types(&self) -> BTreeMap<&'a str, (&'a TypeDecl, &'a SourceFile)> {
        let mut m = BTreeMap::new();
        for f in self.p.files.iter().filter(|f| !f.is_impl) {
            for it in &f.items {
                if let Item::Type(t) = it {
                    m.insert(t.name.as_str(), (t, f));
                }
            }
        }
        m
    }

    fn spec_fns(&self) -> BTreeMap<&'a str, (&'a FnDecl, &'a SourceFile)> {
        let mut m = BTreeMap::new();
        for f in self.p.files.iter().filter(|f| !f.is_impl) {
            for it in &f.items {
                if let Item::Fn(d) = it {
                    m.insert(d.name.as_str(), (d, f));
                }
            }
        }
        m
    }

    fn is_refine(&self, t: &TypeExpr) -> bool {
        self.env.is_refine(t)
    }

    fn flatten(&self, t: &TypeExpr) -> (String, Vec<Expr>) {
        self.env.flatten(t)
    }

    fn field_ty(&self, rec: &str, field: &str) -> Option<TypeExpr> {
        if let Some((_, fs)) = BUILTIN_RECORDS.iter().find(|(r, _)| *r == rec) {
            let (_, t) = fs.iter().find(|(f, _)| *f == field)?;
            return Some(TypeExpr::Name {
                name: t.to_string(),
                args: vec![],
                cond: None,
                fields: None,
                line: 0,
            });
        }
        match self.env.types.get(rec) {
            Some((
                TypeDecl {
                    body: TypeBody::Record(fs), ..
                },
                _,
            )) => fs.iter().find(|f| f.name == field).map(|f| f.ty.clone()),
            _ => None,
        }
    }

    fn example_text(&self, f: &SourceFile, ex: &Example) -> String {
        if !ex.multiline {
            if let Some(Stmt::Expr { expr, .. }) = ex.stmts.first() {
                return expr.to_string();
            }
        }
        let ls = self.lines(&f.path);
        let mut parts = vec![];
        for l in ls.iter().skip(ex.line.saturating_sub(1)).take(ex.nlines.max(1)) {
            let t = l.trim();
            if !t.is_empty() && !t.starts_with("//") {
                parts.push(t.trim_start_matches("example").trim().to_string());
            }
        }
        parts.retain(|p| !p.is_empty());
        parts.join("; ")
    }

    fn tests(&self, d: &FnDecl, f: &SourceFile) -> Vec<(&'static str, String, usize)> {
        let mut out: Vec<(&'static str, String, usize)> =
            d.examples.iter().map(|e| ("example", self.example_text(f, e), e.line)).collect();
        out.extend(d.properties.iter().map(|p| ("property", p.expr.to_string(), p.line)));
        out
    }

    // Tekst ciała funkcji z impl/ bez komentarzy i pustych linii.
    fn body_text(&self, name: &str) -> Option<(String, String, usize)> {
        let (d, f) = self.env.fns.get(name)?.imp?;
        let ls = self.lines(&f.path);
        let next = f
            .items
            .iter()
            .map(|it| match it {
                Item::Type(t) => t.line,
                Item::Fn(x) => x.line,
            })
            .filter(|l| *l > d.line)
            .min()
            .unwrap_or(ls.len() + 1);
        let text: Vec<&str> = ls[d.line.saturating_sub(1)..(next - 1).min(ls.len())]
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with("//"))
            .collect();
        Some((text.join("\n"), f.path.clone(), d.line))
    }

    fn md_text(&self, md: &MdFile) -> Vec<String> {
        self.lines(&format!("{}/{}", self.p.docs_dir, md.rel))
    }

    // Sekcje pliku .md: (kotwica, linia nagłówka, tekst bez bloków sowa).
    fn sections(&self, md: &MdFile) -> Vec<(String, usize, String)> {
        let ls = self.md_text(md);
        let mut out: Vec<(String, usize, String)> = vec![(String::new(), 1, String::new())];
        let mut fence: Option<bool> = None;
        for (i, l) in ls.iter().enumerate() {
            let t = l.trim_start();
            if t.starts_with("```") {
                let sowa = match fence.take() {
                    Some(s) => s,
                    None => {
                        fence = Some(t[3..].trim() == "sowa");
                        fence == Some(true)
                    }
                };
                // Znaczniki bloków sowa nie liczą się do tekstu, innych bloków tak.
                if !sowa {
                    let last = out.last_mut().unwrap();
                    last.2.push_str(t.trim_end());
                    last.2.push('\n');
                }
                continue;
            }
            if fence == Some(true) {
                continue;
            }
            if fence.is_none() && t.starts_with('#') {
                out.push((project::slug(t.trim_start_matches('#')), i + 1, String::new()));
                continue;
            }
            let last = out.last_mut().unwrap();
            last.2.push_str(l.trim_end());
            last.2.push('\n');
        }
        for s in out.iter_mut() {
            s.2 = s.2.trim().to_string();
        }
        out.retain(|s| !(s.0.is_empty() && s.2.is_empty()));
        out
    }
}

// Warunek z nazwanym parametrem (`x => x > 0`) zamienia na α.
enum TyCmp {
    Same,
    Stricter,
    Looser(String),
    Unknown,
}

struct Rv<'a> {
    old: Side<'a>,
    new: Side<'a>,
    out: RefCell<Vec<Change>>,
}

const ARROW: &str = "  →  ";

impl<'a> Rv<'a> {
    fn push(&self, cat: Cat, sym: &str, loc: String, text: String, note: Option<String>) {
        self.out.borrow_mut().push(Change {
            cat,
            sym: sym.to_string(),
            loc,
            text,
            note,
            caps: vec![],
        });
    }

    fn push_cap(&self, sym: &str, loc: String, text: String, caps: Vec<String>) {
        self.out.borrow_mut().push(Change {
            cat: Cat::Cap,
            sym: sym.to_string(),
            loc,
            text,
            note: None,
            caps,
        });
    }

    // Czy term z warunku to liczba: α, α.pole, parametr funkcji.
    fn term_base(&self, e: &Expr, subject: &str, params: &[&Param]) -> Option<String> {
        match &e.kind {
            ExprKind::Ident(n) if n == "α" => Some(subject.to_string()),
            ExprKind::Ident(n) => {
                let p = params.iter().find(|p| &p.name == n)?;
                Some(self.new.flatten(&p.ty).0)
            }
            ExprKind::Field { obj, name } => {
                let b = self.term_base(obj, subject, params)?;
                let t = self.new.field_ty(&b, name).or_else(|| self.old.field_ty(&b, name))?;
                let (nb, _) = if self.new.env.is_type(&t.to_string()) { self.new.flatten(&t) } else { self.old.flatten(&t) };
                Some(nb)
            }
            _ => None,
        }
    }

    // Warianty unii bez danych, np. InvoiceStatus → Issued, Paid; None dla innych typów.
    fn domain(&self, ty: &str) -> Option<Vec<String>> {
        let env = self.new.env;
        let t = TypeExpr::Name {
            name: ty.to_string(),
            args: vec![],
            cond: None,
            fields: None,
            line: 0,
        };
        let ls = env.leaves(&t);
        let plain = |v: &String| env.variants.get(v).is_some_and(|i| i.fields.is_none()) || crate::env::BUILTIN_UNIONS.iter().any(|(_, vs)| vs.contains(&v.as_str()));
        (ls.len() > 1 && ls.iter().all(plain)).then_some(ls)
    }

    fn cmp_ty(&self, old: &TypeExpr, new: &TypeExpr, params: &[&Param]) -> TyCmp {
        let oa: Vec<(String, Vec<Expr>)> = old.alts().iter().map(|t| self.old.flatten(t)).collect();
        let na: Vec<(String, Vec<Expr>)> = new.alts().iter().map(|t| self.new.flatten(t)).collect();
        if oa.len() == 1 && na.len() == 1 && oa[0].0 != na[0].0 {
            return TyCmp::Unknown;
        }
        let mut added = vec![];
        let mut stricter = oa.iter().any(|(ob, _)| !na.iter().any(|(b, _)| b == ob));
        let mut looser = None;
        let mut unknown = false;
        for (b, nc) in &na {
            let Some((_, oc)) = oa.iter().find(|(ob, _)| ob == b) else {
                added.push(b.clone());
                continue;
            };
            let num = |e: &Expr| -> Option<Num> {
                match self.term_base(e, b, params)?.as_str() {
                    "Int" => Some(Num::Int),
                    "Money" => Some(Num::Money),
                    _ => None,
                }
            };
            let dom = |e: &Expr| -> Option<Vec<String>> { self.domain(&self.term_base(e, b, params)?) };
            match solver::implies(nc, oc, &num, &dom) {
                Verdict::Implied => {
                    if solver::implies(oc, nc, &num, &dom) != Verdict::Implied {
                        stricter = true;
                    }
                }
                Verdict::Counter(s) => {
                    looser.get_or_insert(format!("dopuszcza np. {}", s));
                }
                Verdict::Unknown => unknown = true,
            }
        }
        if !added.is_empty() {
            let w = if added.len() == 1 { "nowy wariant" } else { "nowe warianty" };
            return TyCmp::Looser(format!("{} {}", w, added.join(", ")));
        }
        if let Some(n) = looser {
            return TyCmp::Looser(n);
        }
        if unknown {
            TyCmp::Unknown
        } else if stricter {
            TyCmp::Stricter
        } else {
            TyCmp::Same
        }
    }

    fn emit(&self, c: TyCmp, looser: Cat, sym: &str, loc: String, text: String) {
        match c {
            TyCmp::Same => self.push(Cat::Plain, sym, loc, text, Some("to samo znaczenie".into())),
            TyCmp::Stricter => self.push(Cat::Plain, sym, loc, text, Some("ostrzejszy warunek".into())),
            TyCmp::Looser(n) => self.push(looser, sym, loc, text, Some(n)),
            TyCmp::Unknown => self.push(looser, sym, loc, text, Some("nie udało się porównać".into())),
        }
    }

    // `α <= 100  →  α <= 1000`, gdy zmienia się tylko warunek, a inaczej całe typy.
    fn change_text(old: &TypeExpr, new: &TypeExpr) -> String {
        // W unii z jednym zmienionym członem wystarczy ten człon.
        let (oa, na) = (old.alts(), new.alts());
        if oa.len() > 1 && oa.len() == na.len() {
            let diff: Vec<usize> = (0..oa.len()).filter(|i| oa[*i].to_string() != na[*i].to_string()).collect();
            if diff.len() == 1 {
                return Rv::change_text(oa[diff[0]], na[diff[0]]);
            }
        }
        if let (
            TypeExpr::Name {
                name: a,
                args: aa,
                cond: Some(ca),
                ..
            },
            TypeExpr::Name {
                name: b,
                args: ba,
                cond: Some(cb),
                ..
            },
        ) = (old, new)
        {
            if a == b && aa.len() == ba.len() {
                return format!("{}{}{}", ca.expr, ARROW, cb.expr);
            }
        }
        format!("{}{}{}", old, ARROW, new)
    }

    // ---------- sowa.toml ----------

    fn toml(&self) {
        let sec = |p: &'a Project, s: &str| -> &'a Vec<Entry> { p.toml.get(s).unwrap_or(empty_ref()) };
        for (s, cat_new, cat_changed, what) in [
            ("resources", Cat::Cap, Cat::Cap, "zasób"),
            ("resources.test", Cat::Test, Cat::Test, "zasób testowy"),
        ] {
            let (o, n) = (sec(self.old.p, s), sec(self.new.p, s));
            for e in n {
                let loc = format!("sowa.toml:{}", e.line);
                match o.iter().find(|x| x.key == e.key) {
                    None => self.push(cat_new, &e.key, loc, format!("nowy {} {}", what, res_desc(&e.value)), None),
                    Some(x) if val_text(&x.value) != val_text(&e.value) => self.push(
                        cat_changed,
                        &e.key,
                        loc,
                        format!("zmieniony {}: {}{}{}", what, res_desc(&x.value), ARROW, res_desc(&e.value)),
                        None,
                    ),
                    _ => {}
                }
            }
            for x in o {
                if !n.iter().any(|e| e.key == x.key) {
                    let cat = if s == "resources" { Cat::Plain } else { Cat::Test };
                    self.push(cat, &x.key, format!("sowa.toml:{}", x.line), format!("usunięty {}", what), None);
                }
            }
        }
        // Pozostałe sekcje. [review] mówi, kto zatwierdza, więc stoi przy uprawnieniach.
        let mut names: BTreeSet<&String> = self.new.p.toml.keys().collect();
        names.extend(self.old.p.toml.keys());
        for s in names {
            if s == "resources" || s == "resources.test" {
                continue;
            }
            let cat = if s == "review" { Cat::Cap } else { Cat::Plain };
            let (o, n) = (sec(self.old.p, s), sec(self.new.p, s));
            let label = |k: &str| if s.is_empty() { k.to_string() } else { format!("[{}] {}", s, k) };
            for e in n {
                let loc = format!("sowa.toml:{}", e.line);
                match o.iter().find(|x| x.key == e.key) {
                    None if !self.old.p.toml.is_empty() => {
                        self.push(cat, &e.key, loc, format!("{} = {}", label(&e.key), val_text(&e.value)), None)
                    }
                    Some(x) if val_text(&x.value) != val_text(&e.value) => self.push(
                        cat,
                        &e.key,
                        loc,
                        format!("{}: {}{}{}", label(&e.key), val_text(&x.value), ARROW, val_text(&e.value)),
                        None,
                    ),
                    _ => {}
                }
            }
            for x in o {
                if !n.iter().any(|e| e.key == x.key) {
                    self.push(cat, &x.key, format!("sowa.toml:{}", x.line), format!("usunięte {}", label(&x.key)), None);
                }
            }
        }
        // Atrapy: zmiana ciała zmienia to, wobec czego przechodzą testy.
        for e in sec(self.new.p, "resources.test") {
            let Some(fake) = e.value.str("fake") else { continue };
            let (Some((nt, path, line)), Some((ot, _, _))) = (self.new.body_text(fake), self.old.body_text(fake)) else {
                continue;
            };
            if nt != ot {
                self.push(Cat::Test, fake, format!("{}:{}", path, line), "zmienione ciało atrapy".into(), None);
            }
        }
    }

    // Adres zasobu przy parametrze-uprawnieniu: `(mail: smtp.firma.pl:587)`.
    fn annot(&self, pname: &str, ty: &str) -> String {
        let want = if ty == "DbRead" { "Db" } else { ty };
        let res = self.new.p.toml.get("resources").cloned().unwrap_or_default();
        let of_type: Vec<&Entry> = res.iter().filter(|e| e.value.str("type") == Some(want)).collect();
        let e = of_type
            .iter()
            .find(|e| e.key == pname)
            .or_else(|| if of_type.len() == 1 { of_type.first() } else { None });
        match e {
            Some(e) => match first_field(&e.value) {
                Some(v) => format!("{}: {}", e.key, v),
                None => e.key.clone(),
            },
            None => pname.to_string(),
        }
    }

    // ---------- typy ----------

    fn dirs(&self, sym: &str, loc: &str, od: &[Directive], nd: &[Directive]) {
        let text = |ds: &[Directive], k: &str| -> Vec<String> {
            ds.iter().filter(|d| d.kind == k).map(|d| d.text.split_whitespace().collect::<Vec<_>>().join(" ")).collect()
        };
        let (a, b) = (text(od, "desc"), text(nd, "desc"));
        if a != b {
            let w = if a.is_empty() {
                "nowy desc"
            } else if b.is_empty() {
                "usunięty desc"
            } else {
                "zmieniony desc"
            };
            self.push(Cat::Plain, sym, loc.to_string(), w.into(), None);
        }
        for k in ["doc", "why"] {
            let (a, b) = (text(od, k), text(nd, k));
            for x in b.iter().filter(|x| !a.contains(x)) {
                self.push(Cat::Plain, sym, loc.to_string(), format!("nowy {} {}", k, x), None);
            }
            for x in a.iter().filter(|x| !b.contains(x)) {
                self.push(Cat::Plain, sym, loc.to_string(), format!("usunięty {} {}", k, x), None);
            }
        }
    }

    // Funkcje ze specyfikacji, w których wyniku może stać ten człon unii.
    fn in_results(&self, term: &TypeExpr) -> Vec<String> {
        let leaves = self.new.env.leaves(term);
        let mut out = vec![];
        for (name, (d, _)) in self.new.spec_fns() {
            if let Some(r) = &d.ret {
                let rl = self.new.env.leaves(r);
                if leaves.iter().any(|l| rl.contains(l)) {
                    out.push(name.to_string());
                }
            }
        }
        out
    }

    fn fields(&self, sym: &str, path: &str, prefix: &str, of: &[Field], nf: &[Field]) {
        for f in nf {
            let loc = format!("{}:{}", path, f.line);
            match of.iter().find(|x| x.name == f.name) {
                None => self.push(Cat::Plain, sym, loc, format!("{}nowe pole {}: {}", prefix, f.name, f.ty), None),
                Some(x) if x.ty.to_string() != f.ty.to_string() => {
                    let c = self.cmp_ty(&x.ty, &f.ty, &[]);
                    self.emit(c, Cat::Weak, sym, loc, format!("{}pole {}: {}", prefix, f.name, Rv::change_text(&x.ty, &f.ty)));
                }
                _ => {}
            }
        }
        for x in of.iter().filter(|x| !nf.iter().any(|f| f.name == x.name)) {
            self.push(Cat::Plain, sym, format!("{}:{}", path, x.line), format!("{}usunięte pole {}", prefix, x.name), None);
        }
    }

    fn types(&self) {
        let (ot, nt) = (self.old.spec_types(), self.new.spec_types());
        for (name, (t, f)) in &nt {
            let loc = format!("{}:{}", f.path, t.line);
            let Some((o, _)) = ot.get(name) else {
                self.push(Cat::Plain, name, loc, "nowy typ".into(), None);
                continue;
            };
            self.dirs(name, &loc, &o.directives, &t.directives);
            match (&o.body, &t.body) {
                (TypeBody::Record(a), TypeBody::Record(b)) => self.fields(name, &f.path, "", a, b),
                (TypeBody::Rhs(a), TypeBody::Rhs(b))
                    if a.len() == 1 && b.len() == 1 && self.old.is_refine(&a[0]) && self.new.is_refine(&b[0]) =>
                {
                    if a[0].to_string() != b[0].to_string() {
                        let c = self.cmp_ty(&a[0], &b[0], &[]);
                        self.emit(c, Cat::Weak, name, loc, Rv::change_text(&a[0], &b[0]));
                    }
                }
                (TypeBody::Rhs(a), TypeBody::Rhs(b))
                    if !(a.len() == 1 && self.old.is_refine(&a[0])) && !(b.len() == 1 && self.new.is_refine(&b[0])) =>
                {
                    let key = |t: &TypeExpr| match t {
                        TypeExpr::Name { name, .. } => name.clone(),
                        t => t.to_string(),
                    };
                    // Warianty po rozwinięciu nazw unii: `type E = MailError` ma te same, co
                    // `type E = MailRejected | MailTimeout`.
                    let (ol, nl) = (
                        self.old.env.leaves(&TypeExpr::Union(a.clone())),
                        self.new.env.leaves(&TypeExpr::Union(b.clone())),
                    );
                    let line_of = |v: &str| {
                        b.iter()
                            .find_map(|x| match x {
                                TypeExpr::Name { name, line, .. } if name == v => Some(*line),
                                _ => None,
                            })
                            .unwrap_or(t.line)
                    };
                    for v in nl.iter().filter(|v| !ol.contains(v)) {
                        let tloc = format!("{}:{}", f.path, line_of(v));
                        let leaf = TypeExpr::Name {
                            name: v.clone(),
                            args: vec![],
                            cond: None,
                            fields: None,
                            line: 0,
                        };
                        let fns = self.in_results(&leaf);
                        if fns.is_empty() {
                            self.push(Cat::Plain, name, tloc, format!("nowy wariant {}", v), None);
                        } else {
                            let mut list = fns.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                            if fns.len() > 3 {
                                list.push_str(&format!(" i {} innych", fns.len() - 3));
                            }
                            self.push(Cat::Weak, name, tloc, format!("nowy wariant {}", v), Some(format!("w wyniku {}", list)));
                        }
                    }
                    for v in ol.iter().filter(|v| !nl.contains(v)) {
                        self.push(Cat::Plain, name, loc.clone(), format!("usunięty wariant {}", v), None);
                    }
                    // Ten sam człon po obu stronach: pola wariantu albo warunek.
                    for term in b {
                        let Some(x) = a.iter().find(|x| key(x) == key(term)) else { continue };
                        let tl = match term {
                            TypeExpr::Name { line, .. } => *line,
                            _ => t.line,
                        };
                        let tloc = format!("{}:{}", f.path, tl);
                        if let (TypeExpr::Name { fields: Some(fa), .. }, TypeExpr::Name { fields: Some(fb), .. }) = (x, term) {
                            self.fields(name, &f.path, &format!("{}: ", key(term)), fa, fb);
                        } else if x.to_string() != term.to_string() && self.new.is_refine(term) {
                            let c = self.cmp_ty(x, term, &[]);
                            self.emit(c, Cat::Weak, name, tloc, Rv::change_text(x, term));
                        }
                    }
                }
                _ => {
                    let show = |t: &TypeDecl| docslock::type_text(t).replace('\n', ";");
                    self.push(
                        Cat::Weak,
                        name,
                        loc,
                        format!("{}{}{}", show(o), ARROW, show(t)),
                        Some("nie udało się porównać".into()),
                    );
                }
            }
        }
        for (name, (o, f)) in &ot {
            if !nt.contains_key(name) {
                self.push(Cat::Plain, name, format!("{}:{}", f.path, o.line), "usunięty typ".into(), None);
            }
        }
    }

    // ---------- funkcje ----------

    fn fns(&self) {
        let (of, nf) = (self.old.spec_fns(), self.new.spec_fns());
        for (name, (d, f)) in &nf {
            let loc = format!("{}:{}", f.path, d.line);
            let Some((o, ofile)) = of.get(name) else {
                let caps: Vec<&Param> = d.params.iter().filter(|p| is_cap(&p.ty)).collect();
                if caps.is_empty() {
                    self.push(Cat::Plain, name, loc, "nowa funkcja".into(), None);
                } else {
                    let tys: Vec<String> = caps.iter().map(|p| p.ty.to_string()).collect();
                    let res: Vec<String> = caps.iter().map(|p| self.annot(&p.name, &p.ty.to_string())).collect();
                    let mut uniq = tys.clone();
                    uniq.dedup();
                    self.push_cap(name, loc, format!("nowa funkcja z {} ({})", uniq.join(", "), res.join(", ")), tys);
                }
                continue;
            };
            self.dirs(name, &loc, &o.directives, &d.directives);
            let params: Vec<&Param> = d.params.iter().chain(o.params.iter()).collect();
            for p in &d.params {
                let ploc = format!("{}:{}", f.path, p.line);
                let pt = p.ty.to_string();
                match o.params.iter().find(|x| x.name == p.name) {
                    None if is_cap(&p.ty) => self.push_cap(
                        name,
                        ploc,
                        format!("nowe uprawnienie {} ({})", pt, self.annot(&p.name, &pt)),
                        vec![pt.clone()],
                    ),
                    None => self.push(Cat::Plain, name, ploc, format!("nowy parametr {}: {}", p.name, pt), None),
                    Some(x) if x.ty.to_string() != pt => {
                        let xt = x.ty.to_string();
                        if is_cap(&x.ty) || is_cap(&p.ty) {
                            let narrower = xt == "Db" && pt == "DbRead" || !is_cap(&p.ty);
                            let text = format!("uprawnienie {}: {}{}{}", p.name, xt, ARROW, pt);
                            if narrower {
                                self.push(Cat::Plain, name, ploc, text, None);
                            } else {
                                let t = format!("{} ({})", text, self.annot(&p.name, &pt));
                                self.push_cap(name, ploc, t, vec![pt.clone()]);
                            }
                        } else {
                            let c = self.cmp_ty(&x.ty, &p.ty, &params);
                            self.emit(c, Cat::Wide, name, ploc, format!("parametr {}: {}", p.name, Rv::change_text(&x.ty, &p.ty)));
                        }
                    }
                    _ => {}
                }
            }
            for x in o.params.iter().filter(|x| !d.params.iter().any(|p| p.name == x.name)) {
                let t = if is_cap(&x.ty) {
                    format!("bez uprawnienia {}: {}", x.name, x.ty)
                } else {
                    format!("usunięty parametr {}", x.name)
                };
                self.push(Cat::Plain, name, loc.clone(), t, None);
            }
            match (&o.ret, &d.ret) {
                (Some(a), Some(b)) if a.to_string() != b.to_string() => {
                    let c = self.cmp_ty(a, b, &params);
                    self.emit(c, Cat::Weak, name, loc.clone(), format!("wynik: {}", Rv::change_text(a, b)));
                }
                (None, Some(b)) => self.push(Cat::Plain, name, loc.clone(), format!("wynik: brak{}{}", ARROW, b), None),
                (Some(a), None) => self.push(
                    Cat::Weak,
                    name,
                    loc.clone(),
                    format!("wynik: {}{}brak", a, ARROW),
                    Some("nie udało się porównać".into()),
                ),
                _ => {}
            }
            let (ot, nt) = (self.old.tests(o, ofile), self.new.tests(d, f));
            for (k, text, line) in &ot {
                if !nt.iter().any(|(k2, t2, _)| k2 == k && t2 == text) {
                    self.push(Cat::Test, name, format!("{}:{}", ofile.path, line), format!("{} {}", k, text), None);
                }
            }
            for (k, text, line) in &nt {
                if !ot.iter().any(|(k2, t2, _)| k2 == k && t2 == text) {
                    self.push(Cat::Plain, name, format!("{}:{}", f.path, line), format!("nowy {}", k), None);
                }
            }
        }
        for (name, (o, f)) in &of {
            if nf.contains_key(name) {
                continue;
            }
            let loc = format!("{}:{}", f.path, o.line);
            let (ne, np) = (o.examples.len(), o.properties.len());
            if ne + np > 0 {
                self.push(Cat::Test, name, loc, format!("usunięta funkcja ({} example, {} property)", ne, np), None);
            } else {
                self.push(Cat::Plain, name, loc, "usunięta funkcja".into(), None);
            }
        }
        // Nagłówki plików.
        for f in self.new.p.files.iter().filter(|f| !f.is_impl) {
            if let Some(o) = self.old.p.files.iter().find(|x| x.path == f.path) {
                self.dirs(&f.path, &format!("{}:1", f.path), &o.header, &f.header);
            }
        }
    }

    // ---------- docs/ ----------

    fn docs(&self) {
        let norm = |c: &str| -> String {
            c.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
        };
        let first = |c: &str| -> String {
            let l = c.lines().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or("");
            if c.lines().filter(|l| !l.trim().is_empty()).count() > 1 {
                format!("{} …", l)
            } else {
                l.to_string()
            }
        };
        let dd = |p: &Project| p.docs_dir.clone();
        for md in &self.new.p.docs {
            let path = format!("{}/{}", dd(self.new.p), md.rel);
            let Some(o) = self.old.p.docs.iter().find(|x| x.rel == md.rel) else {
                self.push(Cat::Plain, &md.rel, format!("{}:1", path), "nowy plik".into(), None);
                continue;
            };
            let opath = format!("{}/{}", dd(self.old.p), o.rel);
            for (line, code) in &o.blocks {
                if !md.blocks.iter().any(|(_, c)| norm(c) == norm(code)) {
                    let sym = docslock::section_of(o, *line);
                    self.push(Cat::Test, &sym, format!("{}:{}", opath, line), format!("blok sowa: {}", first(code)), None);
                }
            }
            for (line, code) in &md.blocks {
                if !o.blocks.iter().any(|(_, c)| norm(c) == norm(code)) {
                    let sym = docslock::section_of(md, *line);
                    self.push(Cat::Plain, &sym, format!("{}:{}", path, line), "nowy blok sowa".into(), None);
                }
            }
            let (os, ns) = (self.old.sections(o), self.new.sections(md));
            let sym = |a: &str| if a.is_empty() { md.rel.clone() } else { format!("{}#{}", md.rel, a) };
            for (a, line, text) in &ns {
                let loc = format!("{}:{}", path, line);
                match os.iter().find(|x| &x.0 == a) {
                    None => self.push(Cat::Plain, &sym(a), loc, "nowa sekcja".into(), None),
                    Some(x) if &x.2 != text => self.push(Cat::Plain, &sym(a), loc, "zmieniony tekst".into(), None),
                    _ => {}
                }
            }
            for (a, line, _) in os.iter().filter(|x| !ns.iter().any(|y| y.0 == x.0)) {
                self.push(Cat::Plain, &sym(a), format!("{}:{}", opath, line), "usunięta sekcja".into(), None);
            }
        }
        for o in &self.old.p.docs {
            if self.new.p.docs.iter().any(|x| x.rel == o.rel) {
                continue;
            }
            let opath = format!("{}/{}", dd(self.old.p), o.rel);
            self.push(Cat::Plain, &o.rel, format!("{}:1", opath), "usunięty plik".into(), None);
            for (line, code) in &o.blocks {
                let sym = docslock::section_of(o, *line);
                self.push(Cat::Test, &sym, format!("{}:{}", opath, line), format!("blok sowa: {}", first(code)), None);
            }
        }
    }

    // ---------- impl/ ----------

    // (zmienione pliki, zmienione linie, pliki z właścicielem w CODEOWNERS i ich zmienione linie)
    // Funkcje z ciałem w plikach impl/, których plik impl/ (a ze `spec` także plik src/)
    // zmienił się od bazy.
    fn changed_fns(&self, spec: bool) -> HashSet<String> {
        let changed = |path: &str| {
            let text = |s: &Side| if s.p.files.iter().any(|f| f.path == path) { s.lines(path) } else { vec![] };
            changed_lines(&text(&self.old), &text(&self.new)) > 0
        };
        self.new
            .env
            .fns
            .iter()
            .filter(|(n, i)| i.body().is_some() && *n != "main" && (changed(&i.imp.unwrap().1.path) || spec && i.spec.is_some_and(|(_, f)| changed(&f.path))))
            .map(|(n, _)| n.clone())
            .collect()
    }

    fn impl_stats(&self) -> (usize, usize, Vec<(String, usize)>) {
        let mut paths: BTreeSet<String> = BTreeSet::new();
        for s in [&self.old, &self.new] {
            paths.extend(s.p.files.iter().filter(|f| f.is_impl).map(|f| f.path.clone()));
        }
        let (mut files, mut lines, mut owned) = (0, 0, vec![]);
        for path in paths {
            let text = |s: &Side| -> Vec<String> {
                if s.p.files.iter().any(|f| f.path == path) {
                    s.lines(&path)
                } else {
                    vec![]
                }
            };
            let n = changed_lines(&text(&self.old), &text(&self.new));
            if n == 0 {
                continue;
            }
            files += 1;
            lines += n;
            if self.new.p.codeowners.iter().any(|(pat, _)| glob_match(pat, &path)) {
                owned.push((path, n));
            }
        }
        (files, lines, owned)
    }
}

fn empty_ref() -> &'static Vec<Entry> {
    static EMPTY: Vec<Entry> = Vec::new();
    &EMPTY
}

fn val_text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::List(xs) => format!("[{}]", xs.iter().map(val_text).collect::<Vec<_>>().join(", ")),
        Value::Table(kv) => {
            let a: Vec<String> = kv.iter().map(|(k, v)| format!("{} = {}", k, val_text(v))).collect();
            format!("{{ {} }}", a.join(", "))
        }
    }
}

fn first_field(v: &Value) -> Option<String> {
    let Value::Table(kv) = v else { return None };
    kv.iter().find_map(|(k, x)| match x {
        Value::Str(s) if k != "type" && !k.ends_with("_env") => Some(s.clone()),
        _ => None,
    })
}

// `Mailer: smtp.firma.pl:587, nadawca faktury@firma.pl, login z SMTP_LOGIN`
fn res_desc(v: &Value) -> String {
    let ty = v.str("type").unwrap_or("?");
    let mut parts = vec![];
    if let Value::Table(kv) = v {
        for (k, x) in kv {
            if k == "type" {
                continue;
            }
            let s = val_text(x);
            parts.push(match k.as_str() {
                "listen" => format!("nasłuch {}", s),
                "from" => format!("nadawca {}", s),
                "now" => format!("zegar zatrzymany na {}", s),
                "fake" => format!("atrapa {}", s),
                k if k.ends_with("_env") => format!("{} z {}", &k[..k.len() - 4], s),
                _ if s == "memory" => "w pamięci".into(),
                _ => s,
            });
        }
    }
    if parts.is_empty() {
        ty.to_string()
    } else {
        format!("{}: {}", ty, parts.join(", "))
    }
}

// Linie dodane plus usunięte, z najdłuższego wspólnego podciągu.
fn changed_lines(a: &[String], b: &[String]) -> usize {
    let mut s = 0;
    while s < a.len() && s < b.len() && a[s] == b[s] {
        s += 1;
    }
    let mut e = 0;
    while e < a.len() - s && e < b.len() - s && a[a.len() - 1 - e] == b[b.len() - 1 - e] {
        e += 1;
    }
    let (a, b) = (&a[s..a.len() - e], &b[s..b.len() - e]);
    if a.len() * b.len() > 16_000_000 {
        return a.len() + b.len();
    }
    let mut prev = vec![0u32; b.len() + 1];
    for x in a {
        let mut cur = vec![0u32; b.len() + 1];
        for (j, y) in b.iter().enumerate() {
            cur[j + 1] = if x == y { prev[j] + 1 } else { prev[j + 1].max(cur[j]) };
        }
        prev = cur;
    }
    a.len() + b.len() - 2 * prev[b.len()] as usize
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git: {}", e))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

// Projekt z REF w katalogu tymczasowym; None, gdy w REF go jeszcze nie było.
fn load_base(root: &Path, reference: &str, tmp: &Path) -> Result<Option<Project>, String> {
    let prefix = git(root, &["rev-parse", "--show-prefix"]).map_err(|e| format!("sowa review potrzebuje repozytorium git: {}", e))?;
    git(root, &["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", reference)])
        .map_err(|_| format!("--base {}: nie ma takiego commitu ani gałęzi", reference))?;
    // archive i ls-tree z podkatalogu biorą ścieżki od niego, więc idą z katalogu głównego.
    let top = PathBuf::from(git(root, &["rev-parse", "--show-toplevel"])?);
    let tree = format!("{}:{}", reference, prefix);
    if git(&top, &["cat-file", "-e", &format!("{}sowa.toml", tree)]).is_err() {
        return Ok(None);
    }
    let _ = std::fs::remove_dir_all(tmp);
    std::fs::create_dir_all(tmp).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    let tar = tmp.join("base.tar");
    git(&top, &["archive", "--format=tar", "-o", &tar.to_string_lossy(), &tree])?;
    let dir = tmp.join("base");
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    let st = Command::new("tar")
        .arg("-xf")
        .arg(&tar)
        .arg("-C")
        .arg(&dir)
        .status()
        .map_err(|e| format!("tar: {}", e))?;
    if !st.success() {
        return Err("tar nie rozpakował bazy".into());
    }
    project::load(&dir).map(Some).map_err(|ds| {
        let msgs: Vec<String> = ds.iter().take(5).map(|d| d.to_string()).collect();
        format!("--base {}: projekt w bazie się nie wczytuje:\n  {}", reference, msgs.join("\n  "))
    })
}

fn empty_project(p: &Project) -> Project {
    Project {
        root: PathBuf::new(),
        name: p.name.clone(),
        toml: Default::default(),
        src_dir: p.src_dir.clone(),
        impl_dir: p.impl_dir.clone(),
        docs_dir: p.docs_dir.clone(),
        files: vec![],
        docs: vec![],
        codeowners: vec![],
        gitattributes: vec![],
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    format!("{}{}", s, " ".repeat(w.saturating_sub(n)))
}

pub fn review(p: &Project, env: &Env, reference: &str, mutate: bool) -> Result<String, String> {
    let tmp = std::env::temp_dir().join(format!("sowa-review-{}", std::process::id()));
    let res = load_base(&p.root, reference, &tmp).map(|base| report(p, env, base, mutate));
    let _ = std::fs::remove_dir_all(&tmp);
    res
}

// `warunki wyniku: 3 udowodnione, 1 sprawdzany w runtime (line_net)` dla zmienionych funkcji
// z warunkiem w typie wyniku. Warunek bez dowodu sprawdza runtime przy każdym return.
fn proofs(env: &Env, fns: &HashSet<String>) -> String {
    let mut fns: Vec<&String> = fns.iter().collect();
    fns.sort();
    let res: Vec<(&String, bool)> = fns.into_iter().filter_map(|f| crate::prove::prove(env, f).map(|ok| (f, ok))).collect();
    if res.is_empty() {
        return String::new();
    }
    let open: Vec<&str> = res.iter().filter(|(_, ok)| !ok).map(|(f, _)| f.as_str()).collect();
    let mut parts = vec![];
    if open.len() < res.len() {
        parts.push(plural(res.len() - open.len(), "udowodniony", "udowodnione", "udowodnionych"));
    }
    if !open.is_empty() {
        parts.push(format!(
            "{} w runtime ({})",
            plural(open.len(), "sprawdzany", "sprawdzane", "sprawdzanych"),
            open.join(", ")
        ));
    }
    format!("      warunki wyniku: {}\n", parts.join(", "))
}

fn report(p: &Project, env: &Env, base: Option<Project>, mutate: bool) -> String {
    let fresh = base.is_none();
    let base = base.unwrap_or_else(|| empty_project(p));
    let benv = Env::build(&base.files);
    let rv = Rv {
        old: Side {
            p: &base,
            env: &benv,
            lines: RefCell::new(BTreeMap::new()),
        },
        new: Side {
            p,
            env,
            lines: RefCell::new(BTreeMap::new()),
        },
        out: RefCell::new(vec![]),
    };
    rv.toml();
    rv.types();
    rv.fns();
    rv.docs();
    let mut all = rv.out.take();
    // Kolejność kategorii jest stała, a w kategorii zostaje kolejność z plików.
    all.sort_by_key(|c| c.cat);
    let summary = if fresh {
        let n = all.iter().filter(|c| c.cat == Cat::Plain).count();
        all.retain(|c| c.cat != Cat::Plain);
        let ts = rv.new.spec_types().len();
        let fs = rv.new.spec_fns();
        let pure = fs.values().filter(|(d, _)| !d.params.iter().any(|x| is_cap(&x.ty))).count();
        let ex: usize = fs.values().map(|(d, _)| d.examples.len()).sum();
        let pr: usize = fs.values().map(|(d, _)| d.properties.len()).sum();
        let bl: usize = p.docs.iter().map(|m| m.blocks.len()).sum();
        (n > 0).then(|| {
            format!(
                "{}, {}, {} example, {} property, {} sowa w {}/",
                plural(ts, "typ", "typy", "typów"),
                plural(pure, "czysta funkcja", "czyste funkcje", "czystych funkcji"),
                ex,
                pr,
                plural(bl, "blok", "bloki", "bloków"),
                p.docs_dir
            )
        })
    } else {
        None
    };

    let mut o = String::new();
    let head = format!("Specyfikacja ({}/, {}/, sowa.toml)", p.src_dir, p.docs_dir);
    let count = plural(all.len(), "zmiana", "zmiany", "zmian");
    if fresh {
        let rest = if summary.is_some() { ", reszta zwykła" } else { "" };
        o.push_str(&format!("{}: nowy projekt, {}{}\n", head, count, rest));
    } else if all.is_empty() {
        o.push_str(&format!("{}: bez zmian\n", head));
    } else {
        o.push_str(&format!("{}: {}\n", head, count));
    }
    if !all.is_empty() {
        o.push('\n');
    }
    let iw = format!("[{}]", all.len()).len().max(4);
    // Bardzo długie symbole i miejsca (sekcje .md) wychodzą poza kolumnę, żeby nie rozpychać reszty.
    let fits: Vec<&Change> = all.iter().filter(|c| c.sym.chars().count() <= 24).collect();
    let sw = fits.iter().map(|c| c.sym.chars().count()).max().unwrap_or(0) + 2;
    let lw = fits.iter().map(|c| c.loc.chars().count()).max().unwrap_or(0) + 3;
    let indent = 1 + iw + 1 + 16 + sw + lw;
    let last_cap = all.iter().rposition(|c| c.cat == Cat::Cap);
    for (i, c) in all.iter().enumerate() {
        o.push_str(&format!(
            " {:>iw$} {}{}{}{}\n",
            format!("[{}]", i + 1),
            pad(c.cat.label(), 16),
            pad(&c.sym, sw.max(c.sym.chars().count() + 2)),
            pad(&c.loc, lw.max(c.loc.chars().count() + 2)),
            c.text,
            iw = iw
        ));
        if let Some(n) = &c.note {
            o.push_str(&format!("{}{}\n", " ".repeat(indent), n));
        }
        if Some(i) == last_cap {
            o.push_str(&holders(&all, &rv.new));
        }
    }
    if let Some(s) = summary {
        o.push_str(&format!("{}zwykłe: {}\n", " ".repeat(6), s));
    }

    let (files, lines, owned) = rv.impl_stats();
    o.push('\n');
    let code = format!("Kod ({}/)", p.impl_dir);
    if files == 0 {
        o.push_str(&format!("{}: bez zmian.\n", code));
    } else if owned.is_empty() {
        o.push_str(&format!(
            "{}: {}, {}, nie wymaga przeglądu.\n",
            code,
            plural(files, "plik", "pliki", "plików"),
            plural(lines, "linia", "linie", "linii")
        ));
    } else {
        o.push_str(&format!(
            "{}: {}, {}.\n",
            code,
            plural(files, "plik", "pliki", "plików"),
            plural(lines, "linia", "linie", "linii")
        ));
        let list: Vec<String> = owned.iter().map(|(f, n)| format!("{} ({})", f, plural(*n, "linia", "linie", "linii"))).collect();
        o.push_str(&format!("      do przeczytania (CODEOWNERS): {}\n", list.join(", ")));
        if owned.len() < files {
            o.push_str("      reszta nie wymaga przeglądu\n");
        }
    }
    o.push_str(&proofs(env, &rv.changed_fns(true)));
    if mutate {
        let fns = rv.changed_fns(false);
        if fns.is_empty() {
            o.push_str("      mutacje: zmiany nie dotykają ciał funkcji\n");
        } else {
            match crate::mutate::run(p, env, fns) {
                Ok(r) => o.push_str(&format!("      {}", crate::mutate::text(&r, "      "))),
                Err(e) => o.push_str(&format!("      mutacje: {}\n", e.lines().last().unwrap_or(""))),
            }
        }
    }

    match docslock::pending(p, env) {
        None => o.push_str(&format!("\nNie ma {}: opisy w {}/ nie są śledzone.\n", docslock::FILE, p.docs_dir)),
        Some(pd) if pd.is_empty() => {}
        Some(pd) => {
            // Po jednym wierszu na plik: `plik.md: cały plik, #sekcja, #sekcja`.
            let mut by: Vec<(String, Vec<String>)> = vec![];
            for x in &pd {
                let (file, part) = match x.target.split_once('#') {
                    Some((f, a)) => (f.to_string(), format!("#{}", a)),
                    None => (x.target.clone(), "cały plik".to_string()),
                };
                match by.iter_mut().find(|(f, _)| *f == file) {
                    Some((_, ps)) => ps.push(part),
                    None => by.push((file, vec![part])),
                }
            }
            let line = |(f, ps): &(String, Vec<String>)| format!("{}{}", f, if ps == &["cały plik"] { String::new() } else { format!(": {}", ps.join(", ")) });
            if by.len() == 1 && by[0].1.len() == 1 {
                o.push_str(&format!("\nOpisy do przejrzenia ({}): {}\n", docslock::FILE, pd[0].target));
            } else {
                o.push_str(&format!("\nOpisy do przejrzenia ({}):\n", docslock::FILE));
                for b in &by {
                    o.push_str(&format!("      {}\n", line(b)));
                }
            }
            o.push_str("      po przejrzeniu: sowa review --confirm-docs\n");
        }
    }
    o
}

// `Http (ksef) mają tylko: main, handle, post_ksef` dla uprawnień, które pojawiły się w zmianie.
fn holders(all: &[Change], new: &Side) -> String {
    let mut tys: Vec<String> = vec![];
    for c in all {
        for t in &c.caps {
            if !tys.contains(t) {
                tys.push(t.clone());
            }
        }
    }
    let mut o = String::new();
    let res = new.p.toml.get("resources").cloned().unwrap_or_default();
    let mut first = true;
    for t in tys {
        let mut who = vec![];
        for f in new.p.files.iter().filter(|f| !f.is_impl) {
            for it in &f.items {
                if let Item::Fn(d) = it {
                    if d.params.iter().any(|p| p.ty.to_string() == t) {
                        who.push(d.name.clone());
                    }
                }
            }
        }
        let want = if t == "DbRead" { "Db" } else { t.as_str() };
        let names: Vec<&str> = res.iter().filter(|e| e.value.str("type") == Some(want)).map(|e| e.key.as_str()).collect();
        let label = if names.is_empty() { t.clone() } else { format!("{} ({})", t, names.join(", ")) };
        if first {
            o.push('\n');
            first = false;
        }
        let verb = if who.len() == 1 { "ma" } else { "mają" };
        o.push_str(&format!("      {} {} tylko: {}\n", label, verb, who.join(", ")));
    }
    if !first {
        o.push('\n');
    }
    o
}
