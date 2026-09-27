// Generator JS dla Buna. Każda funkcja użytkownika jest async, każde jej wywołanie ma await.
// Warunki typów to synchroniczne domknięcia (v_α) => ..., więc nie wołają funkcji użytkownika.
//
// Nazwy: zmienne v_nazwa, funkcje f_nazwa, wbudowane $b.nazwa, warianty bez danych $V.Nazwa.

use crate::ast::*;
use crate::env::*;
use crate::project::Project;
use std::collections::HashSet;
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq)]
enum Ret {
    // return w funkcji: sprawdź typ wyniku.
    Conform,
    // return w lambdzie.
    Plain,
    // return w bloku `or` (wyrażenie): rzuć $Ret, złapie go funkcja albo lambda.
    Throw,
}

struct Cx {
    scopes: Vec<HashSet<String>>,
    ret: Ret,
    // W warunku typu nie ma await.
    sync: bool,
    tmp: usize,
    file: String,
}

pub struct Gen<'a> {
    env: &'a Env<'a>,
    pub diags: Vec<Diag>,
}

pub fn js_str(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn v(name: &str) -> String {
    format!("v_{}", name)
}

impl Cx {
    fn new(file: &str, ret: Ret) -> Cx {
        Cx {
            scopes: vec![HashSet::new()],
            ret,
            sync: false,
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
        format!("$t{}", self.tmp)
    }
}

impl<'a> Gen<'a> {
    pub fn new(env: &'a Env<'a>) -> Gen<'a> {
        Gen { env, diags: vec![] }
    }

    fn err(&mut self, cx: &Cx, line: usize, msg: impl Into<String>) {
        self.diags.push(Diag::new(&cx.file, line, msg));
    }

    // ---------- typy ----------

    fn ty(&mut self, t: &TypeExpr, cx: &mut Cx) -> String {
        match t {
            TypeExpr::Union(v) => {
                let a: Vec<String> = v.iter().map(|x| self.ty(x, cx)).collect();
                format!("$union([{}])", a.join(", "))
            }
            TypeExpr::Name { name, args, cond, line, .. } => {
                let base = if PRIMS.contains(&name.as_str()) {
                    format!("$P.{}", name)
                } else if name == "List" {
                    let e = match args.first() {
                        Some(a) => self.ty(a, cx),
                        None => "$P.Any".into(),
                    };
                    format!("$list({})", e)
                } else if CAPS.contains(&name.as_str()) {
                    format!("$cap({})", js_str(name))
                } else if self.env.is_type(name) {
                    format!("$ref({})", js_str(name))
                } else if self.env.variants.contains_key(name) {
                    format!("$var({})", js_str(name))
                } else {
                    self.err(cx, *line, format!("nieznany typ {}", name));
                    "$P.Any".into()
                };
                match cond {
                    None => base,
                    Some(c) => {
                        let src = format!("{}", t);
                        let hints = hints(c);
                        let sync = cx.sync;
                        cx.sync = true;
                        cx.scopes.push(HashSet::new());
                        cx.declare(&c.param);
                        let body = self.expr(&c.expr, cx);
                        cx.scopes.pop();
                        cx.sync = sync;
                        format!("$refine({}, ({}) => {}, {}, {})", base, v(&c.param), body, js_str(&src), hints)
                    }
                }
            }
        }
    }

    // ---------- program ----------

    pub fn types_js(&mut self, out: &mut String) {
        let env = self.env;
        for (name, (t, f)) in &env.types {
            let mut cx = Cx::new(&f.path, Ret::Plain);
            match &t.body {
                TypeBody::Record(fields) => {
                    let fs: Vec<String> = fields
                        .iter()
                        .map(|fl| format!("[{}, {}]", js_str(&fl.name), self.ty(&fl.ty, &mut cx)))
                        .collect();
                    writeln!(out, "$T[{}] = $rec({}, [{}]);", js_str(name), js_str(name), fs.join(", ")).unwrap();
                }
                TypeBody::Rhs(terms) => {
                    let mut alts = vec![];
                    for term in terms {
                        let TypeExpr::Name { name: tn, fields, .. } = term else { continue };
                        if env.is_type(tn) {
                            alts.push(self.ty(term, &mut cx));
                        } else {
                            match fields {
                                Some(fs) => {
                                    let fs: Vec<String> = fs.iter().map(|fl| format!("[{}, {}]", js_str(&fl.name), self.ty(&fl.ty, &mut cx))).collect();
                                    writeln!(out, "$VD[{}] = [{}];", js_str(tn), fs.join(", ")).unwrap();
                                }
                                None => {
                                    writeln!(out, "$VD[{}] = null;", js_str(tn)).unwrap();
                                }
                            }
                            alts.push(format!("$var({})", js_str(tn)));
                        }
                    }
                    let d = if alts.len() == 1 {
                        alts.pop().unwrap()
                    } else {
                        format!("$union([{}])", alts.join(", "))
                    };
                    writeln!(out, "$T[{}] = $named({}, {});", js_str(name), js_str(name), d).unwrap();
                }
            }
        }
        writeln!(out, "$init_variants();").unwrap();
    }

    pub fn fns_js(&mut self, out: &mut String) {
        let env = self.env;
        let mut names = vec![];
        for (name, info) in &env.fns {
            let Some(body) = info.body() else { continue };
            let (d, f) = info.imp.unwrap();
            names.push(name.clone());
            let mut cx = Cx::new(&f.path, Ret::Conform);
            let params: Vec<String> = d.params.iter().map(|p| v(&p.name)).collect();
            writeln!(out, "async function f_{}({}) {{", name, params.join(", ")).unwrap();
            for p in &d.params {
                cx.declare(&p.name);
                let t = self.ty(&p.ty, &mut cx);
                writeln!(
                    out,
                    "  {} = $conform({}, {}, {});",
                    v(&p.name),
                    t,
                    v(&p.name),
                    js_str(&format!("{}: parametr {}", name, p.name))
                )
                .unwrap();
            }
            match &d.ret {
                Some(r) => {
                    let t = self.ty(r, &mut cx);
                    writeln!(out, "  const $rt = {};", t).unwrap();
                }
                None => writeln!(out, "  const $rt = $P.Any;").unwrap(),
            }
            writeln!(out, "  const $rtn = {};", js_str(&format!("{}: wynik", name))).unwrap();
            writeln!(out, "  try {{").unwrap();
            let mut b = String::new();
            self.block(body, &mut cx, &mut b, 2);
            out.push_str(&b);
            writeln!(out, "  }} catch ($e) {{ if ($e instanceof $Ret) return $conform($rt, $e.v, $rtn); throw $e; }}").unwrap();
            writeln!(out, "}}").unwrap();
        }
        let fs: Vec<String> = names.iter().map(|n| format!("{}: f_{}", n, n)).collect();
        writeln!(out, "const $FNS = {{{}}};", fs.join(", ")).unwrap();
    }

    // Kod programu bez punktu wejścia.
    pub fn program(&mut self) -> String {
        let mut out = String::new();
        out.push_str(include_str!("runtime.js"));
        out.push_str("\n// ---- program ----\n");
        self.types_js(&mut out);
        self.fns_js(&mut out);
        out
    }

    // Testy: przykłady, property i bloki sowa z dokumentacji.
    pub fn tests(&mut self, project: &Project, test_res: &[String]) -> String {
        let env = self.env;
        let mut out = String::from("const $TESTS = [];\n");
        for (name, info) in &env.fns {
            for (d, f) in [info.spec, info.imp].into_iter().flatten() {
                for ex in &d.examples {
                    let src = if ex.multiline {
                        format!("przykład {}", name)
                    } else {
                        format!("{}", ex_src(&ex.stmts))
                    };
                    let mut cx = Cx::new(&f.path, Ret::Plain);
                    let body = self.test_body(&ex.stmts, &mut cx, test_res);
                    writeln!(
                        out,
                        "$TESTS.push({{kind: \"example\", fn: {}, file: {}, line: {}, src: {}, run: async ($R) => {{\n{}}}}});",
                        js_str(name),
                        js_str(&f.path),
                        ex.line,
                        js_str(&src),
                        body
                    )
                    .unwrap();
                }
                for pr in &d.properties {
                    let mut cx = Cx::new(&f.path, Ret::Plain);
                    let mut gens = vec![];
                    for p in &d.params {
                        if is_cap(&p.ty) {
                            continue;
                        }
                        cx.declare(&p.name);
                        gens.push(format!("[{}, {}]", js_str(&p.name), self.ty(&p.ty, &mut cx)));
                    }
                    let stmts = vec![Stmt::Expr {
                        expr: pr.expr.clone(),
                        line: pr.line,
                    }];
                    let body = self.test_body(&stmts, &mut cx, test_res);
                    writeln!(
                        out,
                        "$TESTS.push({{kind: \"property\", fn: {}, file: {}, line: {}, src: {}, gens: () => [{}], run: async ($R, $p) => {{\n{}{}}}}});",
                        js_str(name),
                        js_str(&f.path),
                        pr.line,
                        js_str(&pr.expr.to_string()),
                        gens.join(", "),
                        d.params
                            .iter()
                            .filter(|p| !is_cap(&p.ty))
                            .map(|p| format!("  const {} = $p[{}];\n", v(&p.name), js_str(&p.name)))
                            .collect::<String>(),
                        body
                    )
                    .unwrap();
                }
            }
        }
        for md in &project.docs {
            let path = format!("{}/{}", project.docs_dir, md.rel);
            for (line, code) in &md.blocks {
                match crate::parser::parse_block_src(code, &path, *line) {
                    Ok(stmts) => {
                        let mut cx = Cx::new(&path, Ret::Plain);
                        let body = self.test_body(&stmts, &mut cx, test_res);
                        writeln!(
                            out,
                            "$TESTS.push({{kind: \"doc\", fn: null, file: {}, line: {}, src: {}, run: async ($R) => {{\n{}}}}});",
                            js_str(&path),
                            line,
                            js_str("blok sowa"),
                            body
                        )
                        .unwrap();
                    }
                    Err(d) => self.diags.push(d),
                }
            }
        }
        out
    }

    fn test_body(&mut self, stmts: &[Stmt], cx: &mut Cx, test_res: &[String]) -> String {
        let mut out = String::new();
        for r in test_res {
            if !cx.has(r) {
                cx.declare(r);
                writeln!(out, "  const {} = $R[{}];", v(r), js_str(r)).unwrap();
            }
        }
        for s in stmts {
            match s {
                Stmt::Expr { expr, line } => {
                    let src = js_str(&expr.to_string());
                    match &expr.kind {
                        ExprKind::Bin { op: "==", l, r } => {
                            let a = self.expr(l, cx);
                            let b = self.expr(r, cx);
                            writeln!(out, "  $check_eq({}, {}, {}, {});", a, b, src, line).unwrap();
                        }
                        ExprKind::Is { e, ty, neg } => {
                            let a = self.expr(e, cx);
                            let t = self.ty(ty, cx);
                            writeln!(out, "  $check_is({}, {}, {}, {}, {});", a, t, neg, src, line).unwrap();
                        }
                        _ => {
                            let a = self.expr(expr, cx);
                            writeln!(out, "  $check({}, {}, {});", a, src, line).unwrap();
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

    fn stmt(&mut self, s: &Stmt, cx: &mut Cx, out: &mut String, ind: usize) {
        let pad = "  ".repeat(ind);
        match s {
            Stmt::Set { name, is_var, expr, .. } => {
                let e = self.expr(expr, cx);
                if !*is_var && cx.has(name) {
                    writeln!(out, "{}{} = {};", pad, v(name), e).unwrap();
                } else {
                    let kw = if *is_var { "let" } else { "const" };
                    writeln!(out, "{}{} {} = {};", pad, kw, v(name), e).unwrap();
                    cx.declare(name);
                }
            }
            Stmt::Return { expr, .. } => {
                let e = match expr {
                    Some(e) => self.expr(e, cx),
                    None => "undefined".into(),
                };
                match cx.ret {
                    Ret::Conform => writeln!(out, "{}return $conform($rt, {}, $rtn);", pad, e).unwrap(),
                    Ret::Plain => writeln!(out, "{}return {};", pad, e).unwrap(),
                    Ret::Throw => writeln!(out, "{}throw new $Ret({});", pad, e).unwrap(),
                }
            }
            Stmt::Expr { expr, .. } => {
                let e = self.expr(expr, cx);
                writeln!(out, "{}{};", pad, e).unwrap();
            }
            Stmt::If { cond, then, els, .. } => {
                let c = self.expr(cond, cx);
                writeln!(out, "{}if ($bool({})) {{", pad, c).unwrap();
                self.block(then, cx, out, ind + 1);
                if let Some(e) = els {
                    writeln!(out, "{}}} else {{", pad).unwrap();
                    self.block(e, cx, out, ind + 1);
                }
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::For { var, iter, body, .. } => {
                let it = self.expr(iter, cx);
                writeln!(out, "{}for (const {} of $iter({})) {{", pad, v(var), it).unwrap();
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
                    writeln!(out, "{}  const {} = {};", pad, t, e).unwrap();
                    subs.push(t);
                }
                let mut first = true;
                for arm in arms {
                    let mut conds = vec![];
                    let mut binds = vec![];
                    let all_wild = arm.pats.len() == 1 && matches!(arm.pats[0], Pat::Wild);
                    if !all_wild {
                        if arm.pats.len() != subs.len() {
                            self.err(cx, arm.line, format!("gałąź ma {} wzorców, a match {} wartości", arm.pats.len(), subs.len()));
                        }
                        for (p, s) in arm.pats.iter().zip(subs.iter()) {
                            self.pat(p, s, cx, &mut conds, &mut binds);
                        }
                    }
                    let c = if conds.is_empty() { "true".to_string() } else { conds.join(" && ") };
                    writeln!(out, "{}  {}if ({}) {{", pad, if first { "" } else { "else " }, c).unwrap();
                    first = false;
                    cx.scopes.push(HashSet::new());
                    for (n, acc) in &binds {
                        writeln!(out, "{}    const {} = {};", pad, v(n), acc).unwrap();
                        cx.declare(n);
                    }
                    self.block(&arm.body, cx, out, ind + 2);
                    cx.scopes.pop();
                    writeln!(out, "{}  }}", pad).unwrap();
                }
                writeln!(out, "{}  else $nomatch([{}], {}, {});", pad, subs.join(", "), js_str(&cx.file), line).unwrap();
                writeln!(out, "{}}}", pad).unwrap();
            }
        }
    }

    fn pat(&mut self, p: &Pat, acc: &str, cx: &mut Cx, conds: &mut Vec<String>, binds: &mut Vec<(String, String)>) {
        match p {
            Pat::Wild => {}
            Pat::Str(s) => conds.push(format!("{} === {}", acc, js_str(s))),
            Pat::Bind(n) => binds.push((n.clone(), acc.to_string())),
            Pat::List(ps) => {
                conds.push(format!("Array.isArray({}) && {}.length === {}", acc, acc, ps.len()));
                for (i, p) in ps.iter().enumerate() {
                    self.pat(p, &format!("{}[{}]", acc, i), cx, conds, binds);
                }
            }
            Pat::Name { name, bind } => {
                if self.env.variants.contains_key(name) && !self.env.is_type(name) {
                    conds.push(format!("$isv({}, {})", acc, js_str(name)));
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
                    conds.push(format!("$is({}, {})", t, acc));
                } else {
                    self.err(cx, 0, format!("nieznany wariant albo typ {} we wzorcu", name));
                }
                if let Some(b) = bind {
                    binds.push((b.clone(), acc.to_string()));
                }
            }
        }
    }

    // ---------- wyrażenia ----------

    fn aw(&self, cx: &Cx, e: String) -> String {
        if cx.sync { e } else { format!("(await {})", e) }
    }

    fn expr(&mut self, e: &Expr, cx: &mut Cx) -> String {
        match &e.kind {
            ExprKind::Int(n) => format!("{}", n),
            ExprKind::Dec(s) => format!("$dec({})", js_str(s)),
            ExprKind::Str(s) => js_str(s),
            ExprKind::Bool(b) => format!("{}", b),
            ExprKind::Html(segs) => {
                let parts: Vec<String> = segs
                    .iter()
                    .map(|s| match s {
                        HtmlSeg::Lit(t) => format!("$raw({})", js_str(t)),
                        HtmlSeg::Expr(x) => self.expr(x, cx),
                    })
                    .collect();
                format!("$html([{}])", parts.join(", "))
            }
            ExprKind::Ident(n) => {
                if cx.has(n) {
                    v(n)
                } else if self.env.variants.contains_key(n) {
                    format!("$V[{}]", js_str(n))
                } else {
                    self.err(cx, e.line, format!("nieznana nazwa {}", n));
                    "undefined".into()
                }
            }
            ExprKind::List(xs) => {
                let a: Vec<String> = xs.iter().map(|x| self.expr(x, cx)).collect();
                format!("[{}]", a.join(", "))
            }
            ExprKind::Call { name, args } => self.call(name, args, e.line, cx),
            ExprKind::Method { obj, name, targs, args } => {
                if cx.sync {
                    self.err(cx, e.line, "w warunku typu nie ma wywołań metod");
                }
                let o = self.expr(obj, cx);
                let mut pos = vec![];
                let mut named = vec![];
                for a in args {
                    let x = self.expr(&a.value, cx);
                    match &a.name {
                        Some(n) => named.push(format!("{}: {}", n, x)),
                        None => pos.push(x),
                    }
                }
                let ts: Vec<String> = targs.iter().map(|t| self.ty(t, cx)).collect();
                format!(
                    "(await $call({}, {}, [{}], {{{}}}, [{}], {}))",
                    o,
                    js_str(name),
                    pos.join(", "),
                    named.join(", "),
                    ts.join(", "),
                    e.line
                )
            }
            ExprKind::Field { obj, name } => {
                let o = self.expr(obj, cx);
                format!("$f({}, {})", o, js_str(name))
            }
            ExprKind::Bin { op, l, r } => {
                let a = self.expr(l, cx);
                let b = self.expr(r, cx);
                match *op {
                    "+" => format!("$add({}, {})", a, b),
                    "-" => format!("$sub({}, {})", a, b),
                    "*" => format!("$mul({}, {})", a, b),
                    "/" => format!("$div({}, {})", a, b),
                    "%" => format!("$mod({}, {})", a, b),
                    "==" => format!("$eq({}, {})", a, b),
                    "!=" => format!("!$eq({}, {})", a, b),
                    "<" | "<=" | ">" | ">=" => format!("($cmp({}, {}) {} 0)", a, b, op),
                    "&&" => format!("($bool({}) && $bool({}))", a, b),
                    "||" => format!("($bool({}) || $bool({}))", a, b),
                    _ => unreachable!(),
                }
            }
            ExprKind::Not(x) => format!("!$bool({})", self.expr(x, cx)),
            ExprKind::Neg(x) => format!("$neg({})", self.expr(x, cx)),
            ExprKind::Is { e: x, ty, neg } => {
                let a = self.expr(x, cx);
                let t = self.ty(ty, cx);
                format!("{}$is({}, {})", if *neg { "!" } else { "" }, t, a)
            }
            ExprKind::As { e: x, ty, alt } => {
                let a = self.expr(x, cx);
                let t = self.ty(ty, cx);
                let src = js_str(&format!("{}", ty));
                match alt.as_deref() {
                    None => format!("$as_strict({}, {}, {})", t, a, src),
                    Some(alt) => {
                        if cx.sync {
                            self.err(cx, e.line, "w warunku typu nie ma `or`");
                        }
                        let tv = cx.tmp();
                        let body = match alt {
                            Alt::Value(x) => format!("return {};", self.expr(x, cx)),
                            Alt::Return(x) => {
                                let r = match x {
                                    Some(x) => self.expr(x, cx),
                                    None => "undefined".into(),
                                };
                                format!("throw new $Ret({});", r)
                            }
                            Alt::Block(stmts) => {
                                let saved = cx.ret;
                                cx.ret = Ret::Throw;
                                let mut b = String::new();
                                self.block(stmts, cx, &mut b, 3);
                                cx.ret = saved;
                                format!("\n{}      $orblock_end();\n    ", b)
                            }
                        };
                        format!(
                            "(await (async ({}) => {{ if ({} !== $FAIL) return {}; {} }})($as({}, {})))",
                            tv, tv, tv, body, t, a
                        )
                    }
                }
            }
            ExprKind::With { e: x, fields } => {
                let a = self.expr(x, cx);
                let fs: Vec<String> = fields.iter().map(|(n, val)| format!("{}: {}", n, self.expr(val, cx))).collect();
                format!("$with({}, {{{}}})", a, fs.join(", "))
            }
            ExprKind::Lambda { param, body } => {
                if cx.sync {
                    self.err(cx, e.line, "w warunku typu nie ma lambd");
                }
                cx.scopes.push(HashSet::new());
                cx.declare(param);
                let saved = cx.ret;
                cx.ret = Ret::Plain;
                let out = match body {
                    LambdaBody::Expr(x) => {
                        let b = self.expr(x, cx);
                        format!(
                            "(async ({}) => {{ try {{ return {}; }} catch ($e) {{ if ($e instanceof $Ret) return $e.v; throw $e; }} }})",
                            v(param),
                            b
                        )
                    }
                    LambdaBody::Block(stmts) => {
                        let mut b = String::new();
                        self.block(stmts, cx, &mut b, 3);
                        format!(
                            "(async ({}) => {{ try {{\n{}  }} catch ($e) {{ if ($e instanceof $Ret) return $e.v; throw $e; }} }})",
                            v(param),
                            b
                        )
                    }
                };
                cx.ret = saved;
                cx.scopes.pop();
                out
            }
            ExprKind::Try(x) => {
                if cx.sync {
                    self.err(cx, e.line, "w warunku typu nie ma `try`");
                    return "undefined".into();
                }
                let ExprKind::Call { name, .. } = &x.kind else {
                    self.err(cx, e.line, "`try` stoi tylko przed wywołaniem funkcji");
                    return "undefined".into();
                };
                let Some(info) = self.env.fns.get(name) else {
                    self.err(
                        cx,
                        e.line,
                        format!("`try` stoi tylko przed wywołaniem funkcji z programu, a {} nią nie jest", name),
                    );
                    return "undefined".into();
                };
                let d = info.decl();
                let alts: Vec<TypeExpr> = d.ret.as_ref().map(|r| r.alts().into_iter().cloned().collect()).unwrap_or_default();
                if alts.len() < 2 {
                    self.err(cx, e.line, format!("`try`: wynik {} nie ma wariantów błędu", name));
                    return self.expr(x, cx);
                }
                // Warunki w typie błędu nie mogą odwoływać się do parametrów wołanej funkcji.
                let rest = TypeExpr::Union(alts[1..].to_vec());
                let t = self.ty(&rest, cx);
                let c = self.expr(x, cx);
                format!("$try({}, {})", c, t)
            }
        }
    }

    fn call(&mut self, name: &str, args: &[Arg], line: usize, cx: &mut Cx) -> String {
        let env = self.env;
        if let Some(info) = env.fns.get(name) {
            if cx.sync {
                self.err(cx, line, format!("warunek typu nie może wołać funkcji {}: warunki są czyste i proste", name));
                return "undefined".into();
            }
            let d = info.decl();
            let mut slots: Vec<Option<String>> = vec![None; d.params.len()];
            let mut i = 0;
            for a in args {
                let x = self.expr(&a.value, cx);
                match &a.name {
                    Some(n) => match d.params.iter().position(|p| &p.name == n) {
                        Some(k) => slots[k] = Some(x),
                        None => self.err(cx, line, format!("{} nie ma parametru {}", name, n)),
                    },
                    None => {
                        if i < slots.len() {
                            slots[i] = Some(x);
                        } else {
                            self.err(cx, line, format!("za dużo argumentów dla {}", name));
                        }
                        i += 1;
                    }
                }
            }
            let a: Vec<String> = slots
                .into_iter()
                .enumerate()
                .map(|(k, s)| {
                    s.unwrap_or_else(|| {
                        self.diags
                            .push(Diag::new(&cx.file, line, format!("brak argumentu {} dla {}", d.params[k].name, name)));
                        "undefined".into()
                    })
                })
                .collect();
            return self.aw(cx, format!("f_{}({})", name, a.join(", ")));
        }
        if env.record_fields(name).is_some() || env.variant_fields(name).is_some() {
            let fs: Vec<String> = args
                .iter()
                .map(|a| {
                    let x = self.expr(&a.value, cx);
                    format!("{}: {}", a.name.clone().unwrap_or_default(), x)
                })
                .collect();
            let f = if env.record_fields(name).is_some() { "$mk" } else { "$mkv" };
            return format!("{}({}, {{{}}})", f, js_str(name), fs.join(", "));
        }
        if let Some((_, is_async)) = builtin_fn(name) {
            let a: Vec<String> = args.iter().map(|a| self.expr(&a.value, cx)).collect();
            let c = format!("$b.{}({})", name, a.join(", "));
            if is_async {
                if cx.sync {
                    self.err(cx, line, format!("w warunku typu nie ma {}", name));
                }
                return self.aw(cx, c);
            }
            return c;
        }
        self.err(cx, line, format!("nieznana funkcja {}", name));
        "undefined".into()
    }
}

pub fn is_cap(t: &TypeExpr) -> bool {
    matches!(t, TypeExpr::Name { name, .. } if CAPS.contains(&name.as_str()))
}

fn ex_src(stmts: &[Stmt]) -> String {
    match stmts.first() {
        Some(Stmt::Expr { expr, .. }) => expr.to_string(),
        _ => String::new(),
    }
}

// Podpowiedzi dla generatora w property: z warunku typu, bez rozwiązywania go.
fn hints(c: &Cond) -> String {
    let mut h: Vec<String> = vec![];
    let mut conj = vec![];
    flatten_and(&c.expr, &mut conj);
    let is_p = |e: &Expr| matches!(&e.kind, ExprKind::Ident(n) if *n == c.param);
    let len_of_p = |e: &Expr| matches!(&e.kind, ExprKind::Call { name, args } if name == "len" && args.len() == 1 && is_p(&args[0].value));
    let num = |e: &Expr| match &e.kind {
        ExprKind::Int(n) => Some(n.to_string()),
        ExprKind::Dec(s) => Some(s.clone()),
        ExprKind::Neg(x) => match &x.kind {
            ExprKind::Int(n) => Some(format!("-{}", n)),
            _ => None,
        },
        _ => None,
    };
    for e in conj {
        match &e.kind {
            ExprKind::Bin { op, l, r } if is_p(l) => {
                if let Some(n) = num(r) {
                    match *op {
                        ">" => h.push(format!("min: {}, min_ex: true", n)),
                        ">=" => h.push(format!("min: {}", n)),
                        "<" => h.push(format!("max: {}, max_ex: true", n)),
                        "<=" => h.push(format!("max: {}", n)),
                        _ => {}
                    }
                }
            }
            ExprKind::Bin { op, l, r } if len_of_p(l) => {
                if let Some(n) = num(r) {
                    match *op {
                        "==" => h.push(format!("len: {}", n)),
                        ">" => h.push(format!("minlen: {} + 1", n)),
                        ">=" => h.push(format!("minlen: {}", n)),
                        "<" => h.push(format!("maxlen: {} - 1", n)),
                        "<=" => h.push(format!("maxlen: {}", n)),
                        _ => {}
                    }
                }
            }
            ExprKind::Call { name, args } if !args.is_empty() && is_p(&args[0].value) => {
                let s = args
                    .get(1)
                    .and_then(|a| if let ExprKind::Str(s) = &a.value.kind { Some(s.clone()) } else { None });
                match (name.as_str(), s) {
                    ("matches", Some(s)) => h.push(format!("re: {}", js_str(&s))),
                    ("starts_with", Some(s)) => h.push(format!("prefix: {}", js_str(&s))),
                    ("only_digits", _) => h.push("digits: true".into()),
                    ("valid_email", _) => h.push("email: true".into()),
                    ("nip_checksum_ok", _) => h.push("digits: true".into()),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    format!("{{{}}}", h.join(", "))
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
