// Generator Rusta dla `sowa test --rust`: te same testy co codegen.rs, ale jako program w Ruście,
// który rustc kompiluje razem z runtime.rs do jednego pliku wykonywalnego.
//
// Nazwy: zmienne v_nazwa, funkcje f_nazwa, typy i stałe bez zmiennych z otoczenia to funkcje c_N
// z wartością w thread_local. Każda funkcja Sowy to fn(V, ...) -> R, a błąd idzie przez `?`.
// `or` i `try` są rozwinięte w miejscu: return w bloku `or` wychodzi z najbliższej funkcji
// albo lambdy, tak jak $Ret w JS.

use crate::ast::*;
use crate::env::*;
use crate::project::Project;
use crate::toml;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq)]
enum Ret {
    // return w funkcji: sprawdź typ wyniku.
    Conform,
    // return w lambdzie.
    Plain,
    // return w teście kończy test, a `try` i `or return` to niezaliczony test.
    Test,
    // return w bloku `or` w teście.
    TestThrow,
}

struct Cx {
    scopes: Vec<HashSet<String>>,
    // Nazwy z otoczenia użyte w lambdzie albo warunku typu: trzeba je sklonować do domknięcia.
    caps: Vec<HashSet<String>>,
    ret: Ret,
    tmp: usize,
    file: String,
}

impl Cx {
    fn new(file: &str, ret: Ret) -> Cx {
        Cx {
            scopes: vec![HashSet::new()],
            caps: vec![],
            ret,
            tmp: 0,
            file: file.to_string(),
        }
    }
    fn has(&self, n: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(n))
    }
    fn declare(&mut self, n: &str) {
        self.scopes.last_mut().unwrap().insert(n.to_string());
    }
    fn tmp(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }
    // Nazwy z ramki, które po zamknięciu zakresów domknięcia nadal są w otoczeniu.
    fn pop_caps(&mut self) -> Vec<String> {
        let f = self.caps.pop().unwrap_or_default();
        let mut out: Vec<String> = f.into_iter().filter(|n| self.has(n)).collect();
        out.sort();
        out
    }
}

pub struct Gen<'a> {
    env: &'a Env<'a>,
    consts: String,
    const_ids: HashMap<String, usize>,
}

fn q(s: &str) -> String {
    format!("{:?}", s)
}

fn ident(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            let _ = write!(out, "_u{:x}", c as u32);
        }
    }
    out
}

fn v(name: &str) -> String {
    format!("v_{}", ident(name))
}

fn clones(names: &[String]) -> String {
    names.iter().map(|n| format!("let {} = {}.clone(); ", v(n), v(n))).collect()
}

impl<'a> Gen<'a> {
    pub fn new(env: &'a Env<'a>) -> Gen<'a> {
        Gen {
            env,
            consts: String::new(),
            const_ids: HashMap::new(),
        }
    }

    // Stała liczona raz na wątek.
    fn konst(&mut self, ty: &str, expr: String) -> String {
        let key = format!("{}|{}", ty, expr);
        if let Some(id) = self.const_ids.get(&key) {
            return format!("c_{}()", id);
        }
        let id = self.const_ids.len() + 1;
        self.const_ids.insert(key, id);
        writeln!(
            self.consts,
            "fn c_{id}() -> {ty} {{ thread_local! {{ static C: {ty} = {expr}; }} C.with(|c| c.clone()) }}"
        )
        .unwrap();
        format!("c_{}()", id)
    }

    // ---------- typy ----------

    fn ty(&mut self, t: &TypeExpr, cx: &mut Cx) -> String {
        cx.caps.push(HashSet::new());
        let raw = self.ty_raw(t, cx);
        let caps = cx.pop_caps();
        if caps.is_empty() { self.konst("T", raw) } else { raw }
    }

    fn ty_raw(&mut self, t: &TypeExpr, cx: &mut Cx) -> String {
        match t {
            TypeExpr::Union(xs) => {
                let a: Vec<String> = xs.iter().map(|x| self.ty_raw(x, cx)).collect();
                format!("tunion(vec![{}])", a.join(", "))
            }
            TypeExpr::Name { name, args, cond, .. } => {
                let base = if PRIMS.contains(&name.as_str()) {
                    format!("tp(P::{})", name)
                } else if name == "List" {
                    let e = match args.first() {
                        Some(a) => self.ty_raw(a, cx),
                        None => "tany()".into(),
                    };
                    format!("tlist({})", e)
                } else if CAPS.contains(&name.as_str()) {
                    format!("tcap({})", q(name))
                } else if self.env.is_type(name) {
                    format!("tref({})", q(name))
                } else if self.env.variants.contains_key(name) {
                    format!("tvar({})", q(name))
                } else {
                    "tany()".into()
                };
                match cond {
                    None => base,
                    Some(c) => {
                        let src = format!("{}", t);
                        let h = hints(c);
                        cx.caps.push(HashSet::new());
                        cx.scopes.push(HashSet::new());
                        cx.declare(&c.param);
                        let body = self.expr(&c.expr, cx);
                        cx.scopes.pop();
                        let caps = cx.pop_caps();
                        format!(
                            "trefine({}, {{ {}Rc::new(move |{}: V| -> R {{ {}Ok({}) }}) }}, {}, {})",
                            base,
                            clones(&caps),
                            v(&c.param),
                            clones(&caps),
                            body,
                            q(&src),
                            h
                        )
                    }
                }
            }
        }
    }

    // ---------- program ----------

    fn types_rs(&mut self, out: &mut String) {
        let env = self.env;
        out.push_str("fn init_types() {\n    init_builtins();\n");
        for (name, (t, f)) in &env.types {
            let mut cx = Cx::new(&f.path, Ret::Plain);
            match &t.body {
                TypeBody::Record(fields) => {
                    let fs: Vec<String> = fields
                        .iter()
                        .map(|fl| format!("({}, {})", q(&fl.name), self.ty_raw(&fl.ty, &mut cx)))
                        .collect();
                    writeln!(out, "    defrec({}, vec![{}]);", q(name), fs.join(", ")).unwrap();
                }
                TypeBody::Rhs(terms) => {
                    let mut alts = vec![];
                    for term in terms {
                        let TypeExpr::Name { name: tn, fields, .. } = term else { continue };
                        if env.is_type(tn) {
                            alts.push(self.ty_raw(term, &mut cx));
                        } else {
                            match fields {
                                Some(fs) => {
                                    let fs: Vec<String> =
                                        fs.iter().map(|fl| format!("({}, {})", q(&fl.name), self.ty_raw(&fl.ty, &mut cx))).collect();
                                    writeln!(out, "    defvar({}, Some(vec![{}]));", q(tn), fs.join(", ")).unwrap();
                                }
                                None => writeln!(out, "    defvar({}, None);", q(tn)).unwrap(),
                            }
                            alts.push(format!("tvar({})", q(tn)));
                        }
                    }
                    let d = if alts.len() == 1 {
                        alts.pop().unwrap()
                    } else {
                        format!("tunion(vec![{}])", alts.join(", "))
                    };
                    writeln!(out, "    defnamed({}, {});", q(name), d).unwrap();
                }
            }
        }
        out.push_str("    init_variants();\n}\n\n");
    }

    fn fns_rs(&mut self, out: &mut String) {
        let env = self.env;
        for (name, info) in &env.fns {
            let Some(body) = info.body() else { continue };
            let (d, f) = info.imp.unwrap();
            let mut cx = Cx::new(&f.path, Ret::Conform);
            let params: Vec<String> = d.params.iter().map(|p| format!("mut {}: V", v(&p.name))).collect();
            writeln!(out, "fn f_{}({}) -> R {{", ident(name), params.join(", ")).unwrap();
            for p in &d.params {
                cx.declare(&p.name);
                let t = self.ty(&p.ty, &mut cx);
                writeln!(
                    out,
                    "    {} = conform(&{}, {}, &Wh::S({}))?;",
                    v(&p.name),
                    t,
                    v(&p.name),
                    q(&format!("{}: parametr {}", name, p.name))
                )
                .unwrap();
            }
            let t = match &d.ret {
                Some(r) => self.ty(r, &mut cx),
                None => "tany()".into(),
            };
            writeln!(out, "    let rt: T = {};", t).unwrap();
            writeln!(out, "    let rtn: &str = {};", q(&format!("{}: wynik", name))).unwrap();
            self.block(body, &mut cx, out, 1);
            writeln!(out, "    Ok(V::Unit)\n}}\n").unwrap();
        }
    }

    // Cały program testów bez runtime.
    pub fn tests_program(&mut self, project: &Project, test_res: &[String]) -> String {
        let mut out = String::new();
        self.types_rs(&mut out);
        self.fns_rs(&mut out);
        let env = self.env;
        let mut tests: Vec<String> = vec![];
        let mut n = 0;
        for (name, info) in &env.fns {
            for (d, f) in [info.spec, info.imp].into_iter().flatten() {
                for ex in &d.examples {
                    let src = if ex.multiline {
                        format!("przykład {}", name)
                    } else {
                        ex_src(&ex.stmts)
                    };
                    let mut cx = Cx::new(&f.path, Ret::Test);
                    n += 1;
                    let body = self.test_body(&ex.stmts, &mut cx, test_res);
                    writeln!(out, "fn t_{}(r: &Res, p: &[V]) -> R {{\n{}    Ok(V::Unit)\n}}\n", n, body).unwrap();
                    tests.push(format!(
                        "Test {{ kind: \"example\", fname: {}, file: {}, line: {}, src: {}, gens: None, run: t_{} }}",
                        q(name),
                        q(&f.path),
                        ex.line,
                        q(&src),
                        n
                    ));
                }
                for pr in &d.properties {
                    let mut cx = Cx::new(&f.path, Ret::Test);
                    let mut gens = vec![];
                    let mut lets = String::new();
                    for p in d.params.iter().filter(|p| !crate::codegen::is_cap(&p.ty)) {
                        cx.declare(&p.name);
                        let t = self.ty(&p.ty, &mut cx);
                        let _ = writeln!(lets, "    let mut {} = p[{}].clone();", v(&p.name), gens.len());
                        gens.push(format!("({}, {})", q(&p.name), t));
                    }
                    let stmts = vec![Stmt::Expr {
                        expr: pr.expr.clone(),
                        line: pr.line,
                    }];
                    n += 1;
                    let body = self.test_body(&stmts, &mut cx, test_res);
                    writeln!(out, "fn g_{}() -> Vec<(&'static str, T)> {{ vec![{}] }}", n, gens.join(", ")).unwrap();
                    writeln!(out, "fn t_{}(r: &Res, p: &[V]) -> R {{\n{}{}    Ok(V::Unit)\n}}\n", n, lets, body).unwrap();
                    tests.push(format!(
                        "Test {{ kind: \"property\", fname: {}, file: {}, line: {}, src: {}, gens: Some(g_{}), run: t_{} }}",
                        q(name),
                        q(&f.path),
                        pr.line,
                        q(&pr.expr.to_string()),
                        n,
                        n
                    ));
                }
            }
        }
        for md in &project.docs {
            let path = format!("{}/{}", project.docs_dir, md.rel);
            for (line, code) in &md.blocks {
                // Błędy parsowania zgłasza już generator JS.
                let Ok(stmts) = crate::parser::parse_block_src(code, &path, *line) else { continue };
                let mut cx = Cx::new(&path, Ret::Test);
                n += 1;
                let body = self.test_body(&stmts, &mut cx, test_res);
                writeln!(out, "fn t_{}(r: &Res, p: &[V]) -> R {{\n{}    Ok(V::Unit)\n}}\n", n, body).unwrap();
                tests.push(format!(
                    "Test {{ kind: \"doc\", fname: \"\", file: {}, line: {}, src: \"blok sowa\", gens: None, run: t_{} }}",
                    q(&path),
                    line,
                    n
                ));
            }
        }
        writeln!(out, "fn tests() -> Vec<Test> {{\n    vec![\n        {}\n    ]\n}}\n", tests.join(",\n        ")).unwrap();
        out.push_str(&spec_rs(project));
        out.push_str("\nfn main() {\n    init_types();\n    run_tests(tests(), spec());\n}\n\n// ---- stałe ----\n");
        out.push_str(&self.consts);
        out
    }

    fn test_body(&mut self, stmts: &[Stmt], cx: &mut Cx, test_res: &[String]) -> String {
        let mut out = String::new();
        for r in test_res {
            if !cx.has(r) {
                cx.declare(r);
                writeln!(out, "    let mut {} = res(r, {});", v(r), q(r)).unwrap();
            }
        }
        for s in stmts {
            match s {
                Stmt::Expr { expr, .. } => {
                    let src = q(&expr.to_string());
                    match &expr.kind {
                        ExprKind::Bin { op: "==", l, r } => {
                            let a = self.expr(l, cx);
                            let b = self.expr(r, cx);
                            writeln!(out, "    check_eq({}, {}, {})?;", a, b, src).unwrap();
                        }
                        ExprKind::Is { e, ty, neg } => {
                            let a = self.expr(e, cx);
                            let t = self.ty(ty, cx);
                            writeln!(out, "    check_is({}, &{}, {}, {})?;", a, t, neg, src).unwrap();
                        }
                        _ => {
                            let a = self.expr(expr, cx);
                            writeln!(out, "    check({}, {})?;", a, src).unwrap();
                        }
                    }
                }
                _ => self.stmt(s, cx, &mut out, 1),
            }
        }
        out
    }

    // ---------- instrukcje ----------

    fn block(&mut self, stmts: &[Stmt], cx: &mut Cx, out: &mut String, ind: usize) {
        cx.scopes.push(HashSet::new());
        for s in stmts {
            self.stmt(s, cx, out, ind);
        }
        cx.scopes.pop();
    }

    // Wyjście z funkcji po `try` albo `or return`.
    fn ret_err(&self, cx: &Cx, e: &str) -> String {
        match cx.ret {
            Ret::Conform => format!("return conform(&rt, {}, &Wh::S(rtn));", e),
            Ret::Plain => format!("return Ok({});", e),
            Ret::Test | Ret::TestThrow => format!("return Err(Ctl::Ret({}));", e),
        }
    }

    fn stmt(&mut self, s: &Stmt, cx: &mut Cx, out: &mut String, ind: usize) {
        let pad = "    ".repeat(ind);
        match s {
            Stmt::Set { name, is_var, expr, .. } => {
                let e = self.expr(expr, cx);
                if !*is_var && cx.has(name) {
                    writeln!(out, "{}{} = {};", pad, v(name), e).unwrap();
                } else {
                    writeln!(out, "{}let mut {} = {};", pad, v(name), e).unwrap();
                    cx.declare(name);
                }
            }
            Stmt::Return { expr, .. } => {
                let e = match expr {
                    Some(e) => self.expr(e, cx),
                    None => "V::Unit".into(),
                };
                let r = match cx.ret {
                    Ret::Conform => format!("return conform(&rt, {}, &Wh::S(rtn));", e),
                    Ret::Plain | Ret::Test => format!("return Ok({});", e),
                    Ret::TestThrow => format!("return Err(Ctl::Ret({}));", e),
                };
                writeln!(out, "{}{}", pad, r).unwrap();
            }
            Stmt::Expr { expr, .. } => {
                let e = self.expr(expr, cx);
                writeln!(out, "{}{};", pad, e).unwrap();
            }
            Stmt::If { cond, then, els, .. } => {
                let c = self.expr(cond, cx);
                writeln!(out, "{}if bool_({})? {{", pad, c).unwrap();
                self.block(then, cx, out, ind + 1);
                if let Some(e) = els {
                    writeln!(out, "{}}} else {{", pad).unwrap();
                    self.block(e, cx, out, ind + 1);
                }
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::For { var, iter, body, .. } => {
                let it = self.expr(iter, cx);
                writeln!(out, "{}for {} in iter_({})?.iter() {{", pad, v(var), it).unwrap();
                writeln!(out, "{}    let mut {} = {}.clone();", pad, v(var), v(var)).unwrap();
                cx.scopes.push(HashSet::new());
                cx.declare(var);
                self.block(body, cx, out, ind + 1);
                cx.scopes.pop();
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::Match { subjects, arms, line } => {
                let mut subs = vec![];
                writeln!(out, "{}{{", pad).unwrap();
                for sub in subjects {
                    let t = cx.tmp();
                    let e = self.expr(sub, cx);
                    writeln!(out, "{}    let {} = {};", pad, t, e).unwrap();
                    subs.push(t);
                }
                let mut first = true;
                for arm in arms {
                    let mut conds = vec![];
                    let mut binds = vec![];
                    let all_wild = arm.pats.len() == 1 && matches!(arm.pats[0], Pat::Wild);
                    if !all_wild {
                        for (p, s) in arm.pats.iter().zip(subs.iter()) {
                            self.pat(p, s, cx, &mut conds, &mut binds);
                        }
                    }
                    let c = if conds.is_empty() { "true".to_string() } else { conds.join(" && ") };
                    writeln!(out, "{}    {}if {} {{", pad, if first { "" } else { "else " }, c).unwrap();
                    first = false;
                    cx.scopes.push(HashSet::new());
                    for (n, acc) in &binds {
                        writeln!(out, "{}        let mut {} = {}.clone();", pad, v(n), acc).unwrap();
                        cx.declare(n);
                    }
                    self.block(&arm.body, cx, out, ind + 2);
                    cx.scopes.pop();
                    writeln!(out, "{}    }}", pad).unwrap();
                }
                let vals: Vec<String> = subs.iter().map(|s| format!("{}.clone()", s)).collect();
                writeln!(
                    out,
                    "{}    else {{ return Err(nomatch(vec![{}], {}, {})); }}",
                    pad,
                    vals.join(", "),
                    q(&cx.file),
                    line
                )
                .unwrap();
                writeln!(out, "{}}}", pad).unwrap();
            }
        }
    }

    fn pat(&mut self, p: &Pat, acc: &str, cx: &mut Cx, conds: &mut Vec<String>, binds: &mut Vec<(String, String)>) {
        match p {
            Pat::Wild => {}
            Pat::Str(s) => conds.push(format!("is_str(&{}, {})", acc, q(s))),
            Pat::Bind(n) => binds.push((n.clone(), acc.to_string())),
            Pat::List(ps) => {
                conds.push(format!("list_len(&{}) == Some({})", acc, ps.len()));
                for (i, p) in ps.iter().enumerate() {
                    self.pat(p, &format!("idx(&{}, {})", acc, i), cx, conds, binds);
                }
            }
            Pat::Name { name, bind } => {
                if self.env.variants.contains_key(name) && !self.env.is_type(name) {
                    conds.push(format!("isv(&{}, {})", acc, q(name)));
                } else if self.env.is_type(name) {
                    let t = self.ty(
                        &TypeExpr::Name {
                            name: name.clone(),
                            args: vec![],
                            cond: None,
                            fields: None,
                            line: 0,
                        },
                        cx,
                    );
                    conds.push(format!("is(&{}, &{})", t, acc));
                }
                if let Some(b) = bind {
                    binds.push((b.clone(), acc.to_string()));
                }
            }
        }
    }

    // ---------- wyrażenia ----------

    fn expr(&mut self, e: &Expr, cx: &mut Cx) -> String {
        match &e.kind {
            ExprKind::Int(n) => format!("V::Int({})", n),
            ExprKind::Dec(s) => self.konst("V", format!("dec_lit({})", q(s))),
            ExprKind::Str(s) => self.konst("V", format!("s({})", q(s))),
            ExprKind::Bool(b) => format!("V::Bool({})", b),
            ExprKind::Html(segs) => {
                let h = format!("h{}", cx.tmp());
                let mut out = format!("{{ let mut {} = String::new(); ", h);
                for seg in segs {
                    match seg {
                        HtmlSeg::Lit(t) => {
                            let _ = write!(out, "{}.push_str({}); ", h, q(t));
                        }
                        HtmlSeg::Expr(x) => {
                            let x = self.expr(x, cx);
                            let _ = write!(out, "html_part(&mut {}, &{})?; ", h, x);
                        }
                    }
                }
                let _ = write!(out, "V::Html(Rc::from({})) }}", h);
                out
            }
            ExprKind::Ident(n) => {
                if cx.has(n) {
                    for f in cx.caps.iter_mut() {
                        f.insert(n.clone());
                    }
                    format!("{}.clone()", v(n))
                } else if self.env.variants.contains_key(n) {
                    self.konst("V", format!("vsing({})", q(n)))
                } else {
                    "V::Unit".into()
                }
            }
            ExprKind::List(xs) => {
                let a: Vec<String> = xs.iter().map(|x| self.expr(x, cx)).collect();
                format!("list(vec![{}])", a.join(", "))
            }
            ExprKind::Call { name, args } => self.call(name, args, cx),
            ExprKind::Method { obj, name, targs, args } => {
                let o = self.expr(obj, cx);
                let mut pos = vec![];
                let mut named = vec![];
                for a in args {
                    let x = self.expr(&a.value, cx);
                    match &a.name {
                        Some(n) => named.push(format!("({}, {})", q(n), x)),
                        None => pos.push(x),
                    }
                }
                let ts: Vec<String> = targs.iter().map(|t| self.ty(t, cx)).collect();
                format!(
                    "call({}, {}, vec![{}], vec![{}], vec![{}], {})?",
                    o,
                    q(name),
                    pos.join(", "),
                    named.join(", "),
                    ts.join(", "),
                    e.line
                )
            }
            ExprKind::Field { obj, name } => {
                let o = self.expr(obj, cx);
                format!("field({}, {})?", o, q(name))
            }
            ExprKind::Bin { op, l, r } => {
                let a = self.expr(l, cx);
                let b = self.expr(r, cx);
                match *op {
                    "+" => format!("add({}, {})?", a, b),
                    "-" => format!("sub({}, {})?", a, b),
                    "*" => format!("mul({}, {})?", a, b),
                    "/" => format!("div({}, {})?", a, b),
                    "%" => format!("rem({}, {})?", a, b),
                    "==" => format!("V::Bool(eq(&{}, &{}))", a, b),
                    "!=" => format!("V::Bool(!eq(&{}, &{}))", a, b),
                    "<" | "<=" | ">" | ">=" => format!("V::Bool(cmp(&{}, &{})? {} 0)", a, b, op),
                    "&&" => format!("V::Bool(bool_({})? && bool_({})?)", a, b),
                    "||" => format!("V::Bool(bool_({})? || bool_({})?)", a, b),
                    _ => unreachable!(),
                }
            }
            ExprKind::Not(x) => format!("V::Bool(!bool_({})?)", self.expr(x, cx)),
            ExprKind::Neg(x) => format!("neg({})?", self.expr(x, cx)),
            ExprKind::Is { e: x, ty, neg } => {
                let a = self.expr(x, cx);
                let t = self.ty(ty, cx);
                format!("V::Bool({}is(&{}, &{}))", if *neg { "!" } else { "" }, t, a)
            }
            ExprKind::As { e: x, ty, alt } => {
                let a = self.expr(x, cx);
                let t = self.ty(ty, cx);
                let src = q(&format!("{}", ty));
                match alt.as_deref() {
                    None => format!("as_strict(&{}, {}, {})?", t, a, src),
                    Some(alt) => {
                        let tv = cx.tmp();
                        let body = match alt {
                            Alt::Value(x) => self.expr(x, cx),
                            Alt::Return(x) => {
                                let r = match x {
                                    Some(x) => self.expr(x, cx),
                                    None => "V::Unit".into(),
                                };
                                format!("{{ {} }}", self.ret_err(cx, &r))
                            }
                            Alt::Block(stmts) => {
                                let saved = cx.ret;
                                if cx.ret == Ret::Test {
                                    cx.ret = Ret::TestThrow;
                                }
                                let mut b = String::new();
                                self.block(stmts, cx, &mut b, 3);
                                cx.ret = saved;
                                format!("{{\n{}            return Err(orblock_end());\n        }}", b)
                            }
                        };
                        format!("match as_(&{}, {})? {{ Some({}) => {}, None => {} }}", t, a, tv, tv, body)
                    }
                }
            }
            ExprKind::With { e: x, fields } => {
                let a = self.expr(x, cx);
                let fs: Vec<String> = fields.iter().map(|(n, val)| format!("({}, {})", q(n), self.expr(val, cx))).collect();
                format!("with({}, vec![{}])?", a, fs.join(", "))
            }
            ExprKind::Lambda { param, body } => {
                cx.caps.push(HashSet::new());
                cx.scopes.push(HashSet::new());
                cx.declare(param);
                let saved = cx.ret;
                cx.ret = Ret::Plain;
                let b = match body {
                    LambdaBody::Expr(x) => format!("Ok({})", self.expr(x, cx)),
                    LambdaBody::Block(stmts) => {
                        let mut b = String::from("\n");
                        self.block(stmts, cx, &mut b, 3);
                        b.push_str("            Ok(V::Unit)\n        ");
                        b
                    }
                };
                cx.ret = saved;
                cx.scopes.pop();
                let caps = cx.pop_caps();
                // Klon na wejściu, żeby `var` z otoczenia dało się zmienić w lambdzie (jak kopia).
                let inner: String = caps.iter().map(|n| format!("let mut {} = {}.clone(); ", v(n), v(n))).collect();
                format!(
                    "{{ {}V::Fn(Rc::new(move |mut {}: V| -> R {{ {}{} }})) }}",
                    clones(&caps),
                    v(param),
                    inner,
                    b
                )
            }
            ExprKind::Try(x) => {
                let ExprKind::Call { name, .. } = &x.kind else { return "V::Unit".into() };
                let Some(info) = self.env.fns.get(name) else { return "V::Unit".into() };
                let d = info.decl();
                let alts: Vec<TypeExpr> = d.ret.as_ref().map(|r| r.alts().into_iter().cloned().collect()).unwrap_or_default();
                if alts.len() < 2 {
                    return self.expr(x, cx);
                }
                let rest = TypeExpr::Union(alts[1..].to_vec());
                let t = self.ty(&rest, cx);
                let c = self.expr(x, cx);
                let tv = cx.tmp();
                format!("{{ let {} = {}; if is(&{}, &{}) {{ {} }} {} }}", tv, c, t, tv, self.ret_err(cx, &tv), tv)
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Arg], cx: &mut Cx) -> String {
        let env = self.env;
        if let Some(info) = env.fns.get(name) {
            let d = info.decl();
            let mut slots: Vec<Option<String>> = vec![None; d.params.len()];
            let mut i = 0;
            for a in args {
                let x = self.expr(&a.value, cx);
                match &a.name {
                    Some(n) => {
                        if let Some(k) = d.params.iter().position(|p| &p.name == n) {
                            slots[k] = Some(x);
                        }
                    }
                    None => {
                        if i < slots.len() {
                            slots[i] = Some(x);
                        }
                        i += 1;
                    }
                }
            }
            let a: Vec<String> = slots.into_iter().map(|s| s.unwrap_or_else(|| "V::Unit".into())).collect();
            return format!("f_{}({})?", ident(name), a.join(", "));
        }
        if env.record_fields(name).is_some() || env.variant_fields(name).is_some() {
            let fs: Vec<String> = args
                .iter()
                .map(|a| {
                    let x = self.expr(&a.value, cx);
                    format!("({}, {})", q(&a.name.clone().unwrap_or_default()), x)
                })
                .collect();
            let f = if env.record_fields(name).is_some() { "mk" } else { "mkv" };
            return format!("{}({}, vec![{}])?", f, q(name), fs.join(", "));
        }
        if let Some((arity, _)) = builtin_fn(name) {
            let mut a: Vec<String> = args.iter().map(|a| self.expr(&a.value, cx)).collect();
            a.truncate(arity);
            while a.len() < arity {
                a.push("V::Unit".into());
            }
            return format!("b_{}({})?", name, a.join(", "));
        }
        "V::Unit".into()
    }
}

fn ex_src(stmts: &[Stmt]) -> String {
    match stmts.first() {
        Some(Stmt::Expr { expr, .. }) => expr.to_string(),
        _ => String::new(),
    }
}

fn f64_lit(x: f64) -> String {
    format!("{:?}", x)
}

// Te same podpowiedzi dla generatora co w codegen.rs.
fn hints(c: &Cond) -> String {
    let mut h: Vec<String> = vec![];
    let mut conj = vec![];
    flatten_and(&c.expr, &mut conj);
    let is_p = |e: &Expr| matches!(&e.kind, ExprKind::Ident(n) if *n == c.param);
    let len_of_p = |e: &Expr| matches!(&e.kind, ExprKind::Call { name, args } if name == "len" && args.len() == 1 && is_p(&args[0].value));
    let num = |e: &Expr| -> Option<f64> {
        match &e.kind {
            ExprKind::Int(n) => Some(*n as f64),
            ExprKind::Dec(s) => s.parse().ok(),
            ExprKind::Neg(x) => match &x.kind {
                ExprKind::Int(n) => Some(-(*n as f64)),
                _ => None,
            },
            _ => None,
        }
    };
    for e in conj {
        match &e.kind {
            ExprKind::Bin { op, l, r } if is_p(l) => {
                if let Some(n) = num(r) {
                    match *op {
                        ">" => h.push(format!("min: Some({}), min_ex: true", f64_lit(n))),
                        ">=" => h.push(format!("min: Some({})", f64_lit(n))),
                        "<" => h.push(format!("max: Some({}), max_ex: true", f64_lit(n))),
                        "<=" => h.push(format!("max: Some({})", f64_lit(n))),
                        _ => {}
                    }
                }
            }
            ExprKind::Bin { op, l, r } if len_of_p(l) => {
                if let Some(n) = num(r) {
                    let n = n as i64;
                    match *op {
                        "==" => h.push(format!("len: Some({})", n)),
                        ">" => h.push(format!("minlen: Some({})", n + 1)),
                        ">=" => h.push(format!("minlen: Some({})", n)),
                        "<" => h.push(format!("maxlen: Some({})", n - 1)),
                        "<=" => h.push(format!("maxlen: Some({})", n)),
                        _ => {}
                    }
                }
            }
            ExprKind::Call { name, args } if !args.is_empty() && is_p(&args[0].value) => {
                let s = args
                    .get(1)
                    .and_then(|a| if let ExprKind::Str(s) = &a.value.kind { Some(s.clone()) } else { None });
                match (name.as_str(), s) {
                    ("matches", Some(s)) => h.push(format!("re: Some({})", q(&s))),
                    ("starts_with", Some(s)) => h.push(format!("prefix: Some({})", q(&s))),
                    ("only_digits", _) | ("nip_checksum_ok", _) => h.push("digits: true".into()),
                    ("valid_email", _) => h.push("email: true".into()),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    // Późniejsza podpowiedź tego samego pola wygrywa, jak w obiekcie JS.
    let mut fields: Vec<(String, String)> = vec![];
    for part in h.iter().flat_map(|x| x.split(", ")) {
        let (k, val) = part.split_once(": ").unwrap();
        fields.retain(|(f, _)| f != k);
        fields.push((k.to_string(), val.to_string()));
    }
    let body: Vec<String> = fields.iter().map(|(k, val)| format!("{}: {}", k, val)).collect();
    format!("Hints {{ {}{}..Default::default() }}", body.join(", "), if body.is_empty() { "" } else { ", " })
}

fn flatten_and<'e>(e: &'e Expr, out: &mut Vec<&'e Expr>) {
    match &e.kind {
        ExprKind::Bin { op: "&&", l, r } => {
            flatten_and(l, out);
            flatten_and(r, out);
        }
        _ => out.push(e),
    }
}

// Zasoby z [resources.test] jako Spec dla runtime.
fn spec_rs(project: &Project) -> String {
    let entries = project.toml.get("resources.test").cloned().unwrap_or_default();
    let mut items = vec![];
    for e in &entries {
        let get = |k: &str| match e.value.get(k) {
            Some(toml::Value::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let opt = |x: Option<String>| match x {
            Some(s) => format!("Some({})", q(&s)),
            None => "None".into(),
        };
        let fake = match get("fake") {
            Some(f) => format!("Some(f_{} as fn(V) -> R)", ident(&f)),
            None => "None".into(),
        };
        items.push(format!(
            "({}, Spec {{ ty: {}, url: {}, now: {}, fake: {} }})",
            q(&e.key),
            q(&get("type").unwrap_or_default()),
            opt(get("url")),
            opt(get("now")),
            fake
        ));
    }
    format!("fn spec() -> Vec<(&'static str, Spec)> {{\n    vec![{}]\n}}\n", items.join(", "))
}
