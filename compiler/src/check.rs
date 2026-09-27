// sowa check: reguły projektu, których nie pilnuje sama składnia.
// Zwraca (błędy, uwagi). Nieznane nazwy i wywołania zgłasza też codegen.

use crate::ast::*;
use crate::codegen::is_cap;
use crate::env::*;
use crate::project::{MdFile, Project};
use std::collections::{BTreeMap, HashSet};

pub fn resource_names(p: &Project, section: &str) -> Vec<String> {
    p.toml.get(section).map(|es| es.iter().map(|e| e.key.clone()).collect()).unwrap_or_default()
}

struct Ck<'a> {
    p: &'a Project,
    // Typ parametru lambdy w db.transaction(tx => ...).
    db_ty: &'a TypeExpr,
    env: &'a Env<'a>,
    errs: Vec<Diag>,
    warns: Vec<Diag>,
    // Głębokość lambd: return w lambdzie nie wychodzi z funkcji, więc nie sprawdza typu wyniku.
    lam: usize,
}

pub fn check(p: &Project, env: &Env) -> (Vec<Diag>, Vec<Diag>) {
    let db_ty: &TypeExpr = Box::leak(Box::new(TypeExpr::Name {
        name: "Db".into(),
        args: vec![],
        cond: None,
        fields: None,
        line: 0,
    }));
    let mut ck = Ck {
        p,
        env,
        db_ty,
        errs: vec![],
        warns: vec![],
        lam: 0,
    };
    ck.signatures();
    ck.resources();
    ck.docs();
    for f in &p.files {
        for it in &f.items {
            match it {
                Item::Fn(d) => {
                    ck.fn_types(d, f);
                    ck.directives(&d.directives, &d.order, f);
                    ck.sizes(d, f);
                    if let Some(body) = &d.body {
                        let mut sc = Scopes::new();
                        for pr in &d.params {
                            sc.declare(&pr.name, false, pr.line, Some(pr.ty.clone()));
                        }
                        ck.block(body, &mut sc, f, d);
                        ck.unused(&mut sc, f, true);
                    }
                }
                Item::Type(t) => {
                    ck.directives(&t.directives, &[], f);
                    ck.type_decl(t, f);
                }
            }
        }
        for d in &f.header {
            ck.directive_ref(d, f);
        }
        for d in &f.stray {
            ck.errs
                .push(Diag::new(&f.path, d.line, format!("`{}` bez typu ani funkcji pod spodem", d.kind)));
        }
    }
    (ck.errs, ck.warns)
}

struct Var {
    name: String,
    is_var: bool,
    used: bool,
    line: usize,
    // Typ z sygnatury (parametr) albo wywnioskowany z wartości; None, gdy nieznany.
    ty: Option<TypeExpr>,
}

struct Scopes {
    s: Vec<Vec<Var>>,
    done: Vec<Var>,
}

impl Scopes {
    fn new() -> Self {
        Scopes { s: vec![vec![]], done: vec![] }
    }
    fn find(&mut self, n: &str) -> Option<&mut Var> {
        self.s.iter_mut().rev().flat_map(|s| s.iter_mut()).find(|v| v.name == n)
    }
    fn declare(&mut self, n: &str, is_var: bool, line: usize, ty: Option<TypeExpr>) {
        self.s.last_mut().unwrap().push(Var {
            name: n.to_string(),
            is_var,
            used: false,
            line,
            ty,
        });
    }
    fn push(&mut self) {
        self.s.push(vec![]);
    }
    fn pop(&mut self) {
        let s = self.s.pop().unwrap();
        self.done.extend(s);
    }
}

impl<'a> Ck<'a> {
    fn err(&mut self, f: &str, line: usize, m: impl Into<String>) {
        self.errs.push(Diag::new(f, line, m));
    }

    // ---------- src a impl ----------

    fn signatures(&mut self) {
        for (name, info) in &self.env.fns {
            match (info.spec, info.imp) {
                (Some((s, sf)), Some((i, imf))) => {
                    if s.signature() != i.signature() {
                        self.err(
                            &imf.path,
                            i.line,
                            format!(
                                "sygnatura różni się od specyfikacji ({}:{}):\n  src:  {}\n  impl: {}",
                                sf.path,
                                s.line,
                                s.signature(),
                                i.signature()
                            ),
                        );
                    }
                    if sf.module != imf.module {
                        self.err(
                            &imf.path,
                            i.line,
                            format!(
                                "{} jest w specyfikacji {}, więc jego ciało należy do {}/{}.sowa",
                                name, sf.path, self.p.impl_dir, sf.module
                            ),
                        );
                    }
                    if s.body.is_some() {
                        self.err(&sf.path, s.line, "specyfikacja w src/ nie ma ciała funkcji");
                    }
                    if !i.examples.is_empty() || !i.properties.is_empty() {
                        // Przykłady w impl są dozwolone dla funkcji prywatnych; tu to podwójne miejsce.
                        self.warns
                            .push(Diag::new(&imf.path, i.line, "przykłady funkcji ze specyfikacją należą do src/"));
                    }
                }
                (Some((s, sf)), None) => {
                    self.err(&sf.path, s.line, format!("{} nie ma ciała w {}/{}.sowa", name, self.p.impl_dir, sf.module));
                }
                (None, Some((i, imf))) => {
                    if i.body.is_none() {
                        self.err(&imf.path, i.line, format!("{} nie ma ciała", name));
                    }
                }
                (None, None) => {}
            }
        }
        // Funkcje prywatne (tylko w impl) wołane z innego modułu.
        for f in &self.p.files {
            for it in &f.items {
                if let Item::Fn(d) = it {
                    if let Some(b) = &d.body {
                        let mut calls = vec![];
                        for s in b {
                            stmt_calls(s, &mut calls);
                        }
                        for (c, line) in calls {
                            if let Some(info) = self.env.fns.get(&c) {
                                if info.is_private() && info.module() != f.module {
                                    let m = format!(
                                        "{} jest prywatna w {}/{}.sowa; żeby wołać ją z {}, dodaj ją do {}/{}.sowa",
                                        c,
                                        self.p.impl_dir,
                                        info.module(),
                                        f.module,
                                        self.p.src_dir,
                                        info.module()
                                    );
                                    self.err(&f.path, line, m);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ---------- typy w sygnaturach ----------

    fn fn_types(&mut self, d: &FnDecl, f: &SourceFile) {
        let mut names: HashSet<&str> = HashSet::new();
        for p in &d.params {
            if !names.insert(&p.name) {
                self.err(&f.path, p.line, format!("parametr {} powtarza się", p.name));
            }
            self.type_known(&p.ty, f);
            self.no_nested_cap(&p.ty, f, false);
        }
        if let Some(r) = &d.ret {
            self.type_known(r, f);
            self.no_nested_cap(r, f, true);
        }
    }

    fn type_known(&mut self, t: &TypeExpr, f: &SourceFile) {
        for a in t.alts() {
            if let TypeExpr::Name { name, args, line, .. } = a {
                if !self.env.is_type(name) && !self.env.variants.contains_key(name) {
                    self.err(&f.path, *line, format!("nieznany typ {}", name));
                }
                if name == "List" && args.len() != 1 {
                    self.err(&f.path, *line, "List bierze jeden typ: List<T>");
                }
                for x in args {
                    self.type_known(x, f);
                }
            }
        }
    }

    fn no_nested_cap(&mut self, t: &TypeExpr, f: &SourceFile, in_ret: bool) {
        for a in t.alts() {
            if let TypeExpr::Name { name, args, line, .. } = a {
                if CAPS.contains(&name.as_str()) && in_ret {
                    self.err(&f.path, *line, format!("{} to uprawnienie: nie może być wynikiem funkcji", name));
                }
                for x in args {
                    if is_cap(x) {
                        self.err(&f.path, *line, "uprawnienia nie mogą być elementami list");
                    }
                    self.no_nested_cap(x, f, in_ret);
                }
            }
        }
    }

    fn type_decl(&mut self, t: &TypeDecl, f: &SourceFile) {
        match &t.body {
            TypeBody::Record(fs) => {
                let mut seen = HashSet::new();
                for fl in fs {
                    if !seen.insert(&fl.name) {
                        self.err(&f.path, fl.line, format!("pole {} powtarza się", fl.name));
                    }
                    if is_cap(&fl.ty) {
                        self.err(&f.path, fl.line, "uprawnienie nie może być polem rekordu");
                    }
                    self.type_known(&fl.ty, f);
                }
            }
            TypeBody::Rhs(terms) => {
                for term in terms {
                    if let TypeExpr::Name { name, fields, args, .. } = term {
                        if self.env.is_type(name) {
                            for x in args {
                                self.type_known(x, f);
                            }
                        }
                        for fl in fields.iter().flatten() {
                            if is_cap(&fl.ty) {
                                self.err(&f.path, fl.line, "uprawnienie nie może być polem wariantu");
                            }
                            self.type_known(&fl.ty, f);
                        }
                    }
                }
            }
        }
    }

    // ---------- zasoby i main ----------

    fn resources(&mut self) {
        let res = resource_names(self.p, "resources");
        let test = resource_names(self.p, "resources.test");
        let Some(main) = self.env.fns.get("main") else {
            self.err("sowa.toml", 0, "projekt nie ma funkcji main");
            return;
        };
        let (d, f) = main.spec.or(main.imp).unwrap();
        let params: Vec<&str> = d.params.iter().map(|p| p.name.as_str()).collect();
        if params != res.iter().map(|s| s.as_str()).collect::<Vec<_>>() {
            self.err(
                &f.path,
                d.line,
                format!(
                    "parametry main ({}) muszą odpowiadać zasobom z [resources] w sowa.toml ({})",
                    params.join(", "),
                    res.join(", ")
                ),
            );
        }
        for p in &d.params {
            if !is_cap(&p.ty) {
                self.err(&f.path, p.line, format!("main dostaje tylko uprawnienia, a {} ma typ {}", p.name, p.ty));
                continue;
            }
            let TypeExpr::Name { name: ty, .. } = &p.ty else { continue };
            for (sec, list) in [("resources", &res), ("resources.test", &test)] {
                if !list.contains(&p.name) {
                    continue;
                }
                let entry = self.p.toml[sec].iter().find(|e| e.key == p.name).unwrap();
                let declared = entry.value.str("type").unwrap_or("");
                if declared != ty {
                    self.err(
                        "sowa.toml",
                        entry.line,
                        format!("[{}] {}: type = \"{}\", a main ma {}: {}", sec, p.name, declared, p.name, ty),
                    );
                }
            }
        }
        for t in &test {
            if !res.contains(t) {
                let line = self.p.toml["resources.test"].iter().find(|e| &e.key == t).unwrap().line;
                self.err("sowa.toml", line, format!("[resources.test] {}: nie ma go w [resources]", t));
            }
        }
        // Atrapy HTTP: czysta funkcja (HttpRequest) -> HttpResponse, pod opieką CODEOWNERS.
        let entries = self.p.toml.get("resources.test").cloned().unwrap_or_default();
        for e in entries {
            let Some(fake) = e.value.str("fake") else { continue };
            let Some(info) = self.env.fns.get(fake) else {
                self.err("sowa.toml", e.line, format!("atrapa {} nie istnieje", fake));
                continue;
            };
            let d = info.decl();
            let ok = d.params.len() == 1 && d.params[0].ty.to_string() == "HttpRequest" && d.ret.as_ref().map(|r| r.to_string()) == Some("HttpResponse".into());
            if !ok {
                self.err(
                    "sowa.toml",
                    e.line,
                    format!("atrapa {} musi mieć sygnaturę fn {}(req: HttpRequest) -> HttpResponse", fake, fake),
                );
            }
            if let Some((_, f)) = info.imp {
                if !self.p.codeowners.iter().any(|pat| glob_match(pat, &f.path)) {
                    self.err(
                        &f.path,
                        0,
                        format!("ciało atrapy {} musi mieć właściciela w CODEOWNERS: człowiek czyta atrapy", fake),
                    );
                }
                let generated = self
                    .p
                    .gitattributes
                    .iter()
                    .any(|(pat, attrs)| glob_match(pat, &f.path) && attrs.iter().any(|a| a == "-linguist-generated"));
                if !generated {
                    self.err(
                        &f.path,
                        0,
                        format!("{} musi mieć -linguist-generated w .gitattributes, żeby był widoczny w diffie", f.path),
                    );
                }
            }
        }
    }

    // ---------- dokumentacja ----------

    fn doc_target(&mut self, text: &str, file: &str, line: usize) {
        let t = text.trim();
        let (path, anchor) = match t.split_once('#') {
            Some((p, a)) => (p, Some(a)),
            None => (t, None),
        };
        let Some(md) = self.p.docs.iter().find(|m| m.rel == path) else {
            self.err(file, line, format!("nie ma pliku {}/{}", self.p.docs_dir, path));
            return;
        };
        if let Some(a) = anchor {
            if !md.anchors.iter().any(|x| x == a) {
                self.err(file, line, format!("{}/{} nie ma nagłówka #{}", self.p.docs_dir, path, a));
            }
        }
    }

    fn directive_ref(&mut self, d: &Directive, f: &SourceFile) {
        match d.kind.as_str() {
            "doc" | "why" => self.doc_target(&d.text, &f.path, d.line),
            "desc" => {
                for name in brace_refs(&d.text) {
                    if !self.symbol_exists(&name) {
                        self.err(&f.path, d.line, format!("desc odwołuje się do {{{}}}, którego nie ma", name));
                    }
                }
            }
            _ => {}
        }
    }

    fn symbol_exists(&self, n: &str) -> bool {
        self.env.is_type(n) || self.env.variants.contains_key(n) || self.env.fns.contains_key(n) || builtin_fn(n).is_some()
    }

    fn directives(&mut self, ds: &[Directive], order: &[(String, usize)], f: &SourceFile) {
        for d in ds {
            self.directive_ref(d, f);
            if d.kind == "desc" && d.lines > 3 {
                self.warns
                    .push(Diag::new(&f.path, d.line, format!("desc ma {} linie; dłuższy opis należy do docs/", d.lines)));
            }
        }
        const RANK: [&str; 5] = ["desc", "doc", "why", "example", "property"];
        let mut last = 0;
        for (k, line) in order {
            let Some(r) = RANK.iter().position(|x| x == k) else { continue };
            if r < last {
                self.err(
                    &f.path,
                    *line,
                    format!("`{}` stoi po `{}`; kolejność to desc, doc, why, example, property", k, RANK[last]),
                );
            }
            last = last.max(r);
        }
    }

    fn sizes(&mut self, d: &FnDecl, f: &SourceFile) {
        if d.examples.len() + d.properties.len() > 3 {
            self.warns.push(Diag::new(
                &f.path,
                d.line,
                format!(
                    "{} ma {} przykładów i property; więcej niż 3 zwykle znaczy, że reguła należy do docs/",
                    d.name,
                    d.examples.len() + d.properties.len()
                ),
            ));
        }
        for ex in &d.examples {
            if ex.nlines > 5 {
                self.warns.push(Diag::new(
                    &f.path,
                    ex.line,
                    format!("przykład ma {} linii; dłuższe scenariusze należą do docs/", ex.nlines),
                ));
            }
        }
    }

    fn docs(&mut self) {
        let docs: Vec<&MdFile> = self.p.docs.iter().collect();
        for md in docs {
            for (name, line) in &md.refs {
                if !self.symbol_exists(name) {
                    let path = format!("{}/{}", self.p.docs_dir, md.rel);
                    self.err(&path, *line, format!("odnośnik {{{}}} nie wskazuje typu ani funkcji", name));
                }
            }
        }
    }

    // ---------- ciała funkcji ----------

    fn unused(&mut self, sc: &mut Scopes, f: &SourceFile, all: bool) {
        if all {
            while sc.s.len() > 0 {
                sc.pop();
            }
        }
        for v in sc.done.drain(..) {
            if !v.used && !v.name.starts_with('_') {
                self.warns.push(Diag::new(&f.path, v.line, format!("{} nie jest używane", v.name)));
            }
        }
    }

    fn block(&mut self, stmts: &'a [Stmt], sc: &mut Scopes, f: &SourceFile, d: &FnDecl) {
        sc.push();
        for s in stmts {
            self.stmt(s, sc, f, d);
        }
        sc.pop();
    }

    fn stmt(&mut self, s: &'a Stmt, sc: &mut Scopes, f: &SourceFile, d: &FnDecl) {
        match s {
            Stmt::Set { name, is_var, expr, line } => {
                self.expr(expr, sc, f, d);
                let t = self.local_ty(expr, sc);
                if *is_var {
                    if sc.find(name).is_some() {
                        self.err(&f.path, *line, format!("{} jest już zdefiniowane; nazwy się nie przesłaniają", name));
                    }
                    sc.declare(name, true, *line, t);
                } else {
                    match sc.find(name) {
                        Some(v) if v.is_var => {
                            v.used = true;
                            let old = v.ty.clone();
                            self.reassign(name, old, t, *line, sc, f);
                        }
                        Some(_) => self.err(&f.path, *line, format!("{} jest niezmienne, użyj var", name)),
                        None => sc.declare(name, false, *line, t),
                    }
                }
            }
            Stmt::Return { expr, line } => {
                if let Some(e) = expr {
                    self.expr(e, sc, f, d);
                    self.ret_fits(e, *line, sc, f, d);
                }
            }
            Stmt::Expr { expr, .. } => self.expr(expr, sc, f, d),
            Stmt::If { cond, then, els, line } => {
                self.expr(cond, sc, f, d);
                self.want_bool(cond, "warunek if", *line, sc, f);
                self.block(then, sc, f, d);
                if let Some(e) = els {
                    self.block(e, sc, f, d);
                }
            }
            Stmt::While { cond, body, line } => {
                self.expr(cond, sc, f, d);
                self.want_bool(cond, "warunek while", *line, sc, f);
                self.block(body, sc, f, d);
            }
            Stmt::For { var, iter, body, line } => {
                self.expr(iter, sc, f, d);
                let el = self.ty(iter, sc).and_then(|t| self.elem(&t));
                sc.push();
                self.bind(var, *line, sc, f, el);
                self.block(body, sc, f, d);
                sc.pop();
            }
            Stmt::Match { subjects, arms, line } => {
                for e in subjects {
                    self.expr(e, sc, f, d);
                }
                let tys: Vec<Option<TypeExpr>> = subjects.iter().map(|e| self.local_ty(e, sc)).collect();
                for arm in arms {
                    sc.push();
                    for (k, p) in arm.pats.iter().enumerate() {
                        let t = tys.get(k).cloned().flatten();
                        self.pat(p, t, arm.line, sc, f);
                    }
                    self.block(&arm.body, sc, f, d);
                    sc.pop();
                }
                self.exhaustive(subjects, arms, *line, sc, f);
            }
        }
    }

    fn bind(&mut self, n: &str, line: usize, sc: &mut Scopes, f: &SourceFile, ty: Option<TypeExpr>) {
        if sc.find(n).is_some() {
            self.err(&f.path, line, format!("{} jest już zdefiniowane; nazwy się nie przesłaniają", n));
        }
        sc.declare(n, false, line, ty);
    }

    // Wzorzec dostaje typ podmiotu: `x` cały, `[a, b]` element listy, `Wariant v` sam wariant.
    fn pat(&mut self, p: &Pat, ty: Option<TypeExpr>, line: usize, sc: &mut Scopes, f: &SourceFile) {
        match p {
            Pat::Wild | Pat::Str(_) => {}
            Pat::Bind(n) => self.bind(n, line, sc, f, ty),
            Pat::List(ps) => {
                let el = ty.and_then(|t| self.elem(&t));
                for p in ps {
                    self.pat(p, el.clone(), line, sc, f);
                }
            }
            Pat::Name { name, bind } => {
                if !self.env.variants.contains_key(name) && !self.env.is_type(name) {
                    self.err(&f.path, line, format!("nieznany wariant albo typ {}", name));
                }
                if let Some(b) = bind {
                    self.bind(b, line, sc, f, Some(named(name)));
                }
            }
        }
    }

    // Typ nowej zmiennej lokalnej. Uprawnienia idą dalej tylko przez nazwę parametru, więc zmienna
    // z przypisanym uprawnieniem nie dostaje jego typu.
    fn local_ty(&self, e: &Expr, sc: &mut Scopes) -> Option<TypeExpr> {
        self.ty(e, sc).filter(|t| !is_cap(t))
    }

    // `x = wartość` dla var: podstawa typu nie może się zmienić. Int i Money się mieszają, a zmienna
    // z Int, do której trafia Money, ma odtąd typ Money. Lista bez znanego elementu (`[]`) dostaje
    // typ pierwszej listy ze znanym elementem.
    fn reassign(&mut self, n: &str, old: Option<TypeExpr>, new: Option<TypeExpr>, line: usize, sc: &mut Scopes, f: &SourceFile) {
        let (Some(old), Some(new)) = (old, new) else { return };
        let (Some(a), Some(b)) = (self.base(&old, 0), self.base(&new, 0)) else { return };
        let upgrade = if fits(&a, &b) {
            a == "List" && self.elem(&old).is_none() && self.elem(&new).is_some()
        } else if fits(&b, &a) {
            true
        } else {
            self.err(&f.path, line, format!("{} ma typ {}, a dostaje {}", n, old, b));
            false
        };
        if upgrade {
            if let Some(v) = sc.find(n) {
                v.ty = Some(new);
            }
        }
    }

    // return w funkcji: wartość o znanej podstawie typu musi pasować do któregoś liścia wyniku.
    // Liść bez podstawy (typ ogólny, unia bez nazwy) wyłącza sprawdzenie, a wariant nie pasuje do
    // wartości prostej ani rekordu.
    fn ret_fits(&mut self, e: &Expr, line: usize, sc: &mut Scopes, f: &SourceFile, d: &FnDecl) {
        if self.lam > 0 {
            return;
        }
        let (Some(ret), Some(have)) = (&d.ret, self.kind(e, sc)) else { return };
        for leaf in self.env.leaves(ret) {
            match self.base(&named(&leaf), 0) {
                Some(b) if fits(&b, &have) => return,
                Some(_) => {}
                None if self.env.variants.contains_key(&leaf) => {}
                None => return,
            }
        }
        self.err(&f.path, line, format!("{} zwraca {}, a return daje {}", d.name, ret, have));
    }

    fn want_bool(&mut self, e: &Expr, what: &str, line: usize, sc: &mut Scopes, f: &SourceFile) {
        if let Some(k) = self.kind(e, sc).filter(|k| k != "Bool") {
            self.err(&f.path, line, format!("{} musi być Bool, a jest {}", what, k));
        }
    }

    // match po wyniku funkcji albo zmiennej o znanym typie: każdy liść typu musi mieć gałąź (albo `_`).
    fn exhaustive(&mut self, subjects: &[Expr], arms: &[Arm], line: usize, sc: &mut Scopes, f: &SourceFile) {
        if arms.iter().any(|a| a.pats.iter().all(|p| matches!(p, Pat::Wild | Pat::Bind(_)))) {
            return;
        }
        for (k, subj) in subjects.iter().enumerate() {
            let Some(ty) = self.ty(subj, sc) else { continue };
            let leaves = self.env.leaves(&ty);
            if leaves.iter().any(|l| PRIMS.contains(&l.as_str()) || l == "List") {
                continue;
            }
            let mut covered: HashSet<String> = HashSet::new();
            for a in arms {
                match a.pats.get(k) {
                    Some(Pat::Wild) | Some(Pat::Bind(_)) => return,
                    Some(Pat::Name { name, .. }) => {
                        covered.insert(name.clone());
                        if let Some((d, _)) = self.env.types.get(name) {
                            let t = TypeExpr::Name {
                                name: d.name.clone(),
                                args: vec![],
                                cond: None,
                                fields: None,
                                line: 0,
                            };
                            covered.extend(self.env.leaves(&t));
                        }
                    }
                    _ => {}
                }
            }
            if subjects.len() > 1 {
                continue;
            }
            let missing: Vec<&String> = leaves.iter().filter(|l| !covered.contains(*l)).collect();
            if !missing.is_empty() {
                let m: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
                self.err(&f.path, line, format!("match nie obsługuje: {} (dodaj gałęzie albo `_`)", m.join(", ")));
            }
        }
    }

    // Typ wyrażenia, gdy da się go ustalić bez pełnej inferencji: stałe, zmienne (parametry i lokalne),
    // pola, wywołania, konstruktory, operatory, `as`, `with`, `try` i metody list. W innych razach None.
    fn ty(&self, e: &Expr, sc: &mut Scopes) -> Option<TypeExpr> {
        let n = |s: &str| Some(named(s));
        match &e.kind {
            ExprKind::Int(_) => n("Int"),
            ExprKind::Dec(_) => n("Money"),
            ExprKind::Str(_) => n("String"),
            ExprKind::Bool(_) | ExprKind::Not(_) | ExprKind::Is { .. } => n("Bool"),
            ExprKind::Html(_) => n("Html"),
            ExprKind::List(xs) => Some(list_of(xs.iter().find_map(|x| self.ty(x, sc)))),
            ExprKind::Ident(x) => match sc.find(x) {
                Some(v) => v.ty.clone(),
                None if self.env.variants.contains_key(x) => n(x),
                None => None,
            },
            ExprKind::Field { obj, name } => {
                let t = self.ty(obj, sc)?;
                self.field_ty(&t, name)
            }
            ExprKind::Call { name, args } => {
                if let Some(i) = self.env.fns.get(name) {
                    return i.decl().ret.clone();
                }
                if self.env.record_fields(name).is_some() || self.env.variants.contains_key(name) {
                    return n(name);
                }
                let arg = |k: usize, sc: &mut Scopes| args.get(k).and_then(|a| self.ty(&a.value, sc));
                match name.as_str() {
                    "len" => n("Int"),
                    "to_string" | "to_json" | "trim" | "lower" | "upper" | "remove" | "drop_prefix" | "pad_left" | "join" | "char" => n("String"),
                    "starts_with" | "contains" | "matches" | "only_digits" | "nip_checksum_ok" | "valid_email" => n("Bool"),
                    "segments" | "split" | "chars" => Some(list_of(n("String"))),
                    "at" => arg(0, sc).and_then(|t| self.elem(&t)),
                    "distinct" | "sort_by" => arg(0, sc),
                    _ => None,
                }
            }
            ExprKind::Method { obj, name, args, .. } => {
                let t = self.ty(obj, sc)?;
                match (self.base(&t, 0)?.as_str(), name.as_str()) {
                    ("List", "filter" | "reverse") => Some(t),
                    ("List", "map") => {
                        let el = self.elem(&t);
                        let r = match args.first().map(|a| &a.value.kind) {
                            Some(ExprKind::Lambda {
                                param,
                                body: LambdaBody::Expr(x),
                            }) => {
                                sc.push();
                                sc.declare(param, false, 0, el);
                                let r = self.ty(x, sc);
                                sc.s.pop();
                                r
                            }
                            _ => None,
                        };
                        Some(list_of(r))
                    }
                    ("Clock", "now") => n("DateTime"),
                    ("Clock", "today") => n("Date"),
                    ("Random", "int") => n("Int"),
                    _ => None,
                }
            }
            ExprKind::Bin { op, l, r } => match *op {
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => n("Bool"),
                _ => {
                    let (lt, rt) = (self.ty(l, sc)?, self.ty(r, sc)?);
                    let (a, b) = (self.base(&lt, 0)?, self.base(&rt, 0)?);
                    match (a.as_str(), b.as_str()) {
                        ("Int", "Int") => n("Int"),
                        ("Int" | "Money", "Int" | "Money") => n("Money"),
                        ("String", "String") if *op == "+" => n("String"),
                        ("List", "List") if *op == "+" => Some(if self.elem(&lt).is_some() { lt } else { rt }),
                        _ => None,
                    }
                }
            },
            ExprKind::Neg(x) => self.ty(x, sc),
            ExprKind::As { ty, .. } => Some(ty.clone()),
            ExprKind::With { e: x, .. } => self.ty(x, sc),
            // `try f(...)` daje pierwszy wariant wyniku: resztę, czyli błędy, zabiera try.
            ExprKind::Try(x) => match self.ty(x, sc)? {
                TypeExpr::Union(v) => v.into_iter().next(),
                t => Some(t),
            },
            ExprKind::Lambda { .. } => None,
        }
    }

    // Typ pola rekordu, wbudowanego rekordu albo wariantu z danymi.
    fn field_ty(&self, t: &TypeExpr, name: &str) -> Option<TypeExpr> {
        if let Some(r) = self.base(t, 0) {
            if let Some(fs) = self.record(&r) {
                return fs.iter().find(|x| x.name == name).map(|x| x.ty.clone());
            }
            if let Some((_, fs)) = BUILTIN_RECORDS.iter().find(|(b, _)| *b == r) {
                return fs.iter().find(|(x, _)| *x == name).map(|(_, t)| named(t));
            }
        }
        let TypeExpr::Name { name: v, .. } = t else { return None };
        let fs = self.env.variants.get(v)?.fields?;
        fs.iter().find(|x| x.name == name).map(|x| x.ty.clone())
    }

    // Typ elementu listy, także za aliasem (`type Lines = List<Line>`); None, gdy nieznany.
    fn elem(&self, t: &TypeExpr) -> Option<TypeExpr> {
        let mut t = t;
        for _ in 0..20 {
            let TypeExpr::Name { name, args, .. } = t else { return None };
            if name == "List" {
                return args.first().cloned();
            }
            match self.env.types.get(name) {
                Some((TypeDecl { body: TypeBody::Rhs(ts), .. }, _)) if ts.len() == 1 => t = &ts[0],
                _ => return None,
            }
        }
        None
    }

    fn expr(&mut self, e: &'a Expr, sc: &mut Scopes, f: &SourceFile, d: &FnDecl) {
        match &e.kind {
            ExprKind::Ident(n) => {
                if let Some(v) = sc.find(n) {
                    v.used = true;
                }
            }
            ExprKind::Html(segs) => {
                for s in segs {
                    if let HtmlSeg::Expr(x) = s {
                        self.expr(x, sc, f, d);
                    }
                }
            }
            ExprKind::List(xs) => {
                for x in xs {
                    self.expr(x, sc, f, d);
                }
            }
            ExprKind::Call { name, args } => {
                // Lambda w sort_by(lista, x => ...) dostaje element listy.
                let el = match args.first() {
                    Some(a) if !self.env.fns.contains_key(name) => self.ty(&a.value, sc).and_then(|t| self.elem(&t)),
                    _ => None,
                };
                for a in args {
                    self.lambda(&a.value, el.clone(), sc, f, d);
                }
                self.constructor(name, args, e.line, f);
                if self.record(name).is_some() && !self.env.fns.contains_key(name) {
                    let given: Vec<(&str, &Expr)> =
                        args.iter().filter_map(|a| a.name.as_deref().map(|n| (n, &a.value))).collect();
                    self.field_types(name, &given, e.line, sc, f);
                }
                if let Some(info) = self.env.fns.get(name) {
                    self.cap_args(info.decl(), args, sc, e.line, f);
                    self.arg_types(info.decl(), args, sc, e.line, f);
                } else if let Some((n, _)) = builtin_fn(name) {
                    if args.len() != n {
                        self.err(&f.path, e.line, format!("{} bierze {} argumentów, a dostał {}", name, n, args.len()));
                    }
                }
            }
            ExprKind::Method { obj, name, args, .. } => {
                self.expr(obj, sc, f, d);
                let ot = self.ty(obj, sc);
                // Parametr lambdy: Db w transaction, element listy w map i filter.
                let pt = match name.as_str() {
                    "transaction" => Some(self.db_ty.clone()),
                    "map" | "filter" => ot.as_ref().and_then(|t| self.elem(t)),
                    _ => None,
                };
                for a in args {
                    self.lambda(&a.value, pt.clone(), sc, f, d);
                }
                if let (ExprKind::Ident(o), Some(TypeExpr::Name { name: t, .. })) = (&obj.kind, &ot) {
                    if let Some((_, ms)) = CAP_METHODS.iter().find(|(c, _)| c == t) {
                        if !ms.contains(&name.as_str()) {
                            self.err(&f.path, e.line, format!("{} ({}) nie ma metody {}; dostępne: {}", o, t, name, ms.join(", ")));
                        }
                    }
                }
                if self.kind(obj, sc).as_deref() == Some("List") && !LIST_METHODS.contains(&name.as_str()) {
                    self.err(&f.path, e.line, format!("lista nie ma metody {}; dostępne: {}", name, LIST_METHODS.join(", ")));
                }
            }
            ExprKind::Field { obj, .. } => self.expr(obj, sc, f, d),
            ExprKind::Not(x) => {
                self.expr(x, sc, f, d);
                self.want_bool(x, "argument !", e.line, sc, f);
            }
            ExprKind::Neg(x) => {
                self.expr(x, sc, f, d);
                if let Some(k) = self.kind(x, sc).filter(|k| !is_num(k)) {
                    self.err(&f.path, e.line, format!("minus działa na liczbach, a dostaje {}", k));
                }
            }
            ExprKind::Bin { op, l, r } => {
                self.expr(l, sc, f, d);
                self.expr(r, sc, f, d);
                if *op == "==" || *op == "!=" {
                    for side in [l, r] {
                        if let ExprKind::Ident(n) = &side.kind {
                            if sc.find(n).is_none() && self.env.variant_fields(n).is_some() {
                                self.err(&f.path, e.line, format!("{} ma dane; porównanie z nim to `is {}`", n, n));
                            }
                        }
                    }
                }
                self.bin_types(op, l, r, e.line, sc, f);
            }
            ExprKind::Is { e: x, .. } => self.expr(x, sc, f, d),
            ExprKind::As { e: x, alt, .. } => {
                self.expr(x, sc, f, d);
                match alt.as_deref() {
                    Some(Alt::Value(v)) => self.expr(v, sc, f, d),
                    Some(Alt::Return(Some(v))) => {
                        self.expr(v, sc, f, d);
                        self.ret_fits(v, e.line, sc, f, d);
                    }
                    Some(Alt::Block(b)) => self.block(b, sc, f, d),
                    _ => {}
                }
            }
            ExprKind::With { e: x, fields } => {
                self.expr(x, sc, f, d);
                for (_, v) in fields {
                    self.expr(v, sc, f, d);
                }
                if let Some(r) = self.kind(x, sc).filter(|r| self.record(r).is_some()) {
                    let given: Vec<(&str, &Expr)> = fields.iter().map(|(n, v)| (n.as_str(), v)).collect();
                    self.field_types(&r, &given, e.line, sc, f);
                }
            }
            ExprKind::Lambda { .. } => self.lambda(e, None, sc, f, d),
            ExprKind::Try(x) => {
                self.expr(x, sc, f, d);
                self.try_errors(x, e.line, f, d);
            }
            _ => {}
        }
    }

    // Lambda z parametrem typu pt (None: nieznany); inne wyrażenie zwyczajnie.
    fn lambda(&mut self, e: &'a Expr, pt: Option<TypeExpr>, sc: &mut Scopes, f: &SourceFile, d: &FnDecl) {
        let ExprKind::Lambda { param, body } = &e.kind else {
            return self.expr(e, sc, f, d);
        };
        sc.push();
        self.bind(param, e.line, sc, f, pt);
        self.lam += 1;
        match body {
            LambdaBody::Expr(x) => self.expr(x, sc, f, d),
            LambdaBody::Block(b) => self.block(b, sc, f, d),
        }
        self.lam -= 1;
        sc.pop();
    }

    // Operatory na wartościach o znanej podstawie typu. Int i Money liczą się razem, `+` łączy też
    // teksty i listy, a porównania wymagają tego samego typu po obu stronach.
    fn bin_types(&mut self, op: &str, l: &Expr, r: &Expr, line: usize, sc: &mut Scopes, f: &SourceFile) {
        if op == "&&" || op == "||" {
            for x in [l, r] {
                if let Some(k) = self.kind(x, sc).filter(|k| k != "Bool") {
                    self.err(&f.path, line, format!("{} łączy wartości Bool, a dostaje {}", op, k));
                }
            }
            return;
        }
        let (Some(a), Some(b)) = (self.kind(l, sc), self.kind(r, sc)) else { return };
        let ok = match op {
            "==" | "!=" => fits(&a, &b) || fits(&b, &a),
            "<" | ">" | "<=" | ">=" => (fits(&a, &b) || fits(&b, &a)) && ["Int", "Money", "String", "Date", "DateTime"].contains(&a.as_str()),
            "+" => (is_num(&a) && is_num(&b)) || (a == b && (a == "String" || a == "List")),
            _ => is_num(&a) && is_num(&b),
        };
        if !ok {
            self.err(&f.path, line, format!("{} nie działa na {} i {}", op, a, b));
        }
    }

    // Argument o znanej podstawie typu musi pasować do parametru; uprawnienia sprawdza cap_args.
    fn arg_types(&mut self, callee: &FnDecl, args: &[Arg], sc: &mut Scopes, line: usize, f: &SourceFile) {
        let mut i = 0;
        for a in args {
            let p = match &a.name {
                Some(n) => callee.params.iter().find(|p| &p.name == n),
                None => {
                    i += 1;
                    callee.params.get(i - 1)
                }
            };
            let Some(p) = p else { continue };
            let (Some(want), Some(have)) = (self.base(&p.ty, 0), self.kind(&a.value, sc)) else { continue };
            if !fits(&want, &have) {
                self.err(&f.path, line, format!("{} ma parametr {}: {}, a dostaje {}", callee.name, p.name, p.ty, have));
            }
        }
    }

    // Uprawnienie przekazane dalej musi pasować: Db można dać tam, gdzie DbRead.
    fn cap_args(&mut self, callee: &FnDecl, args: &[Arg], sc: &mut Scopes, line: usize, f: &SourceFile) {
        let mut i = 0;
        for a in args {
            let p = match &a.name {
                Some(n) => callee.params.iter().find(|p| &p.name == n),
                None => {
                    i += 1;
                    callee.params.get(i - 1)
                }
            };
            let Some(p) = p else { continue };
            let TypeExpr::Name { name: want, .. } = &p.ty else { continue };
            if !CAPS.contains(&want.as_str()) {
                continue;
            }
            let have = match &a.value.kind {
                ExprKind::Ident(n) => {
                    sc.find(n)
                        .and_then(|v| v.ty.as_ref())
                        .and_then(|t| if let TypeExpr::Name { name, .. } = t { Some(name.clone()) } else { None })
                }
                _ => None,
            };
            match have {
                Some(h) if &h == want || (h == "Db" && want == "DbRead") => {}
                Some(h) => self.err(&f.path, line, format!("{} ma parametr {}: {}, a dostaje {}", callee.name, p.name, want, h)),
                None => self.err(
                    &f.path,
                    line,
                    format!(
                        "{} ma parametr {}: {}; uprawnienie przekazuje się tylko przez nazwę parametru",
                        callee.name, p.name, want
                    ),
                ),
            }
        }
    }

    fn constructor(&mut self, name: &str, args: &[Arg], line: usize, f: &SourceFile) {
        let fields = self.env.record_fields(name).or_else(|| self.env.variant_fields(name));
        let Some(fields) = fields else { return };
        if self.env.fns.contains_key(name) {
            return;
        }
        let given: Vec<String> = args.iter().filter_map(|a| a.name.clone()).collect();
        if given.len() != args.len() {
            self.err(&f.path, line, format!("{}(...) bierze pola po nazwie: {}", name, fields.join(", ")));
            return;
        }
        let missing: Vec<&String> = fields.iter().filter(|x| !given.contains(x)).collect();
        let extra: Vec<&String> = given.iter().filter(|x| !fields.contains(x)).collect();
        if !missing.is_empty() {
            self.err(
                &f.path,
                line,
                format!("{}: brak pól {}", name, missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
            );
        }
        if !extra.is_empty() {
            self.err(
                &f.path,
                line,
                format!("{} nie ma pól {}", name, extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
            );
        }
    }

    fn record(&self, n: &str) -> Option<&'a Vec<Field>> {
        match self.env.types.get(n) {
            Some((TypeDecl { body: TypeBody::Record(fs), .. }, _)) => Some(fs),
            _ => None,
        }
    }

    // Podstawa typu: Int, Bool, String, List albo nazwa rekordu. Zawężenie i alias (`type Channel =
    // Int(...)`) prowadzą do podstawy, a unia, wariant i reszta typów nie mają podstawy (None).
    fn base(&self, t: &TypeExpr, depth: usize) -> Option<String> {
        let TypeExpr::Name { name, .. } = t else { return None };
        if PRIMS.contains(&name.as_str()) || name == "List" || self.record(name).is_some() {
            return Some(name.clone());
        }
        match self.env.types.get(name) {
            Some((TypeDecl { body: TypeBody::Rhs(ts), .. }, _)) if ts.len() == 1 && depth < 20 => self.base(&ts[0], depth + 1),
            _ => None,
        }
    }

    // Podstawa typu wyrażenia (Int, String, List, nazwa rekordu...), gdy typ jest znany.
    fn kind(&self, e: &Expr, sc: &mut Scopes) -> Option<String> {
        self.ty(e, sc).and_then(|t| self.base(&t, 0))
    }

    // Pola rekordu r w `r with pole: wartość` i w `R(pole: wartość)`: pole musi istnieć, a wartość
    // o znanej podstawie typu musi pasować do pola. Int pasuje też do Money.
    fn field_types(&mut self, r: &str, given: &[(&str, &Expr)], line: usize, sc: &mut Scopes, f: &SourceFile) {
        let Some(fs) = self.record(r) else { return };
        for (n, v) in given {
            let Some(fd) = fs.iter().find(|x| x.name == *n) else {
                self.err(&f.path, line, format!("{} nie ma pola {}", r, n));
                continue;
            };
            let (Some(want), Some(have)) = (self.base(&fd.ty, 0), self.kind(v, sc)) else { continue };
            if !fits(&want, &have) {
                self.err(&f.path, line, format!("pole {}.{} ma typ {}, a dostaje {}", r, n, fd.ty, have));
            }
        }
    }

    // `try f(...)`: błędy f muszą mieścić się w wyniku funkcji, w której stoi try.
    fn try_errors(&mut self, x: &Expr, line: usize, f: &SourceFile, d: &FnDecl) {
        let ExprKind::Call { name, .. } = &x.kind else { return };
        let Some(info) = self.env.fns.get(name) else { return };
        let Some(r) = &info.decl().ret else { return };
        let alts = r.alts();
        if alts.len() < 2 {
            return;
        }
        let mut errs = vec![];
        for a in &alts[1..] {
            errs.extend(self.env.leaves(a));
        }
        // try w przykładzie albo w lambdzie w transakcji: sprawdza je runtime.
        let Some(own) = &d.ret else {
            self.err(
                &f.path,
                line,
                format!("try {}: funkcja {} nie ma w wyniku miejsca na błędy {}", name, d.name, errs.join(", ")),
            );
            return;
        };
        let mine: BTreeMap<String, ()> = self.env.leaves(own).into_iter().map(|l| (l, ())).collect();
        let missing: Vec<&String> = errs.iter().filter(|e| !mine.contains_key(*e)).collect();
        if !missing.is_empty() {
            let m: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            self.err(&f.path, line, format!("try {}: wynik {} nie obejmuje błędów {}", name, d.name, m.join(", ")));
        }
    }
}

fn named(n: &str) -> TypeExpr {
    TypeExpr::Name {
        name: n.to_string(),
        args: vec![],
        cond: None,
        fields: None,
        line: 0,
    }
}

// Lista z elementem typu el; List bez argumentu, gdy element nieznany.
fn list_of(el: Option<TypeExpr>) -> TypeExpr {
    TypeExpr::Name {
        name: "List".into(),
        args: el.into_iter().collect(),
        cond: None,
        fields: None,
        line: 0,
    }
}

// Wartość o podstawie typu have pasuje tam, gdzie want. Int pasuje też do Money.
fn fits(want: &str, have: &str) -> bool {
    want == have || (want == "Money" && have == "Int")
}

fn is_num(k: &str) -> bool {
    k == "Int" || k == "Money"
}

fn brace_refs(text: &str) -> Vec<String> {
    let mut out = vec![];
    let cs: Vec<char> = text.chars().collect();
    let mut in_tick = false;
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '`' {
            in_tick = !in_tick;
        } else if cs[i] == '{' && !in_tick {
            let mut k = i + 1;
            let mut n = String::new();
            while k < cs.len() && (cs[k].is_alphanumeric() || cs[k] == '_') {
                n.push(cs[k]);
                k += 1;
            }
            if k < cs.len() && cs[k] == '}' && !n.is_empty() {
                out.push(n);
            }
        }
        i += 1;
    }
    out
}

fn stmt_calls(s: &Stmt, out: &mut Vec<(String, usize)>) {
    match s {
        Stmt::Set { expr, .. } | Stmt::Expr { expr, .. } => expr_calls(expr, out),
        Stmt::Return { expr, .. } => {
            if let Some(e) = expr {
                expr_calls(e, out)
            }
        }
        Stmt::If { cond, then, els, .. } => {
            expr_calls(cond, out);
            for s in then.iter().chain(els.iter().flatten()) {
                stmt_calls(s, out);
            }
        }
        Stmt::While { cond: iter, body, .. } | Stmt::For { iter, body, .. } => {
            expr_calls(iter, out);
            for s in body {
                stmt_calls(s, out);
            }
        }
        Stmt::Match { subjects, arms, .. } => {
            for e in subjects {
                expr_calls(e, out);
            }
            for a in arms {
                for s in &a.body {
                    stmt_calls(s, out);
                }
            }
        }
    }
}

fn expr_calls(e: &Expr, out: &mut Vec<(String, usize)>) {
    match &e.kind {
        ExprKind::Call { name, args } => {
            out.push((name.clone(), e.line));
            for a in args {
                expr_calls(&a.value, out);
            }
        }
        ExprKind::Method { obj, args, .. } => {
            expr_calls(obj, out);
            for a in args {
                expr_calls(&a.value, out);
            }
        }
        ExprKind::Html(segs) => {
            for s in segs {
                if let HtmlSeg::Expr(x) = s {
                    expr_calls(x, out);
                }
            }
        }
        ExprKind::List(xs) => xs.iter().for_each(|x| expr_calls(x, out)),
        ExprKind::Field { obj, .. } | ExprKind::Not(obj) | ExprKind::Neg(obj) | ExprKind::Try(obj) => expr_calls(obj, out),
        ExprKind::Bin { l, r, .. } => {
            expr_calls(l, out);
            expr_calls(r, out);
        }
        ExprKind::Is { e, .. } => expr_calls(e, out),
        ExprKind::As { e, alt, .. } => {
            expr_calls(e, out);
            match alt.as_deref() {
                Some(Alt::Value(v)) | Some(Alt::Return(Some(v))) => expr_calls(v, out),
                Some(Alt::Block(b)) => b.iter().for_each(|s| stmt_calls(s, out)),
                _ => {}
            }
        }
        ExprKind::With { e, fields } => {
            expr_calls(e, out);
            fields.iter().for_each(|(_, v)| expr_calls(v, out));
        }
        ExprKind::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => expr_calls(x, out),
            LambdaBody::Block(b) => b.iter().for_each(|s| stmt_calls(s, out)),
        },
        _ => {}
    }
}

// Wzorce z CODEOWNERS i .gitattributes: `*` w obrębie segmentu, `**` przez katalogi,
// wzorzec bez `/` pasuje do nazwy pliku w dowolnym katalogu.
pub fn glob_match(pat: &str, path: &str) -> bool {
    let p = pat.trim_start_matches('/');
    if !p.contains('/') {
        return path.rsplit('/').next().map(|n| glob(p, n)).unwrap_or(false) || glob(p, path);
    }
    glob(p, path) || (p.ends_with('/') && path.starts_with(p))
}

fn glob(p: &str, s: &str) -> bool {
    let p: Vec<char> = p.chars().collect();
    let s: Vec<char> = s.chars().collect();
    fn go(p: &[char], s: &[char]) -> bool {
        match p.first() {
            None => s.is_empty(),
            Some('*') if p.get(1) == Some(&'*') => (0..=s.len()).any(|i| go(&p[2..], &s[i..])),
            Some('*') => (0..=s.len()).take_while(|&i| i == 0 || s[i - 1] != '/').any(|i| go(&p[1..], &s[i..])),
            Some(c) => !s.is_empty() && s[0] == *c && go(&p[1..], &s[1..]),
        }
    }
    go(&p, &s)
}
