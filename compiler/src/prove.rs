// Dowody warunków wyniku dla sowa review.
//
// Warunek w typie wyniku (`-> Int(α >= 0 && α <= 23)`, `-> Invoice(α.status == Issued) | E`)
// runtime sprawdza przy każdym return. prove szuka dowodu, że zachodzi zawsze: przechodzi ciało
// z faktami i przy każdym miejscu, z którego funkcja zwraca wartość (return, `or return`, try),
// pyta solver, czy fakty dają warunek z α zamienionym na zwracaną wartość.
//
// Fakty to warunki typów parametrów, warunek if na jego gałęzi (i zaprzeczony po if, który
// kończy się return), warunek z `x = e as T(...) or return`, warunek wyniku wołanej funkcji
// (`t = totals(...)`, gałąź `Invoice invoice =>` w match na wywołaniu) i wariant w gałęzi match.
// Zmienne `var`, przypisywane kilka razy i zmienne pętli nie dają faktów ani nie są podstawiane.
// Stałe lokalne bez uprawnień podstawia się do warunku. Bez dowodu warunek dalej sprawdza runtime.

use crate::ast::*;
use crate::codegen::is_cap;
use crate::env::{BUILTIN_RECORDS, BUILTIN_UNIONS, Env, builtin_fn};
use crate::solver::{self, Num, Verdict};
use std::collections::{HashMap, HashSet};

// None: funkcja nie ma ciała w impl/ albo warunku w typie wyniku.
pub fn prove(env: &Env, name: &str) -> Option<bool> {
    let info = env.fns.get(name)?;
    let body = info.body()?;
    let d = info.decl();
    let goal: Vec<(String, Vec<Expr>)> = d.ret.as_ref()?.alts().iter().map(|t| env.flatten(t)).collect();
    if goal.iter().all(|(_, cs)| cs.is_empty()) {
        return None;
    }
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut mutable = HashSet::new();
    assigns(body, &mut counts, &mut mutable);
    for (n, c) in &counts {
        if *c > 1 || d.params.iter().any(|p| &p.name == n) {
            mutable.insert(n.clone());
        }
    }
    let mut pr = Pr {
        env,
        goal,
        mutable,
        numeric: HashSet::new(),
        ok: true,
    };
    let mut cx = Cx::default();
    for p in &d.params {
        if is_cap(&p.ty) || pr.mutable.contains(&p.name) {
            continue;
        }
        cx.types.insert(p.name.clone(), p.ty.clone());
        let fs = pr.type_facts(&p.ty, &ident(&p.name));
        cx.facts.extend(fs);
    }
    pr.numeric = numeric_vars(env, body, &cx.types);
    pr.block(body, &mut cx);
    Some(pr.ok)
}

#[derive(Clone, Default)]
struct Cx {
    facts: Vec<Expr>,
    // Stałe lokalne do podstawienia, już z podstawionymi wcześniejszymi.
    lets: HashMap<String, Expr>,
    // Znane typy nazw: parametry, `as T`, wynik wywołania, gałąź match.
    types: HashMap<String, TypeExpr>,
}

struct Pr<'a> {
    env: &'a Env<'a>,
    // Alternatywy wyniku: typ bazowy i warunki z α.
    goal: Vec<(String, Vec<Expr>)>,
    mutable: HashSet<String>,
    // Zmienne, do których trafiają tylko liczby: dla solvera Money, bo Money obejmuje Int.
    numeric: HashSet<String>,
    ok: bool,
}

// Zwracana wartość: typ bazowy (None, gdy nieznany), warunki z α i wyrażenie w miejsce α
// (None, gdy wartość nie ma nazwy, np. wynik wywołania: wtedy α zostaje termem).
type Value = (Option<String>, Vec<Expr>, Option<Expr>);

impl<'a> Pr<'a> {
    fn block(&mut self, ss: &[Stmt], cx: &mut Cx) {
        for s in ss {
            if !self.ok {
                return;
            }
            match s {
                Stmt::Set { name, expr, .. } => {
                    self.exits(expr, cx);
                    if !self.mutable.contains(name) {
                        self.bind(name, expr, cx);
                    }
                }
                Stmt::Return { expr: Some(e), .. } => {
                    self.exits(e, cx);
                    self.ret(e, cx);
                }
                Stmt::Return { expr: None, .. } => {}
                Stmt::Expr { expr, .. } => self.exits(expr, cx),
                Stmt::If { cond, then, els, .. } => {
                    self.exits(cond, cx);
                    let mut t = cx.clone();
                    self.assume(cond, true, &mut t);
                    self.block(then, &mut t);
                    if let Some(e) = els {
                        let mut f = cx.clone();
                        self.assume(cond, false, &mut f);
                        self.block(e, &mut f);
                    }
                    let rt = always_returns(then);
                    let rf = els.as_ref().is_some_and(|e| always_returns(e));
                    if rt && !rf {
                        self.assume(cond, false, cx);
                    } else if rf && !rt {
                        self.assume(cond, true, cx);
                    }
                }
                Stmt::For { iter, body, .. } => {
                    self.exits(iter, cx);
                    self.block(body, &mut cx.clone());
                }
                Stmt::While { cond, body, .. } => {
                    self.exits(cond, cx);
                    let mut c = cx.clone();
                    self.assume(cond, true, &mut c);
                    self.block(body, &mut c);
                }
                Stmt::Match { subjects, arms, .. } => {
                    for s in subjects {
                        self.exits(s, cx);
                    }
                    for a in arms {
                        let mut c = cx.clone();
                        if let ([s], [p]) = (subjects.as_slice(), a.pats.as_slice()) {
                            self.arm(s, p, &mut c);
                        }
                        self.block(&a.body, &mut c);
                    }
                }
            }
        }
    }

    fn assume(&self, cond: &Expr, pos: bool, cx: &mut Cx) {
        self.fact(if pos { cond.clone() } else { not(cond) }, cx);
    }

    // Fakt musi być czysty i nie zależeć od zmiennych, które się zmieniają: inaczej w chwili
    // return mógłby już nie zachodzić.
    fn fact(&self, f: Expr, cx: &mut Cx) {
        if self.stable(&f) {
            cx.facts.push(f);
        }
    }

    fn stable(&self, e: &Expr) -> bool {
        pure(self.env, e) && free(e).iter().all(|n| !self.mutable.contains(n))
    }

    fn bind(&self, name: &str, expr: &Expr, cx: &mut Cx) {
        let x = ident(name);
        match &expr.kind {
            ExprKind::As { ty, alt, .. } => {
                let fs = self.type_facts(ty, &x);
                match alt.as_deref() {
                    // `x = e as T or d`: x spełnia warunek T albo jest równe d.
                    Some(Alt::Value(d)) => {
                        if !fs.is_empty() {
                            self.fact(bin("||", all(fs), bin("==", x.clone(), d.clone())), cx);
                        }
                        // Typ bez warunku: x może być równe d.
                        let (tb, _) = self.env.flatten(ty);
                        if self.values(d, cx).iter().all(|(b, _, _)| b.as_deref() == Some(tb.as_str())) {
                            cx.types.insert(name.to_string(), named(&tb));
                        }
                    }
                    Some(Alt::Block(_)) => {}
                    _ => {
                        for f in fs {
                            self.fact(f, cx);
                        }
                        cx.types.insert(name.to_string(), ty.clone());
                    }
                }
            }
            ExprKind::Call { .. } | ExprKind::Try(_) if call_name(expr).is_some_and(|f| self.env.fns.contains_key(f)) => {
                let (call, is_try) = match &expr.kind {
                    ExprKind::Try(c) => (c.as_ref(), true),
                    _ => (expr, false),
                };
                let Some(mut alts) = self.call_alts(call) else { return };
                if is_try {
                    alts.truncate(1);
                }
                if let [(ty, cs)] = alts.as_slice() {
                    cx.types.insert(name.to_string(), ty.clone());
                    for c in cs {
                        self.fact(alpha(c, &x), cx);
                    }
                }
            }
            // Stała z `var` w środku zmieniłaby wartość razem z nim.
            _ if self.stable(expr) => {
                let v = self.inline(expr, cx);
                cx.lets.insert(name.to_string(), v);
            }
            _ => {}
        }
    }

    // Gałąź match: `Typ nazwa` na wywołaniu dostaje warunek tej alternatywy wyniku,
    // a wariant bez danych daje `subject == Wariant`.
    fn arm(&self, s: &Expr, p: &Pat, cx: &mut Cx) {
        match p {
            Pat::Name { name: t, bind: Some(b) } if !self.mutable.contains(b) => {
                let ty = named(t);
                cx.types.insert(b.clone(), ty.clone());
                let call = match &s.kind {
                    ExprKind::Try(c) => c.as_ref(),
                    _ => s,
                };
                if let Some(alts) = self.call_alts(call) {
                    let (tb, _) = self.env.flatten(&ty);
                    let hit: Vec<&(TypeExpr, Vec<Expr>)> = alts.iter().filter(|(a, _)| self.env.flatten(a).0 == tb).collect();
                    if let [(_, cs)] = hit.as_slice() {
                        for c in cs {
                            self.fact(alpha(c, &ident(b)), cx);
                        }
                    }
                }
            }
            Pat::Name { name: v, bind: None } if self.plain_variant(v) && is_term(s) => {
                self.fact(bin("==", s.clone(), ident(v)), cx);
            }
            _ => {}
        }
    }

    fn plain_variant(&self, v: &str) -> bool {
        self.env.variants.get(v).is_some_and(|i| i.fields.is_none()) || BUILTIN_UNIONS.iter().any(|(_, vs)| vs.contains(&v))
    }

    // Wyjścia z funkcji w środku wyrażenia: `or return`, blok po `or`, try. Lambdy mają własny return.
    fn exits(&mut self, e: &Expr, cx: &Cx) {
        match &e.kind {
            ExprKind::Lambda { .. } => {}
            ExprKind::As { e: x, alt, .. } => {
                self.exits(x, cx);
                match alt.as_deref() {
                    Some(Alt::Return(Some(r))) => {
                        self.exits(r, cx);
                        self.ret(r, cx);
                    }
                    Some(Alt::Value(v)) => self.exits(v, cx),
                    Some(Alt::Block(ss)) => self.block(ss, &mut cx.clone()),
                    _ => {}
                }
            }
            ExprKind::Try(x) => {
                self.exits(x, cx);
                // try zwraca od razu wszystko poza pierwszą alternatywą wyniku.
                match self.call_alts(x) {
                    Some(alts) => {
                        for (t, cs) in alts.into_iter().skip(1) {
                            let (b, own) = self.env.flatten(&t);
                            let mut cs = cs;
                            cs.extend(own);
                            self.check((Some(b), cs, None), cx);
                        }
                    }
                    // Metoda uprawnienia z błędami (mail.send): błędy to warianty jej typu błędów.
                    None => match &x.kind {
                        ExprKind::Method { name, .. } if crate::env::CAP_TRY.iter().any(|(_, m, _)| m == name) => {
                            for (_, _, err) in crate::env::CAP_TRY.iter().filter(|(_, m, _)| m == name) {
                                for l in self.env.leaves(&named(err)) {
                                    self.check((Some(l), vec![], None), cx);
                                }
                            }
                        }
                        _ => self.ok = false,
                    },
                }
            }
            _ => {
                for c in children(e) {
                    self.exits(c, cx);
                }
            }
        }
    }

    fn ret(&mut self, e: &Expr, cx: &Cx) {
        for v in self.values(e, cx) {
            self.check(v, cx);
        }
    }

    // Możliwe wartości wyrażenia z typem bazowym i warunkami.
    fn values(&self, e: &Expr, cx: &Cx) -> Vec<Value> {
        let env = self.env;
        let lit = |b: &str| vec![(Some(b.to_string()), vec![], Some(e.clone()))];
        match &e.kind {
            ExprKind::Int(_) => lit("Int"),
            ExprKind::Dec(_) => lit("Money"),
            ExprKind::Str(_) => lit("String"),
            ExprKind::Bool(_) => lit("Bool"),
            ExprKind::Html(_) => lit("Html"),
            ExprKind::Ident(n) if cx.types.contains_key(n) => cx.types[n]
                .alts()
                .iter()
                .map(|t| env.flatten(t))
                .map(|(b, cs)| (Some(b), cs, Some(e.clone())))
                .collect(),
            ExprKind::Ident(n) if cx.lets.contains_key(n) => self.values(&cx.lets[n], cx),
            ExprKind::Ident(n) if env.variants.contains_key(n) => lit(n),
            ExprKind::Call { name, .. } if is_ctor(env, name) => lit(name),
            ExprKind::Call { .. } if self.call_alts(e).is_some() => self
                .call_alts(e)
                .unwrap()
                .into_iter()
                .map(|(t, mut cs)| {
                    let (b, own) = env.flatten(&t);
                    cs.extend(own);
                    (Some(b), cs, None)
                })
                .collect(),
            ExprKind::Try(x) => self.values(x, cx),
            ExprKind::As { e: x, ty, alt } => {
                let mut out: Vec<Value> = ty
                    .alts()
                    .iter()
                    .map(|t| env.flatten(t))
                    .map(|(b, cs)| (Some(b), cs, Some((**x).clone())))
                    .collect();
                if let Some(Alt::Value(v)) = alt.as_deref() {
                    out.extend(self.values(v, cx));
                } else if let Some(Alt::Block(_)) = alt.as_deref() {
                    out.push((None, vec![], None));
                }
                out
            }
            // Po `with` warunki starej wartości już nie obowiązują, zostaje typ.
            ExprKind::With { e: x, .. } => self.values(x, cx).into_iter().map(|(b, _, _)| (b, vec![], Some(e.clone()))).collect(),
            _ => vec![(None, vec![], Some(e.clone()))],
        }
    }

    // Alternatywy wyniku wołanej funkcji z warunkami, w których parametry zastępują argumenty.
    // Argument z uprawnieniem albo lambdą niczego nie podstawia.
    fn call_alts(&self, call: &Expr) -> Option<Vec<(TypeExpr, Vec<Expr>)>> {
        let ExprKind::Call { name, args } = &call.kind else { return None };
        let d = self.env.fns.get(name)?.decl();
        let ret = d.ret.as_ref()?;
        let mut by: HashMap<String, Expr> = HashMap::new();
        let mut pos = 0;
        for a in args {
            let p = match &a.name {
                Some(n) => Some(n.clone()),
                None => {
                    pos += 1;
                    d.params.get(pos - 1).map(|p| p.name.clone())
                }
            };
            if let Some(p) = p.filter(|_| pure(self.env, &a.value)) {
                by.insert(p, a.value.clone());
            }
        }
        let params: HashSet<&str> = d.params.iter().map(|p| p.name.as_str()).collect();
        Some(
            ret.alts()
                .into_iter()
                .map(|t| {
                    let (_, cs) = self.env.flatten(t);
                    let cs = cs
                        .into_iter()
                        // Warunek z parametrem bez argumentu nic o wyniku nie mówi.
                        .filter(|c| free(c).iter().all(|n| !params.contains(n.as_str()) || by.contains_key(n)))
                        .map(|c| subst(&c, &|n| if params.contains(n) { by.get(n).cloned() } else { None }))
                        .collect();
                    (t.clone(), cs)
                })
                .collect(),
        )
    }

    fn check(&mut self, (base, conds, sub): Value, cx: &Cx) {
        let env = self.env;
        let goal: Vec<&(String, Vec<Expr>)> = match &base {
            Some(b) => self.goal.iter().filter(|(g, _)| g == b || (b == "Int" && g == "Money")).collect(),
            None if self.goal.len() == 1 => vec![&self.goal[0]],
            None => {
                self.ok = false;
                return;
            }
        };
        if goal.is_empty() {
            // Wariant spoza warunku, ale nazwana unia może zawierać wariant z warunkiem.
            let ls = env.leaves(&named(base.as_deref().unwrap_or("")));
            if self.goal.iter().any(|(g, cs)| !cs.is_empty() && ls.contains(g)) {
                self.ok = false;
            }
            return;
        }
        if goal.iter().any(|(_, cs)| cs.is_empty()) {
            return;
        }
        let want = goal.iter().map(|(_, cs)| all(cs.clone())).reduce(|a, b| bin("||", a, b)).unwrap();
        let put = |x: &Expr| {
            let x = match &sub {
                Some(v) => alpha(x, v),
                None => x.clone(),
            };
            self.simp(&self.inline(&x, cx))
        };
        let want = put(&want);
        let mut facts: Vec<Expr> = cx.facts.iter().map(|f| self.simp(&self.inline(f, cx))).collect();
        facts.extend(conds.iter().map(put));
        let ab = if sub.is_none() { base.clone() } else { None };
        let num = |e: &Expr| -> Option<Num> {
            match self.term_base(e, cx, ab.as_deref())?.as_str() {
                "Int" => Some(Num::Int),
                // W runtime Money nie musi być w groszach, więc dla dowodu to liczba wymierna.
                "Money" => Some(Num::Dec),
                _ => None,
            }
        };
        let dom = |e: &Expr| -> Option<Vec<String>> { self.domain(&self.term_base(e, cx, ab.as_deref())?) };
        if solver::implies(&facts, &[want], &num, &dom) != Verdict::Implied {
            self.ok = false;
        }
    }

    // Typ bazowy termu: α, nazwa, pole.
    fn term_base(&self, e: &Expr, cx: &Cx, alpha: Option<&str>) -> Option<String> {
        match &e.kind {
            ExprKind::Ident(n) if n == "α" => alpha.map(|a| a.to_string()),
            ExprKind::Ident(n) => match cx.types.get(n).map(|t| t.alts()) {
                Some(ts) if ts.len() == 1 => Some(self.env.flatten(ts[0]).0),
                _ if self.numeric.contains(n) => Some("Money".into()),
                _ => None,
            },
            ExprKind::Field { obj, name } => {
                let b = self.term_base(obj, cx, alpha)?;
                let t = self.field_ty(&b, name)?;
                Some(self.env.flatten(&t).0)
            }
            _ => None,
        }
    }

    fn field_ty(&self, rec: &str, field: &str) -> Option<TypeExpr> {
        if let Some((_, fs)) = BUILTIN_RECORDS.iter().find(|(r, _)| *r == rec) {
            return fs.iter().find(|(f, _)| *f == field).map(|(_, t)| named(t));
        }
        if let Some((
            TypeDecl {
                body: TypeBody::Record(fs), ..
            },
            _,
        )) = self.env.types.get(rec)
        {
            return fs.iter().find(|f| f.name == field).map(|f| f.ty.clone());
        }
        let fs = self.env.variants.get(rec)?.fields?;
        fs.iter().find(|f| f.name == field).map(|f| f.ty.clone())
    }

    fn domain(&self, ty: &str) -> Option<Vec<String>> {
        let ls = self.env.leaves(&named(ty));
        (ls.len() > 1 && ls.iter().all(|v| self.plain_variant(v))).then_some(ls)
    }

    // Warunki typu z jedną alternatywą, z α zamienionym na x.
    fn type_facts(&self, t: &TypeExpr, x: &Expr) -> Vec<Expr> {
        match t.alts().as_slice() {
            [one] => self.env.flatten(one).1.iter().map(|c| alpha(c, x)).collect(),
            _ => vec![],
        }
    }

    fn inline(&self, e: &Expr, cx: &Cx) -> Expr {
        subst(e, &|n| cx.lets.get(n).cloned())
    }

    // Uproszczenia, których solver nie zna: pole konstruktora, ta sama strona porównania,
    // starts_with na sklejeniu ze stałym początkiem, `is` na konstruktorze.
    fn simp(&self, e: &Expr) -> Expr {
        let env = self.env;
        let k = |b: bool| Expr {
            kind: ExprKind::Bool(b),
            line: e.line,
        };
        let s = |x: &Expr| Box::new(self.simp(x));
        match &e.kind {
            ExprKind::Field { obj, name } => {
                let o = self.simp(obj);
                match &o.kind {
                    ExprKind::Call { name: c, args } if is_ctor(env, c) => {
                        if let Some(v) = ctor_arg(env, c, args, name) {
                            return self.simp(v);
                        }
                    }
                    ExprKind::With { e: base, fields } => {
                        return match fields.iter().find(|(f, _)| f == name) {
                            Some((_, v)) => self.simp(v),
                            None => self.simp(&Expr {
                                kind: ExprKind::Field {
                                    obj: base.clone(),
                                    name: name.clone(),
                                },
                                line: e.line,
                            }),
                        };
                    }
                    _ => {}
                }
                Expr {
                    kind: ExprKind::Field {
                        obj: Box::new(o),
                        name: name.clone(),
                    },
                    line: e.line,
                }
            }
            ExprKind::Bin { op, l, r } => {
                let (l, r) = (s(l), s(r));
                if matches!(*op, "==" | "<=" | ">=") && l.to_string() == r.to_string() && pure(env, &l) {
                    return k(true);
                }
                Expr {
                    kind: ExprKind::Bin { op, l, r },
                    line: e.line,
                }
            }
            ExprKind::Not(x) => Expr {
                kind: ExprKind::Not(s(x)),
                line: e.line,
            },
            ExprKind::Call { name, args } if name == "starts_with" && args.len() == 2 => {
                let a = self.simp(&args[0].value);
                if let (Some(p), ExprKind::Str(q)) = (lead(&a), &args[1].value.kind) {
                    if p.starts_with(q.as_str()) {
                        return k(true);
                    }
                }
                e.clone()
            }
            ExprKind::Is { e: x, ty, neg } => {
                let x = self.simp(x);
                let v = match &x.kind {
                    ExprKind::Call { name, .. } if is_ctor(env, name) => Some(name.clone()),
                    ExprKind::Ident(n) if env.variants.contains_key(n) => Some(n.clone()),
                    _ => None,
                };
                // Typ z warunkiem albo unia z tym wariantem zależy od danych, więc zostaje.
                if let (Some(v), TypeExpr::Name { name: t, cond: None, .. }) = (v, ty) {
                    let known = env.is_type(t) || env.variants.contains_key(t);
                    if &v == t {
                        return k(!*neg);
                    }
                    if known && !env.leaves(ty).contains(&v) {
                        return k(*neg);
                    }
                }
                Expr {
                    kind: ExprKind::Is {
                        e: Box::new(x),
                        ty: ty.clone(),
                        neg: *neg,
                    },
                    line: e.line,
                }
            }
            _ => e.clone(),
        }
    }
}

fn ident(n: &str) -> Expr {
    Expr {
        kind: ExprKind::Ident(n.to_string()),
        line: 0,
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

fn bin(op: &'static str, l: Expr, r: Expr) -> Expr {
    Expr {
        line: l.line,
        kind: ExprKind::Bin {
            op,
            l: Box::new(l),
            r: Box::new(r),
        },
    }
}

fn not(e: &Expr) -> Expr {
    Expr {
        kind: ExprKind::Not(Box::new(e.clone())),
        line: e.line,
    }
}

fn all(cs: Vec<Expr>) -> Expr {
    cs.into_iter().reduce(|a, b| bin("&&", a, b)).unwrap_or_else(|| Expr {
        kind: ExprKind::Bool(true),
        line: 0,
    })
}

fn alpha(c: &Expr, x: &Expr) -> Expr {
    subst(c, &|n| (n == "α").then(|| x.clone()))
}

fn call_name(e: &Expr) -> Option<&String> {
    match &e.kind {
        ExprKind::Call { name, .. } => Some(name),
        ExprKind::Try(c) => call_name(c),
        _ => None,
    }
}

fn is_ctor(env: &Env, n: &str) -> bool {
    env.variants.contains_key(n) || env.record_fields(n).is_some()
}

// Argument konstruktora dla pola: nazwany albo na pozycji pola w definicji.
fn ctor_arg<'e>(env: &Env, c: &str, args: &'e [Arg], field: &str) -> Option<&'e Expr> {
    if let Some(a) = args.iter().find(|a| a.name.as_deref() == Some(field)) {
        return Some(&a.value);
    }
    let fs = env.record_fields(c).or_else(|| env.variant_fields(c))?;
    let i = fs.iter().position(|f| f == field)?;
    args.get(i).filter(|a| a.name.is_none()).map(|a| &a.value)
}

// Stały początek tekstu: "abc" albo "abc" + reszta.
fn lead(e: &Expr) -> Option<&str> {
    match &e.kind {
        ExprKind::Str(s) => Some(s),
        ExprKind::Bin { op: "+", l, .. } => lead(l),
        _ => None,
    }
}

fn is_term(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Ident(_) => true,
        ExprKind::Field { obj, .. } => is_term(obj),
        _ => false,
    }
}

// Wyrażenie bez uprawnień: przy każdym obliczeniu daje to samo.
fn pure(env: &Env, e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Dec(_) | ExprKind::Str(_) | ExprKind::Bool(_) | ExprKind::Ident(_) => true,
        ExprKind::Field { obj, .. } | ExprKind::Not(obj) | ExprKind::Neg(obj) => pure(env, obj),
        ExprKind::Bin { l, r, .. } => pure(env, l) && pure(env, r),
        ExprKind::List(xs) => xs.iter().all(|x| pure(env, x)),
        ExprKind::Is { e, .. } => pure(env, e),
        ExprKind::With { e, fields } => pure(env, e) && fields.iter().all(|(_, v)| pure(env, v)),
        ExprKind::Call { name, args } => {
            let ok = is_ctor(env, name) || builtin_fn(name).is_some() || env.fns.get(name).is_some_and(|i| !i.decl().params.iter().any(|p| is_cap(&p.ty)));
            ok && args.iter().all(|a| pure(env, &a.value))
        }
        _ => false,
    }
}

// Nazwy użyte w wyrażeniu (bez nazw typów i wariantów pisanych wielką literą).
fn free(e: &Expr) -> HashSet<String> {
    let mut out = HashSet::new();
    free_into(e, &mut out);
    out
}

fn free_into(e: &Expr, out: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::Ident(n) => {
            out.insert(n.clone());
        }
        ExprKind::Lambda { param, body } => {
            let mut inner = HashSet::new();
            match body {
                LambdaBody::Expr(b) => free_into(b, &mut inner),
                // Blok lambdy: ostrożnie wszystkie nazwy z otoczenia są w użyciu.
                LambdaBody::Block(_) => {
                    inner.insert("\u{0}blok".into());
                }
            }
            inner.remove(param);
            out.extend(inner);
        }
        _ => {
            for c in children(e) {
                free_into(c, out);
            }
        }
    }
}

fn children(e: &Expr) -> Vec<&Expr> {
    match &e.kind {
        ExprKind::Field { obj, .. } | ExprKind::Not(obj) | ExprKind::Neg(obj) | ExprKind::Try(obj) => vec![obj],
        ExprKind::Is { e, .. } => vec![e],
        ExprKind::As { e, alt, .. } => {
            let mut v: Vec<&Expr> = vec![e];
            if let Some(Alt::Value(x) | Alt::Return(Some(x))) = alt.as_deref() {
                v.push(x);
            }
            v
        }
        ExprKind::With { e, fields } => std::iter::once(&**e).chain(fields.iter().map(|(_, v)| v)).collect(),
        ExprKind::Bin { l, r, .. } => vec![l, r],
        ExprKind::List(xs) => xs.iter().collect(),
        ExprKind::Call { args, .. } => args.iter().map(|a| &a.value).collect(),
        ExprKind::Method { obj, args, .. } => std::iter::once(&**obj).chain(args.iter().map(|a| &a.value)).collect(),
        ExprKind::Html(segs) => segs
            .iter()
            .filter_map(|s| match s {
                HtmlSeg::Expr(x) => Some(x),
                HtmlSeg::Lit(_) => None,
            })
            .collect(),
        _ => vec![],
    }
}

fn always_returns(ss: &[Stmt]) -> bool {
    match ss.last() {
        Some(Stmt::Return { .. }) => true,
        Some(Stmt::If { then, els: Some(e), .. }) => always_returns(then) && always_returns(e),
        Some(Stmt::Match { arms, .. }) => !arms.is_empty() && arms.iter().all(|a| always_returns(&a.body)),
        _ => false,
    }
}

// Liczba przypisań każdej nazwy w ciele (z blokami lambd i gałęziami match); `var` i zmienne
// pętli od razu trafiają do zmiennych.
fn assigns(ss: &[Stmt], counts: &mut HashMap<String, usize>, mutable: &mut HashSet<String>) {
    let ex = |e: &Expr, counts: &mut HashMap<String, usize>, mutable: &mut HashSet<String>| assigns_expr(e, counts, mutable);
    for s in ss {
        match s {
            Stmt::Set { name, is_var, expr, .. } => {
                *counts.entry(name.clone()).or_insert(0) += 1;
                if *is_var {
                    mutable.insert(name.clone());
                }
                ex(expr, counts, mutable);
            }
            Stmt::Return { expr: Some(e), .. } | Stmt::Expr { expr: e, .. } => ex(e, counts, mutable),
            Stmt::Return { expr: None, .. } => {}
            Stmt::If { cond, then, els, .. } => {
                ex(cond, counts, mutable);
                assigns(then, counts, mutable);
                if let Some(e) = els {
                    assigns(e, counts, mutable);
                }
            }
            Stmt::For { var, iter, body, .. } => {
                mutable.insert(var.clone());
                ex(iter, counts, mutable);
                assigns(body, counts, mutable);
            }
            Stmt::While { cond, body, .. } => {
                ex(cond, counts, mutable);
                assigns(body, counts, mutable);
            }
            Stmt::Match { subjects, arms, .. } => {
                for s in subjects {
                    ex(s, counts, mutable);
                }
                for a in arms {
                    for p in &a.pats {
                        pat_binds(p, counts);
                    }
                    assigns(&a.body, counts, mutable);
                }
            }
        }
    }
}

fn pat_binds(p: &Pat, counts: &mut HashMap<String, usize>) {
    match p {
        Pat::Bind(n) | Pat::Name { bind: Some(n), .. } => *counts.entry(n.clone()).or_insert(0) += 1,
        Pat::List(ps) => ps.iter().for_each(|p| pat_binds(p, counts)),
        _ => {}
    }
}

fn assigns_expr(e: &Expr, counts: &mut HashMap<String, usize>, mutable: &mut HashSet<String>) {
    match &e.kind {
        ExprKind::Lambda { param, body } => {
            mutable.insert(param.clone());
            match body {
                LambdaBody::Expr(b) => assigns_expr(b, counts, mutable),
                LambdaBody::Block(ss) => assigns(ss, counts, mutable),
            }
        }
        ExprKind::As { alt, .. } if matches!(alt.as_deref(), Some(Alt::Block(_))) => {
            if let Some(Alt::Block(ss)) = alt.as_deref() {
                assigns(ss, counts, mutable);
            }
            for c in children(e) {
                assigns_expr(c, counts, mutable);
            }
        }
        _ => {
            for c in children(e) {
                assigns_expr(c, counts, mutable);
            }
        }
    }
}

// Zmienne, którym ciało przypisuje tylko liczby (największy punkt stały: `net = net + x`).
fn numeric_vars(env: &Env, body: &[Stmt], types: &HashMap<String, TypeExpr>) -> HashSet<String> {
    let mut sets: Vec<(String, Expr)> = vec![];
    collect_sets(body, &mut sets);
    let mut num: HashSet<String> = sets.iter().map(|(n, _)| n.clone()).collect();
    loop {
        let bad: Vec<String> = sets
            .iter()
            .filter(|(n, e)| num.contains(n) && !numeric(env, e, &num, types))
            .map(|(n, _)| n.clone())
            .collect();
        if bad.is_empty() {
            return num;
        }
        for n in bad {
            num.remove(&n);
        }
    }
}

fn collect_sets(ss: &[Stmt], out: &mut Vec<(String, Expr)>) {
    for s in ss {
        match s {
            Stmt::Set { name, expr, .. } => out.push((name.clone(), expr.clone())),
            Stmt::If { then, els, .. } => {
                collect_sets(then, out);
                if let Some(e) = els {
                    collect_sets(e, out);
                }
            }
            Stmt::For { body, .. } | Stmt::While { body, .. } => collect_sets(body, out),
            Stmt::Match { arms, .. } => arms.iter().for_each(|a| collect_sets(&a.body, out)),
            _ => {}
        }
    }
}

fn numeric(env: &Env, e: &Expr, num: &HashSet<String>, types: &HashMap<String, TypeExpr>) -> bool {
    let is_num = |t: &TypeExpr| t.alts().len() == 1 && matches!(env.flatten(t).0.as_str(), "Int" | "Money");
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Dec(_) => true,
        ExprKind::Neg(x) => numeric(env, x, num, types),
        ExprKind::Bin { op, l, r } if matches!(*op, "+" | "-" | "*" | "/" | "%") => numeric(env, l, num, types) && numeric(env, r, num, types),
        ExprKind::Ident(n) => num.contains(n) || types.get(n).is_some_and(is_num),
        ExprKind::Call { name, .. } if matches!(name.as_str(), "round" | "sum" | "len") => true,
        ExprKind::Call { name, .. } => env.fns.get(name).and_then(|i| i.decl().ret.as_ref()).is_some_and(is_num),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str, name: &str) -> Option<bool> {
        let f = crate::parser::parse_file(src, "impl/t.sowa", "t", true).unwrap_or_else(|d| panic!("{}", d));
        let files = vec![f];
        let env = Env::build(&files);
        prove(&env, name)
    }

    #[test]
    fn literals_and_match() {
        let src = "type Rate = A | B\nfn pct(r: Rate) -> Int(α >= 0 && α <= 23)\n  match r\n    A => return 23\n    B => return 0\n";
        assert_eq!(run(src, "pct"), Some(true));
        let src = "type Rate = A | B\nfn pct(r: Rate) -> Int(α >= 0 && α <= 23)\n  match r\n    A => return 24\n    B => return 0\n";
        assert_eq!(run(src, "pct"), Some(false));
    }

    #[test]
    fn params_and_if() {
        let src = "fn f(x: Int(α > 0)) -> Int(α > 1)\n  return x + 1\n";
        assert_eq!(run(src, "f"), Some(true));
        let src = "fn f(x: Int) -> Int(α >= 0)\n  if x < 0\n    return 0 - x\n  return x\n";
        assert_eq!(run(src, "f"), Some(true));
        let src = "fn f(x: Int) -> Int(α >= 0)\n  return x\n";
        assert_eq!(run(src, "f"), Some(false));
        assert_eq!(run("fn f(x: Int) -> Int\n  return x\n", "f"), None);
    }

    #[test]
    fn records_and_calls() {
        let src = "type T
  a: Money
  b: Money
  c: Money

fn t(x: Money, y: Money) -> T(α.c == α.a + α.b)
  var a = x
  a = a + 1
  return T(a: a, b: y, c: a + y)
fn u(x: Money) -> T(α.c == α.a + α.b)
  t0 = t(x, x)
  return T(a: t0.a, b: t0.b, c: t0.c)
fn w(x: Money) -> T(α.c == α.a + α.b)
  return t(x, 1)
";
        assert_eq!(run(src, "t"), Some(true));
        assert_eq!(run(src, "u"), Some(true));
        assert_eq!(run(src, "w"), Some(true));
    }

    // Przypadki, w których łatwo o fałszywy dowód.
    #[test]
    fn no_false_proofs() {
        let src = "type T
  a: Int
  b: Int
  c: Int

fn f() -> T(α.c == α.a + α.b)
  var k = 0
  y = k
  k = 5
  return T(a: y, b: 0, c: k)
fn g(x: Money(α > 0)) -> Money(α >= 0.01)
  return x
fn h(x: Int(α > 0)) -> Int(α >= 1)
  return x
fn i(x: Int) -> Int(α > 0)
  y = x as Int(α > 5) or 0
  return y
fn j(x: Int) -> Int(α >= 0)
  y = x as Int(α > 5) or 0
  return y
";
        assert_eq!(run(src, "f"), Some(false));
        assert_eq!(run(src, "g"), Some(false));
        assert_eq!(run(src, "h"), Some(true));
        assert_eq!(run(src, "i"), Some(false));
        assert_eq!(run(src, "j"), Some(true));
    }

    #[test]
    fn variants_and_as() {
        let src = "type S = Draft | Done
type Doc
  s: S

type E = Nope
fn mk(d: Doc) -> Doc(α.s == Done) | E
  if d.s == Done
    return d
  return Nope
fn mk2(d: Doc) -> Doc(α.s == Done) | E
  x = d as Doc(α.s == Done) or return Nope
  return x
fn mk3(d: Doc) -> Doc(α.s == Done) | E
  return d
fn mk4(d: Doc) -> Doc(α.s == Done) | E
  return d with s: Done
fn mk5(d: Doc) -> Doc(α.s == Done) | E
  return d with s: Draft
fn path(n: String) -> String(starts_with(α, \"/a/\"))
  return \"/a/\" + n
";
        assert_eq!(run(src, "mk"), Some(true));
        assert_eq!(run(src, "mk2"), Some(true));
        assert_eq!(run(src, "mk3"), Some(false));
        assert_eq!(run(src, "mk4"), Some(true));
        assert_eq!(run(src, "mk5"), Some(false));
        assert_eq!(run(src, "path"), Some(true));
    }
}
