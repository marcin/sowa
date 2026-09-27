// Generator Rusta dla `sowa test --rust` i `sowa run --rust`: te same testy i ten sam program co
// codegen.rs, ale jako program w Ruście, który rustc kompiluje razem z runtime.rs do jednego pliku
// wykonywalnego.
//
// Nazwy: zmienne v_nazwa, funkcje f_nazwa, rekordy S_Nazwa, sprawdzenia warunków typu k_N, typy
// i stałe bez zmiennych z otoczenia to funkcje c_N z wartością w thread_local. Błąd idzie przez `?`.
// `or` i `try` są rozwinięte w miejscu: return w bloku `or` wychodzi z najbliższej funkcji albo
// lambdy, tak jak $Ret w JS.
//
// Typy w kompilacji (Rt): Int to i64, Bool to bool, String to Rc<str>, List<T> to Rc<Vec<T>>, a rekord
// użytkownika to struct. Reszta (Money, daty, warianty, unie, uprawnienia) to dynamiczne V z runtime.
// Typ zmiennej to złączenie typów wszystkich przypisanych do niej wartości. Generator liczy go
// punktem stałym: generuje ciało z podpowiedziami z poprzedniego przebiegu, aż przestaną się zmieniać.
// Warunek typu na wartości z typem w kompilacji sprawdza funkcja k_N; wartość, która raz przeszła
// sprawdzenie, nie jest sprawdzana znowu (pola rekordu, elementy listy).

use crate::ast::*;
use crate::env::*;
use crate::moves::*;
use crate::project::Project;
use crate::toml;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq)]
enum Ret {
    // return w funkcji: sprawdź typ wyniku.
    Conform,
    // return w lambdzie.
    Plain,
    // return w lambdzie z map na element tego samego typu: Some(nowy) albo None, gdy element bez zmian.
    Keep,
    // return w teście kończy test, a `try` i `or return` to niezaliczony test.
    Test,
    // return w bloku `or` w teście.
    TestThrow,
}

// Typ wartości znany w kompilacji. Never to typ elementu pustej listy.
#[derive(Clone, PartialEq, Debug)]
enum Rt {
    Never,
    Dyn,
    Int,
    Bool,
    Str,
    List(Box<Rt>),
    Rec(String),
    Cap(String),
}

impl Rt {
    // Wartość jako V.
    fn dynamic(&self) -> bool {
        matches!(self, Rt::Never | Rt::Dyn | Rt::Cap(_))
    }
    fn copy(&self) -> bool {
        matches!(self, Rt::Int | Rt::Bool)
    }
    fn rs(&self) -> String {
        match self {
            Rt::Never | Rt::Dyn | Rt::Cap(_) => "V".into(),
            Rt::Int => "i64".into(),
            Rt::Bool => "bool".into(),
            Rt::Str => "Rc<str>".into(),
            Rt::List(t) => format!("Rc<Vec<{}>>", t.rs()),
            Rt::Rec(n) => format!("S_{}", ident(n)),
        }
    }
}

fn join(a: &Rt, b: &Rt) -> Rt {
    match (a, b) {
        (Rt::Never, x) | (x, Rt::Never) => x.clone(),
        (Rt::List(x), Rt::List(y)) => Rt::List(Box::new(join(x, y))),
        _ if a == b => a.clone(),
        _ => Rt::Dyn,
    }
}

// Wartość typu from pasuje do to bez sprawdzania w runtime.
fn compatible(from: &Rt, to: &Rt) -> bool {
    !from.dynamic() && join(from, to) == *to
}

const EMPTY: &str = "Rc::new(Vec::<V>::new())";

fn coerce(code: String, from: &Rt, to: &Rt) -> String {
    if from == to {
        return code;
    }
    if to.dynamic() {
        return if from.dynamic() { code } else { format!("to_v({})", code) };
    }
    match (from, to) {
        (Rt::List(_), Rt::List(b)) => {
            if code == EMPTY {
                "Rc::new(Vec::new())".into()
            } else {
                format!("lconv::<_, {}>({})?", b.rs(), code)
            }
        }
        (f, Rt::Bool) if f.dynamic() => format!("bool_({})?", code),
        (_, Rt::Bool) => format!("bool_(to_v({}))?", code),
        (f, t) if f.dynamic() => format!("from_v::<{}>({})?", t.rs(), code),
        (_, t) => format!("from_v::<{}>(to_v({}))?", t.rs(), code),
    }
}

fn bool_code(code: String, rt: &Rt) -> String {
    coerce(code, rt, &Rt::Bool)
}

// Klucz zmiennej, która jest referencją na element listy (Gen::sink).
const REF: usize = usize::MAX;

// `x.clone()` dla prostej ścieżki zamienia na referencję bez klonu.
fn clone_path(code: &str) -> Option<&str> {
    let p = code.strip_suffix(".clone()")?;
    let rest = match p.strip_prefix("(*") {
        Some(r) => r.replacen(')', "", 1),
        None => p.to_string(),
    };
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        Some(p)
    } else {
        None
    }
}
fn by_ref(code: &str) -> String {
    match clone_path(code) {
        Some(p) => format!("&{}", p),
        None => format!("&({})", code),
    }
}
// Argument dla parametru przez referencję: element listy bez klonu.
fn ref_arg(code: &str) -> String {
    match code.strip_prefix("at_(").and_then(|c| c.strip_suffix(")?")) {
        Some(inner) => format!("at_ref({})?", inner),
        None => by_ref(code),
    }
}

// Tekst jako &str: literał wprost, reszta przez referencję.
fn str_ref(e: &Expr, code: &str) -> String {
    if let ExprKind::Str(s) = &e.kind {
        return q(s);
    }
    match clone_path(code) {
        Some(p) => format!("&*{}", p),
        None => format!("&*({})", code),
    }
}

struct Cx {
    // Zmienne w zakresach: typ i klucz deklaracji (0 dla typu ustalonego z góry).
    scopes: Vec<HashMap<String, (Rt, usize)>>,
    // Nazwy z otoczenia użyte w lambdzie albo warunku typu: trzeba je sklonować do domknięcia.
    caps: Vec<HashSet<String>>,
    ret: Ret,
    // Typ wyniku funkcji albo lambdy i typ z sygnatury.
    ret_rt: Rt,
    ret_te: Option<TypeExpr>,
    fname: String,
    // Typy wartości w return lambdy.
    rets: Vec<Rt>,
    // Parametr lambdy w trybie Keep, którego nikt nie przypisuje: `return param` to None.
    keep: Option<String>,
    tmp: usize,
    file: String,
    // Typy zmiennych z poprzedniego przebiegu i typy widziane w tym.
    hints: HashMap<usize, Rt>,
    seen: HashMap<usize, Rt>,
}

impl Cx {
    fn new(file: &str, ret: Ret) -> Cx {
        Cx {
            scopes: vec![HashMap::new()],
            caps: vec![],
            ret,
            ret_rt: Rt::Dyn,
            ret_te: None,
            fname: String::new(),
            rets: vec![],
            keep: None,
            tmp: 0,
            file: file.to_string(),
            hints: HashMap::new(),
            seen: HashMap::new(),
        }
    }
    fn has(&self, n: &str) -> bool {
        self.scopes.iter().any(|s| s.contains_key(n))
    }
    fn get(&self, n: &str) -> Option<(Rt, usize)> {
        self.scopes.iter().rev().find_map(|s| s.get(n).cloned())
    }
    fn declare(&mut self, n: &str, rt: Rt, key: usize) {
        self.scopes.last_mut().unwrap().insert(n.to_string(), (rt, key));
    }
    fn observe(&mut self, key: usize, rt: &Rt) {
        if key == 0 || key == REF {
            return;
        }
        let t = match self.seen.get(&key) {
            Some(s) => join(s, rt),
            None => rt.clone(),
        };
        self.seen.insert(key, t);
    }
    fn tmp(&mut self) -> String {
        self.tmp += 1;
        format!("t{}", self.tmp)
    }
    fn names(&self) -> HashSet<String> {
        self.scopes.iter().flat_map(|s| s.keys().cloned()).collect()
    }
    // Nazwy z ramki, które po zamknięciu zakresów domknięcia nadal są w otoczeniu.
    fn pop_caps(&mut self) -> Vec<String> {
        let f = self.caps.pop().unwrap_or_default();
        let mut out: Vec<String> = f.into_iter().filter(|n| self.has(n)).collect();
        out.sort();
        out
    }
}

type RecFields<'a> = Vec<(String, &'a TypeExpr, Rt)>;

pub struct Gen<'a> {
    env: &'a Env<'a>,
    consts: String,
    const_ids: HashMap<String, usize>,
    // Odczyty zmiennych, które przenoszą wartość zamiast ją klonować (moves.rs).
    moves: Moves,
    // Rekordy jako struct: pola w kolejności z definicji.
    recs: BTreeMap<String, RecFields<'a>>,
    // Typy parametrów i wyniku funkcji.
    sigs: HashMap<String, (Vec<Rt>, Rt)>,
    // Parametry przez referencję: ciało ich nie zmienia ani nie przenosi.
    borrowed: HashMap<String, Vec<bool>>,
    // Przypisania `x = at(xs, i)` z referencją do elementu (moves::ref_lets).
    ref_lets: HashSet<usize>,
    // Funkcje k_N według typu.
    checks: HashMap<String, Option<String>>,
}

fn wrap(pre: String, e: String) -> String {
    if pre.is_empty() { e } else { format!("{{ {}{} }}", pre, e) }
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

fn stmt_key(s: &Stmt) -> usize {
    s as *const Stmt as usize
}

// `join(xs.map(..), sep)` albo `+` z nim: tekst warto budować w jednym buforze (Gen::sink).
fn fusable(e: &Expr) -> bool {
    match &e.kind {
        ExprKind::Call { name, args } if name == "join" && args.len() == 2 => {
            matches!(&args[0].value.kind, ExprKind::Method { name, .. } if name == "map")
        }
        ExprKind::Bin { op: "+", l, r } => fusable(l) || fusable(r) || conv(l) || conv(r),
        _ => false,
    }
}
// `to_string(x)` albo `char(x)`: w buforze bez pośredniego tekstu.
fn conv(e: &Expr) -> bool {
    matches!(&e.kind, ExprKind::Call { name, args } if (name == "to_string" || name == "char") && args.len() == 1)
}

// Ciało kończy się return na każdej ścieżce.
fn always_returns(ss: &[Stmt]) -> bool {
    match ss.last() {
        Some(Stmt::Return { .. }) => true,
        Some(Stmt::If { then, els: Some(e), .. }) => always_returns(then) && always_returns(e),
        Some(Stmt::Match { arms, .. }) => !arms.is_empty() && arms.iter().all(|a| always_returns(&a.body)),
        _ => false,
    }
}

// Warunek typu, który zależy tylko od sprawdzanej wartości i funkcji wbudowanych.
fn simple(e: &Expr, param: &str) -> bool {
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Dec(_) | ExprKind::Str(_) | ExprKind::Bool(_) => true,
        ExprKind::Ident(n) => n == param,
        ExprKind::List(xs) => xs.iter().all(|x| simple(x, param)),
        ExprKind::Call { name, args } => builtin_fn(name).is_some() && args.iter().all(|a| simple(&a.value, param)),
        ExprKind::Field { obj, .. } => simple(obj, param),
        ExprKind::Bin { l, r, .. } => simple(l, param) && simple(r, param),
        ExprKind::Not(x) | ExprKind::Neg(x) => simple(x, param),
        _ => false,
    }
}

impl<'a> Gen<'a> {
    pub fn new(env: &'a Env<'a>) -> Gen<'a> {
        Gen {
            env,
            consts: String::new(),
            const_ids: HashMap::new(),
            moves: Moves::new(),
            recs: BTreeMap::new(),
            sigs: HashMap::new(),
            borrowed: HashMap::new(),
            ref_lets: HashSet::new(),
            checks: HashMap::new(),
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

    // ---------- typy w kompilacji ----------

    fn rt_of(&self, t: &TypeExpr) -> Rt {
        self.rt_of_d(t, 0)
    }

    fn rt_of_d(&self, t: &TypeExpr, depth: usize) -> Rt {
        if depth > 20 {
            return Rt::Dyn;
        }
        let TypeExpr::Name { name, args, cond, .. } = t else { return Rt::Dyn };
        if let Some(c) = cond {
            if !simple(&c.expr, &c.param) {
                return Rt::Dyn;
            }
        }
        match name.as_str() {
            "Int" => Rt::Int,
            "String" => Rt::Str,
            "Bool" => Rt::Bool,
            "List" => match args.first() {
                None => Rt::List(Box::new(Rt::Dyn)),
                Some(a) => {
                    let e = self.rt_of_d(a, depth + 1);
                    if e.dynamic() { Rt::Dyn } else { Rt::List(Box::new(e)) }
                }
            },
            n if CAPS.contains(&n) => {
                if cond.is_some() { Rt::Dyn } else { Rt::Cap(n.to_string()) }
            }
            n if self.recs.contains_key(n) => Rt::Rec(n.to_string()),
            n => match self.env.types.get(n) {
                Some((
                    TypeDecl {
                        body: TypeBody::Rhs(terms), ..
                    },
                    _,
                )) if terms.len() == 1 && matches!(&terms[0], TypeExpr::Name { name, .. } if self.env.is_type(name)) => {
                    self.rt_of_d(&terms[0], depth + 1)
                }
                _ => Rt::Dyn,
            },
        }
    }

    fn field_rt(&self, rec: &str, f: &str) -> Option<(Rt, &'a TypeExpr)> {
        self.recs.get(rec)?.iter().find(|(n, _, _)| n == f).map(|(_, te, rt)| (rt.clone(), *te))
    }

    // Rekordy, sygnatury funkcji i funkcje używane jako fake (te zostają dynamiczne: fn(V) -> R).
    fn prepare(&mut self, project: &Project) {
        let env = self.env;
        let mut fakes: HashSet<String> = HashSet::new();
        for entries in project.toml.values() {
            for e in entries {
                if let Some(toml::Value::Str(f)) = e.value.get("fake") {
                    fakes.insert(f.clone());
                }
            }
        }
        // Rekord, który zawiera sam siebie wprost (nie przez listę), zostaje dynamiczny.
        let mut cand: BTreeMap<String, &'a Vec<Field>> = BTreeMap::new();
        for (name, (t, _)) in &env.types {
            if let TypeBody::Record(fs) = &t.body {
                cand.insert(name.clone(), fs);
            }
        }
        loop {
            self.recs = cand
                .iter()
                .map(|(n, fs)| (n.clone(), fs.iter().map(|f| (f.name.clone(), &f.ty, Rt::Dyn)).collect()))
                .collect();
            let rts: BTreeMap<String, Vec<Rt>> =
                cand.iter().map(|(n, fs)| (n.clone(), fs.iter().map(|f| self.rt_of(&f.ty)).collect())).collect();
            let direct = |n: &str| -> Vec<String> {
                rts[n].iter().filter_map(|r| if let Rt::Rec(m) = r { Some(m.clone()) } else { None }).collect()
            };
            let mut cyclic = vec![];
            for n in cand.keys() {
                let mut stack = direct(n);
                let mut seen = HashSet::new();
                while let Some(m) = stack.pop() {
                    if &m == n {
                        cyclic.push(n.clone());
                        break;
                    }
                    if seen.insert(m.clone()) {
                        stack.extend(direct(&m));
                    }
                }
            }
            if cyclic.is_empty() {
                for (n, fs) in self.recs.iter_mut() {
                    for (f, r) in fs.iter_mut().zip(&rts[n]) {
                        f.2 = r.clone();
                    }
                }
                break;
            }
            for n in cyclic {
                cand.remove(&n);
            }
        }
        for (name, info) in &env.fns {
            let d = info.decl();
            let sig = if fakes.contains(name) || info.body().is_none() {
                (vec![Rt::Dyn; d.params.len()], Rt::Dyn)
            } else {
                let ps = d.params.iter().map(|p| self.rt_of(&p.ty)).collect();
                let ret = match (&d.ret, info.body()) {
                    (Some(r), Some(b)) if always_returns(b) => self.rt_of(r),
                    _ => Rt::Dyn,
                };
                (ps, ret)
            };
            if let Some(body) = info.body() {
                Live { outer: &HashSet::new(), moves: &mut self.moves }.body(body);
                self.ref_lets.extend(ref_lets(body, &self.moves));
                let mut set = HashSet::new();
                assigned(body, &mut set);
                let b = d
                    .params
                    .iter()
                    .zip(&sig.0)
                    .map(|(p, rt)| {
                        !rt.dynamic() && !rt.copy() && !set.contains(&p.name) && !moved_any(body, &p.name, &self.moves)
                    })
                    .collect();
                self.borrowed.insert(name.clone(), b);
            }
            self.sigs.insert(name.clone(), sig);
        }
    }

    fn structs_rs(&mut self, out: &mut String) {
        for (n, fs) in &self.recs {
            let s = format!("S_{}", ident(n));
            let decl: Vec<String> = fs.iter().map(|(f, _, rt)| format!("f_{}: {}", ident(f), rt.rs())).collect();
            let tv: Vec<String> = fs.iter().map(|(f, _, _)| format!("({}, self.f_{}.tv())", q(f), ident(f))).collect();
            let fv: Vec<String> = fs.iter().map(|(f, _, _)| format!("f_{}: from_v(fget(&o, {}))?", ident(f), q(f))).collect();
            let eqv: Vec<String> = fs.iter().map(|(f, _, _)| format!("self.f_{0}.eqv(&o.f_{0})", ident(f))).collect();
            writeln!(
                out,
                "#[derive(Clone)]\nstruct {s} {{ {} }}\nimpl Val for {s} {{\n    fn tv(self) -> V {{ V::Rec(Rc::new(Obj {{ n: {n}, f: vec![{}] }})) }}\n    fn fv(v: V) -> Result<Self, Ctl> {{ match v {{ V::Rec(o) if o.n == {n} => Ok({s} {{ {} }}), v => bad(&v) }} }}\n    fn eqv(&self, o: &Self) -> bool {{ true{} }}\n}}\n",
                decl.join(", "),
                tv.join(", "),
                fv.join(", "),
                eqv.iter().map(|e| format!(" && {}", e)).collect::<String>(),
                n = q(n),
            )
            .unwrap();
        }
    }

    // Funkcja k_N sprawdzająca warunki typu t na wartości typu rt; None, gdy nie ma czego sprawdzać.
    fn check_fn(&mut self, t: &TypeExpr, rt: &Rt) -> Option<String> {
        self.check_fn_d(t, rt, 0)
    }

    fn check_fn_d(&mut self, t: &TypeExpr, rt: &Rt, depth: usize) -> Option<String> {
        if depth > 20 || rt.dynamic() {
            return None;
        }
        let key = format!("{}|{:?}", t, rt);
        if let Some(r) = self.checks.get(&key) {
            return r.clone();
        }
        let TypeExpr::Name { name, args, cond, .. } = t else { return None };
        let mut body = String::new();
        if name == "List" {
            if let (Some(a), Rt::List(et)) = (args.first(), rt) {
                if let Some(k) = self.check_fn_d(a, et, depth + 1) {
                    let _ = write!(body, "for e in x.iter() {{ if !{}(e)? {{ return Ok(false); }} }} ", k);
                }
            }
        } else if let Some((
            TypeDecl {
                body: TypeBody::Rhs(terms), ..
            },
            _,
        )) = self.env.types.get(name)
        {
            if terms.len() == 1 {
                if let Some(k) = self.check_fn_d(&terms[0], rt, depth + 1) {
                    let _ = write!(body, "if !{}(x)? {{ return Ok(false); }} ", k);
                }
            }
        }
        if let Some(c) = cond {
            let mut cx = Cx::new("", Ret::Plain);
            cx.ret_rt = Rt::Bool;
            cx.declare(&c.param, rt.clone(), 0);
            let (code, crt) = self.ex(&c.expr, &mut cx);
            let ok = if crt == Rt::Bool {
                code
            } else {
                format!("matches!({}, V::Bool(true))", coerce(code, &crt, &Rt::Dyn))
            };
            let _ = write!(body, "let {}: {} = x.clone(); if !({}) {{ return Ok(false); }} ", v(&c.param), rt.rs(), ok);
        }
        let r = if body.is_empty() {
            None
        } else {
            let id = self.checks.values().filter(|x| x.is_some()).count() + 1;
            writeln!(self.consts, "fn k_{}(x: &{}) -> Result<bool, Ctl> {{ {}Ok(true) }}", id, rt.rs(), body).unwrap();
            Some(format!("k_{}", id))
        };
        self.checks.insert(key, r.clone());
        r
    }

    // Wartość typu from jako wartość typu te (w kompilacji to), z tym samym błędem co conform.
    fn conform_to(&mut self, code: String, from: &Rt, te: &TypeExpr, to: &Rt, wh: &str) -> String {
        let t = self.ty_static(te);
        if to.dynamic() {
            return format!("conform(&{}, {}, &{})?", t, coerce(code, from, &Rt::Dyn), wh);
        }
        if compatible(from, to) {
            let c = coerce(code, from, to);
            return match self.check_fn(te, to) {
                None => c,
                Some(k) => format!(
                    "{{ let ck = {}; if !{}(&ck)? {{ return Err(fail_conform(to_v(ck), &{}, &{})); }} ck }}",
                    c, k, t, wh
                ),
            };
        }
        format!("from_v::<{}>(conform(&{}, {}, &{})?)?", to.rs(), t, coerce(code, from, &Rt::Dyn), wh)
    }

    // ---------- typy w runtime ----------

    fn ty_static(&mut self, t: &TypeExpr) -> String {
        let mut cx = Cx::new("", Ret::Plain);
        self.ty(t, &mut cx)
    }

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
                        cx.scopes.push(HashMap::new());
                        cx.declare(&c.param, Rt::Dyn, 0);
                        let body = self.exd(&c.expr, cx);
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

    // Generuje ciało, aż typy zmiennych przestaną się zmieniać. Zmienna, której typ zmienia się
    // po 8 przebiegach, dostaje V.
    fn infer(&mut self, mk: &dyn Fn() -> Cx, f: &mut dyn FnMut(&mut Self, &mut Cx) -> String) -> String {
        let mut hints: HashMap<usize, Rt> = HashMap::new();
        let mut pass = 0;
        loop {
            let mut cx = mk();
            cx.hints = hints.clone();
            let out = f(self, &mut cx);
            let mut next = hints.clone();
            for (k, t) in &cx.seen {
                let j = match hints.get(k) {
                    Some(h) => join(h, t),
                    None => t.clone(),
                };
                next.insert(*k, j);
            }
            if next == hints {
                return out;
            }
            pass += 1;
            if pass > 8 {
                for (k, t) in next.iter_mut() {
                    if hints.get(k) != Some(t) {
                        *t = Rt::Dyn;
                    }
                }
            }
            hints = next;
        }
    }

    fn fns_rs(&mut self, out: &mut String) {
        let env = self.env;
        for (name, info) in &env.fns {
            let Some(body) = info.body() else { continue };
            let (d, f) = info.imp.unwrap();
            let (prm, ret) = self.sigs[name].clone();
            let brw = self.borrowed[name].clone();
            let params: Vec<String> = d
                .params
                .iter()
                .zip(&prm)
                .zip(&brw)
                .map(|((p, rt), b)| if *b { format!("{}: &{}", v(&p.name), rt.rs()) } else { format!("mut {}: {}", v(&p.name), rt.rs()) })
                .collect();
            writeln!(out, "fn f_{}({}) -> Result<{}, Ctl> {{", ident(name), params.join(", "), ret.rs()).unwrap();
            let path = f.path.clone();
            let mk = || {
                let mut cx = Cx::new(&path, Ret::Conform);
                cx.fname = name.clone();
                cx.ret_rt = ret.clone();
                cx.ret_te = d.ret.clone();
                cx
            };
            let text = self.infer(&mk, &mut |g, cx| {
                let mut o = String::new();
                for ((p, rt), b) in d.params.iter().zip(&prm).zip(&brw) {
                    cx.declare(&p.name, rt.clone(), if *b { REF } else { 0 });
                    let wh = format!("Wh::S({})", q(&format!("{}: parametr {}", name, p.name)));
                    if rt.dynamic() {
                        let t = g.ty(&p.ty, cx);
                        writeln!(o, "    {} = conform(&{}, {}, &{})?;", v(&p.name), t, v(&p.name), wh).unwrap();
                    } else if let Some(k) = g.check_fn(&p.ty, rt) {
                        let t = g.ty_static(&p.ty);
                        writeln!(
                            o,
                            "    if !{}(&{})? {{ return Err(fail_conform(to_v({}.clone()), &{}, &{})); }}",
                            k,
                            v(&p.name),
                            v(&p.name),
                            t,
                            wh
                        )
                        .unwrap();
                    }
                }
                if ret.dynamic() {
                    let t = match &d.ret {
                        Some(r) => g.ty(r, cx),
                        None => "tany()".into(),
                    };
                    writeln!(o, "    let rt: T = {};", t).unwrap();
                    writeln!(o, "    let rtn: &str = {};", q(&format!("{}: wynik", name))).unwrap();
                }
                g.block(body, cx, &mut o, 1);
                if ret.dynamic() {
                    o.push_str("    Ok(V::Unit)\n");
                } else {
                    // Nieosiągalne: ciało zawsze kończy się return.
                    o.push_str("    Err(Ctl::Type(String::new()))\n");
                }
                o
            });
            out.push_str(&text);
            out.push_str("}\n\n");
        }
    }

    // Cały program testów bez runtime.
    pub fn tests_program(&mut self, project: &Project, test_res: &[String]) -> String {
        self.prepare(project);
        let mut out = String::new();
        self.structs_rs(&mut out);
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
                    n += 1;
                    let body = self.test_fn(&f.path, &ex.stmts, test_res, &[]);
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
                    let ps: Vec<&Param> = d.params.iter().filter(|p| !crate::codegen::is_cap(&p.ty)).collect();
                    let mut gens = vec![];
                    let mut cx = Cx::new(&f.path, Ret::Test);
                    for p in &ps {
                        cx.declare(&p.name, Rt::Dyn, 0);
                        let t = self.ty(&p.ty, &mut cx);
                        gens.push(format!("({}, {})", q(&p.name), t));
                    }
                    let stmts = vec![Stmt::Expr {
                        expr: pr.expr.clone(),
                        line: pr.line,
                    }];
                    n += 1;
                    let body = self.test_fn(&f.path, &stmts, test_res, &ps);
                    writeln!(out, "fn g_{}() -> Vec<(&'static str, T)> {{ vec![{}] }}", n, gens.join(", ")).unwrap();
                    writeln!(out, "fn t_{}(r: &Res, p: &[V]) -> R {{\n{}    Ok(V::Unit)\n}}\n", n, body).unwrap();
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
                n += 1;
                let body = self.test_fn(&path, &stmts, test_res, &[]);
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
        out.push_str(&spec_rs(project, "resources.test", &[]));
        out.push_str("\nfn main() {\n    init_types();\n    run_tests(tests(), spec());\n}\n\n// ---- stałe ----\n");
        out.push_str(&self.consts);
        out
    }

    // Program dla `sowa run --rust`: main dostaje zasoby z [resources] w kolejności parametrów.
    pub fn main_program(&mut self, project: &Project, fakes: &[String]) -> String {
        self.prepare(project);
        let mut out = String::new();
        self.structs_rs(&mut out);
        self.types_rs(&mut out);
        self.fns_rs(&mut out);
        let names = crate::check::resource_names(project, "resources");
        let args: Vec<String> = names.iter().map(|n| format!("res(r, {})", q(n))).collect();
        out.push_str(&spec_rs(project, "resources", fakes));
        writeln!(
            out,
            "\nfn main() {{\n    init_types();\n    run_main(|r| f_main({}).map(to_v), spec());\n}}\n\n// ---- stałe ----",
            args.join(", ")
        )
        .unwrap();
        out.push_str(&self.consts);
        out
    }

    // Ciało testu: zasoby, parametry property i instrukcje.
    fn test_fn(&mut self, file: &str, stmts: &[Stmt], test_res: &[String], params: &[&Param]) -> String {
        let mk = || Cx::new(file, Ret::Test);
        self.infer(&mk, &mut |g, cx| {
            let mut out = String::new();
            for (i, p) in params.iter().enumerate() {
                cx.declare(&p.name, Rt::Dyn, 0);
                writeln!(out, "    let mut {} = p[{}].clone();", v(&p.name), i).unwrap();
            }
            out.push_str(&g.test_body(stmts, cx, test_res));
            out
        })
    }

    fn test_body(&mut self, stmts: &[Stmt], cx: &mut Cx, test_res: &[String]) -> String {
        let mut out = String::new();
        for r in test_res {
            if !cx.has(r) {
                cx.declare(r, Rt::Dyn, 0);
                writeln!(out, "    let mut {} = res(r, {});", v(r), q(r)).unwrap();
            }
        }
        for s in stmts {
            match s {
                Stmt::Expr { expr, .. } => {
                    let src = q(&expr.to_string());
                    match &expr.kind {
                        ExprKind::Bin { op: "==", l, r } => {
                            let a = self.exd(l, cx);
                            let b = self.exd(r, cx);
                            writeln!(out, "    check_eq({}, {}, {})?;", a, b, src).unwrap();
                        }
                        ExprKind::Is { e, ty, neg } => {
                            let a = self.exd(e, cx);
                            let t = self.ty(ty, cx);
                            writeln!(out, "    check_is({}, &{}, {}, {})?;", a, t, neg, src).unwrap();
                        }
                        _ => {
                            let a = self.exd(expr, cx);
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
        cx.scopes.push(HashMap::new());
        for s in stmts {
            self.stmt(s, cx, out, ind);
        }
        cx.scopes.pop();
    }

    // Wyjście z funkcji z wartością: return, `try` i `or return`.
    fn ret_code(&mut self, cx: &mut Cx, code: String, rt: &Rt, err: bool) -> String {
        match cx.ret {
            Ret::Conform if cx.ret_rt.dynamic() => {
                format!("return conform(&rt, {}, &Wh::S(rtn));", coerce(code, rt, &Rt::Dyn))
            }
            Ret::Conform => {
                let wh = format!("Wh::S({})", q(&format!("{}: wynik", cx.fname)));
                let te = cx.ret_te.clone().unwrap();
                let to = cx.ret_rt.clone();
                format!("return Ok({});", self.conform_to(code, rt, &te, &to, &wh))
            }
            Ret::Plain => {
                cx.rets.push(rt.clone());
                format!("return Ok({});", coerce(code, rt, &cx.ret_rt))
            }
            Ret::Keep => format!("return Ok(Some({}));", coerce(code, rt, &cx.ret_rt)),
            Ret::Test if !err => format!("return Ok({});", coerce(code, rt, &Rt::Dyn)),
            Ret::Test | Ret::TestThrow => format!("return Err(Ctl::Ret({}));", coerce(code, rt, &Rt::Dyn)),
        }
    }

    fn stmt(&mut self, s: &Stmt, cx: &mut Cx, out: &mut String, ind: usize) {
        let pad = "    ".repeat(ind);
        match s {
            Stmt::Set { name, is_var, expr, .. } => {
                let (e, et) = self.ex(expr, cx);
                match cx.get(name) {
                    Some((vt, key)) if !*is_var => {
                        cx.observe(key, &et);
                        writeln!(out, "{}{} = {};", pad, v(name), coerce(e, &et, &vt)).unwrap();
                    }
                    None if self.ref_lets.contains(&stmt_key(s)) && !et.dynamic() && !et.copy() && e.starts_with("at_(") => {
                        writeln!(out, "{}let {}: &{} = {};", pad, v(name), et.rs(), ref_arg(&e)).unwrap();
                        cx.declare(name, et, REF);
                    }
                    _ => {
                        let key = stmt_key(s);
                        let dt = cx.hints.get(&key).cloned().unwrap_or_else(|| et.clone());
                        cx.observe(key, &et);
                        writeln!(out, "{}let mut {}: {} = {};", pad, v(name), dt.rs(), coerce(e, &et, &dt)).unwrap();
                        cx.declare(name, dt, key);
                    }
                }
            }
            Stmt::Return { expr, .. } => {
                if let (Ret::Keep, Some(Expr { kind: ExprKind::Ident(n), .. })) = (cx.ret, expr) {
                    if cx.keep.as_ref() == Some(n) && cx.get(n).map(|x| x.0) == Some(cx.ret_rt.clone()) {
                        writeln!(out, "{}{{ std::mem::forget({}); return Ok(None); }}", pad, v(n)).unwrap();
                        return;
                    }
                }
                let (e, et) = match expr {
                    Some(e) => self.ex(e, cx),
                    None => ("V::Unit".into(), Rt::Dyn),
                };
                let r = self.ret_code(cx, e, &et, cx.ret == Ret::TestThrow);
                writeln!(out, "{}{}", pad, r).unwrap();
            }
            Stmt::Expr { expr, .. } => {
                let (e, _) = self.ex(expr, cx);
                writeln!(out, "{}{};", pad, e).unwrap();
            }
            Stmt::If { cond, then, els, .. } => {
                let (c, ct) = self.ex(cond, cx);
                writeln!(out, "{}if {} {{", pad, bool_code(c, &ct)).unwrap();
                self.block(then, cx, out, ind + 1);
                if let Some(e) = els {
                    writeln!(out, "{}}} else {{", pad).unwrap();
                    self.block(e, cx, out, ind + 1);
                }
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::While { cond, body, .. } => {
                let (c, ct) = self.ex(cond, cx);
                writeln!(out, "{}while {} {{", pad, bool_code(c, &ct)).unwrap();
                self.block(body, cx, out, ind + 1);
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::For { var, iter, body, .. } => {
                let (it, irt) = self.ex(iter, cx);
                let key = stmt_key(s);
                let t = cx.tmp();
                let et = match &irt {
                    Rt::List(e) => {
                        writeln!(out, "{}for {} in ({}).iter() {{", pad, t, it).unwrap();
                        (**e).clone()
                    }
                    _ => {
                        writeln!(out, "{}for {} in iter_({})?.iter() {{", pad, t, coerce(it, &irt, &Rt::Dyn)).unwrap();
                        Rt::Dyn
                    }
                };
                let dt = cx.hints.get(&key).cloned().unwrap_or_else(|| et.clone());
                cx.observe(key, &et);
                // Element, którego ciało pętli nie zmienia ani nie przenosi, jest czytany przez referencję.
                let mut set = HashSet::new();
                assigned(body, &mut set);
                let by_ref = dt == et && !et.dynamic() && !et.copy() && !set.contains(var) && !moved_any(body, var, &self.moves);
                if by_ref {
                    writeln!(out, "{}    let {}: &{} = {};", pad, v(var), et.rs(), t).unwrap();
                } else {
                    writeln!(out, "{}    let mut {}: {} = {};", pad, v(var), dt.rs(), coerce(format!("{}.clone()", t), &et, &dt))
                        .unwrap();
                }
                cx.scopes.push(HashMap::new());
                cx.declare(var, dt, if by_ref { REF } else { key });
                self.block(body, cx, out, ind + 1);
                cx.scopes.pop();
                writeln!(out, "{}}}", pad).unwrap();
            }
            Stmt::Match { subjects, arms, line } => {
                let mut subs = vec![];
                writeln!(out, "{}{{", pad).unwrap();
                for sub in subjects {
                    let t = cx.tmp();
                    let e = self.exd(sub, cx);
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
                    cx.scopes.push(HashMap::new());
                    for (n, acc) in &binds {
                        writeln!(out, "{}        let mut {} = {}.clone();", pad, v(n), acc).unwrap();
                        cx.declare(n, Rt::Dyn, 0);
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

    // Wyrażenie jako V.
    fn exd(&mut self, e: &Expr, cx: &mut Cx) -> String {
        let (c, t) = self.ex(e, cx);
        coerce(c, &t, &Rt::Dyn)
    }

    fn ex(&mut self, e: &Expr, cx: &mut Cx) -> (String, Rt) {
        if fusable(e) {
            let tmp = cx.tmp;
            let b = cx.tmp();
            if let Some(s) = self.sink(e, &b, cx) {
                return (format!("{{ let mut {b} = sbuf(); {s}sdone({b}) }}"), Rt::Str);
            }
            cx.tmp = tmp;
        }
        self.ex_plain(e, cx)
    }

    // Tekst dopisywany do bufora buf zamiast składania pośrednich list i tekstów: `a + b`,
    // `join(xs, sep)` i `join(xs.map(p => tekst), sep)`. None, gdy któraś część nie jest tekstem
    // znanym w kompilacji; wtedy nic nie zostaje w cx poza licznikiem zmiennych tymczasowych.
    fn sink(&mut self, e: &Expr, buf: &str, cx: &mut Cx) -> Option<String> {
        let tmp = cx.tmp;
        match &e.kind {
            ExprKind::Str(s) => return Some(format!("{}.push_str({}); ", buf, q(s))),
            ExprKind::Call { name, args }
                if conv(e) && args[0].name.is_none() && !self.env.fns.contains_key(name.as_str()) =>
            {
                if let (true, ExprKind::Int(n)) = (name == "char", &args[0].value.kind) {
                    if let Some(c) = u32::try_from(*n).ok().and_then(char::from_u32) {
                        return Some(format!("{}.push_str({}); ", buf, q(&c.to_string())));
                    }
                }
                let (c, t) = self.ex(&args[0].value, cx);
                if t == Rt::Int {
                    let f = if name == "char" { "push_char" } else { "push_int" };
                    let tail = if name == "char" { "?" } else { "" };
                    return Some(format!("{}(&mut {}, {}){}; ", f, buf, c, tail));
                }
                cx.tmp = tmp;
            }
            ExprKind::Bin { op: "+", l, r } => {
                if let Some(a) = self.sink(l, buf, cx) {
                    if let Some(b) = self.sink(r, buf, cx) {
                        return Some(a + &b);
                    }
                }
                cx.tmp = tmp;
            }
            ExprKind::Call { name, args } if name == "join" && args.len() == 2 && args.iter().all(|a| a.name.is_none()) => {
                if let ExprKind::Str(sep) = &args[1].value.kind {
                    let (list, each) = match &args[0].value.kind {
                        ExprKind::Method { obj, name: m, targs, args: margs }
                            if m == "map" && targs.is_empty() && margs.len() == 1 && margs[0].name.is_none() =>
                        {
                            match &margs[0].value.kind {
                                ExprKind::Lambda { param, body: LambdaBody::Expr(body) } => (&**obj, Some((param, &**body))),
                                _ => (&args[0].value, None),
                            }
                        }
                        _ => (&args[0].value, None),
                    };
                    let (o, ot) = self.ex(list, cx);
                    let t = cx.tmp();
                    let inner = match (&ot, each) {
                        (Rt::List(et), Some((param, body))) if !et.dynamic() => {
                            // Element przez referencję.
                            let by_ref = !et.copy();
                            cx.scopes.push(HashMap::new());
                            cx.declare(param, (**et).clone(), if by_ref { REF } else { 0 });
                            let inner = self.sink(body, buf, cx);
                            cx.scopes.pop();
                            inner.map(|i| {
                                if by_ref {
                                    format!("let {}: &{} = {}; {}", v(param), et.rs(), t, i)
                                } else {
                                    format!("let mut {}: {} = {}.clone(); {}", v(param), et.rs(), t, i)
                                }
                            })
                        }
                        (Rt::List(et), None) if **et == Rt::Str => Some(format!("push_s(&mut {}, {}); ", buf, t)),
                        _ => None,
                    };
                    if let Some(inner) = inner {
                        let src = by_ref(&o);
                        return Some(if sep.is_empty() {
                            format!("for {} in ({}).iter() {{ {}}} ", t, src, inner)
                        } else {
                            let first = cx.tmp();
                            format!(
                                "{{ let mut {f} = true; for {t} in ({src}).iter() {{ if !{f} {{ {buf}.push_str({sep}); }} {f} = false; {inner}}} }} ",
                                f = first,
                                sep = q(sep)
                            )
                        });
                    }
                    cx.tmp = tmp;
                }
            }
            _ => {}
        }
        let (c, t) = if fusable(e) { self.ex_plain(e, cx) } else { self.ex(e, cx) };
        if t == Rt::Str {
            Some(format!("push_s(&mut {}, {}); ", buf, str_ref(e, &c)))
        } else {
            cx.tmp = tmp;
            None
        }
    }

    fn ex_plain(&mut self, e: &Expr, cx: &mut Cx) -> (String, Rt) {
        match &e.kind {
            ExprKind::Int(n) => (format!("{}i64", n), Rt::Int),
            ExprKind::Dec(s) => (self.konst("V", format!("dec_lit({})", q(s))), Rt::Dyn),
            ExprKind::Str(s) => (self.konst("Rc<str>", format!("Rc::from({})", q(s))), Rt::Str),
            ExprKind::Bool(b) => (format!("{}", b), Rt::Bool),
            ExprKind::Html(segs) => {
                let h = format!("h{}", cx.tmp());
                let mut out = format!("{{ let mut {} = String::new(); ", h);
                for seg in segs {
                    match seg {
                        HtmlSeg::Lit(t) => {
                            let _ = write!(out, "{}.push_str({}); ", h, q(t));
                        }
                        HtmlSeg::Expr(x) => {
                            let x = self.exd(x, cx);
                            let _ = write!(out, "html_part(&mut {}, &{})?; ", h, x);
                        }
                    }
                }
                let _ = write!(out, "V::Html(Rc::from({})) }}", h);
                (out, Rt::Dyn)
            }
            ExprKind::Ident(n) => match cx.get(n) {
                Some((rt, k)) => {
                    for f in cx.caps.iter_mut() {
                        f.insert(n.clone());
                    }
                    let code = if k == REF {
                        if rt.copy() { format!("(*{})", v(n)) } else { format!("(*{}).clone()", v(n)) }
                    } else if rt.copy() {
                        v(n)
                    } else if self.moves.contains(&key(e)) {
                        if rt.dynamic() { format!("mv(&mut {})", v(n)) } else { v(n) }
                    } else {
                        format!("{}.clone()", v(n))
                    };
                    (code, rt)
                }
                None if self.env.variants.contains_key(n) => (self.konst("V", format!("vsing({})", q(n))), Rt::Dyn),
                None => ("V::Unit".into(), Rt::Dyn),
            },
            ExprKind::List(xs) => {
                if xs.is_empty() {
                    return (EMPTY.into(), Rt::List(Box::new(Rt::Never)));
                }
                let (pre, a) = self.items(&xs.iter().collect::<Vec<_>>(), false, cx);
                let t = a.iter().fold(Rt::Never, |acc, (_, t)| join(&acc, t));
                let t = if t == Rt::Never { Rt::Dyn } else { t };
                let parts: Vec<String> = a.into_iter().map(|(c, rt)| coerce(c, &rt, &t)).collect();
                (wrap(pre, format!("Rc::new(vec![{}])", parts.join(", "))), Rt::List(Box::new(t)))
            }
            ExprKind::Call { name, args } => self.call(name, args, e.line, cx),
            ExprKind::Method { obj, name, targs, args } => self.method(obj, name, targs, args, e.line, cx),
            ExprKind::Field { obj, name } => {
                if let ExprKind::Ident(n) = &obj.kind {
                    if let Some((rt, k)) = cx.get(n) {
                        for f in cx.caps.iter_mut() {
                            f.insert(n.clone());
                        }
                        if let Rt::Rec(r) = &rt {
                            if let Some((frt, _)) = self.field_rt(r, name) {
                                let p = format!("{}.f_{}", v(n), ident(name));
                                // Ostatni odczyt zmiennej wyjmuje pole: lista zostaje z jedną referencją.
                                let take = frt.copy() || (k != REF && self.moves.contains(&key(e)));
                                return (if take { p } else { format!("{}.clone()", p) }, frt);
                            }
                        }
                        if rt.dynamic() {
                            return (format!("field_ref(&{}, {})?", v(n), q(name)), Rt::Dyn);
                        }
                    }
                }
                let (o, rt) = self.ex(obj, cx);
                if let Rt::Rec(r) = &rt {
                    if let Some((frt, _)) = self.field_rt(r, name) {
                        return (format!("({}).f_{}", o, ident(name)), frt);
                    }
                }
                (format!("field({}, {})?", coerce(o, &rt, &Rt::Dyn), q(name)), Rt::Dyn)
            }
            ExprKind::Bin { op, l, r } => self.bin(op, l, r, cx),
            ExprKind::Not(x) => {
                let (c, t) = self.ex(x, cx);
                (format!("(!{})", bool_code(c, &t)), Rt::Bool)
            }
            ExprKind::Neg(x) => {
                let (c, t) = self.ex(x, cx);
                if t == Rt::Int {
                    (format!("(-({}))", c), Rt::Int)
                } else {
                    (format!("neg({})?", coerce(c, &t, &Rt::Dyn)), Rt::Dyn)
                }
            }
            ExprKind::Is { e: x, ty, neg } => {
                let a = self.exd(x, cx);
                let t = self.ty(ty, cx);
                (format!("{}is(&{}, &{})", if *neg { "!" } else { "" }, t, a), Rt::Bool)
            }
            ExprKind::As { e: x, ty, alt } => {
                let a = self.exd(x, cx);
                let t = self.ty(ty, cx);
                let src = q(&format!("{}", ty));
                let code = match alt.as_deref() {
                    None => format!("as_strict(&{}, {}, {})?", t, a, src),
                    Some(alt) => {
                        let tv = cx.tmp();
                        let body = match alt {
                            Alt::Value(x) => self.exd(x, cx),
                            Alt::Return(x) => {
                                let (r, rt) = match x {
                                    Some(x) => self.ex(x, cx),
                                    None => ("V::Unit".into(), Rt::Dyn),
                                };
                                format!("{{ {} }}", self.ret_code(cx, r, &rt, true))
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
                };
                (code, Rt::Dyn)
            }
            ExprKind::With { e: x, fields } => {
                // Pole z innej zmiennej (`text: frame.text`) czytane dopiero przy przypisaniu: gdy
                // to ta sama wartość, licznik referencji się nie zmienia (set_rc).
                let root = |e: &Expr| -> Option<String> {
                    match &e.kind {
                        ExprKind::Field { obj, .. } => match &obj.kind {
                            ExprKind::Ident(n) => Some(n.clone()),
                            _ => None,
                        },
                        _ => None,
                    }
                };
                let base = match &x.kind {
                    ExprKind::Ident(n) => Some(n.clone()),
                    _ => None,
                };
                let late: Vec<bool> = fields
                    .iter()
                    .enumerate()
                    .map(|(i, (_, val))| match root(val) {
                        Some(n) => {
                            Some(&n) != base.as_ref()
                                && !moved_in(x, &n, &self.moves)
                                && !fields.iter().enumerate().any(|(j, (_, w))| j != i && moved_in(w, &n, &self.moves))
                        }
                        None => false,
                    })
                    .collect();
                let items: Vec<&Expr> = std::iter::once(&**x)
                    .chain(fields.iter().zip(&late).filter(|(_, l)| !**l).map(|((_, val), _)| val))
                    .collect();
                let (pre, xs) = self.items(&items, false, cx);
                let mut xs = xs.into_iter();
                let (a, at) = xs.next().unwrap();
                let mut vals = vec![];
                for ((_, val), l) in fields.iter().zip(&late) {
                    vals.push(if *l { (self.ex(val, cx), true) } else { (xs.next().unwrap(), false) });
                }
                if let Rt::Rec(r) = &at {
                    let fs: Option<Vec<(Rt, &TypeExpr)>> = fields.iter().map(|(n, _)| self.field_rt(r, n)).collect();
                    if let Some(fs) = fs {
                        let t = cx.tmp();
                        let mut code = format!("{{ {}let mut {} = {}; ", pre, t, a);
                        for (((n, _), ((c, crt), l)), (frt, te)) in fields.iter().zip(vals).zip(fs) {
                            let wh = format!("Wh::Field({}, {})", q(r), q(n));
                            let rc = matches!(crt, Rt::Str | Rt::List(_)) && crt == frt;
                            if l && rc && clone_path(&c).is_some() && self.conform_to("x".into(), &crt, te, &frt, &wh) == "x" {
                                let _ = write!(code, "set_rc(&mut {}.f_{}, {}); ", t, ident(n), by_ref(&c));
                                continue;
                            }
                            let val = self.conform_to(c, &crt, te, &frt, &wh);
                            let _ = write!(code, "{}.f_{} = {}; ", t, ident(n), val);
                        }
                        let _ = write!(code, "{} }}", t);
                        return (code, at);
                    }
                }
                let a = coerce(a, &at, &Rt::Dyn);
                let fs: Vec<String> = fields
                    .iter()
                    .zip(vals)
                    .map(|((n, _), ((c, t), _))| format!("({}, {})", q(n), coerce(c, &t, &Rt::Dyn)))
                    .collect();
                (wrap(pre, format!("with({}, vec![{}])?", a, fs.join(", "))), Rt::Dyn)
            }
            ExprKind::Lambda { param, body } => {
                let (pre, clo) = self.lambda(param, body, &Rt::Dyn, Some(&Rt::Dyn), false, cx);
                (format!("{{ {}V::Fn(Rc::new({})) }}", pre, clo), Rt::Dyn)
            }
            ExprKind::Try(x) => {
                let ExprKind::Call { name, .. } = &x.kind else { return ("V::Unit".into(), Rt::Dyn) };
                let Some(info) = self.env.fns.get(name) else { return ("V::Unit".into(), Rt::Dyn) };
                let d = info.decl();
                let alts: Vec<TypeExpr> = d.ret.as_ref().map(|r| r.alts().into_iter().cloned().collect()).unwrap_or_default();
                if alts.len() < 2 {
                    return self.ex(x, cx);
                }
                let rest = TypeExpr::Union(alts[1..].to_vec());
                let t = self.ty(&rest, cx);
                let c = self.exd(x, cx);
                let tv = cx.tmp();
                let r = self.ret_code(cx, tv.clone(), &Rt::Dyn, true);
                (format!("{{ let {} = {}; if is(&{}, &{}) {{ {} }} {} }}", tv, c, t, tv, r, tv), Rt::Dyn)
            }
        }
    }

    // Lambda jako domknięcie: (klony z otoczenia, `move |..| ..`). prt to typ parametru; want to
    // wymuszony typ wyniku; by_ref, gdy parametr przychodzi przez referencję (filter); keep dla
    // map: gdy wynik ma typ parametru, lambda zwraca Option (None to element bez zmian, zob. lmap_same).
    // Zwraca też typ wyniku przez cx.rets w ostatnim elemencie.
    fn lambda(
        &mut self,
        param: &str,
        body: &LambdaBody,
        prt: &Rt,
        want: Option<&Rt>,
        by_ref: bool,
        cx: &mut Cx,
    ) -> (String, String) {
        let (pre, clo, _) = self.lambda_t(param, body, prt, want, by_ref, false, cx);
        (pre, clo)
    }

    fn lambda_t(
        &mut self,
        param: &str,
        body: &LambdaBody,
        prt: &Rt,
        want: Option<&Rt>,
        by_ref: bool,
        keep: bool,
        cx: &mut Cx,
    ) -> (String, String, Rt) {
        let mut outer = cx.names();
        outer.remove(param);
        let mut set = HashSet::new();
        let mut live = Live { outer: &outer, moves: &mut self.moves };
        match body {
            LambdaBody::Expr(x) => live.expr_body(x),
            LambdaBody::Block(stmts) => {
                live.body(stmts);
                assigned(stmts, &mut set);
                self.ref_lets.extend(ref_lets(stmts, &self.moves));
            }
        }
        // Zmienne-referencje trafiają do domknięcia jako kopie.
        let refs: Vec<(String, Rt)> = cx
            .scopes
            .iter()
            .flat_map(|s| s.iter())
            .filter(|(n, (_, k))| *k == REF && cx.get(n).map(|x| x.1) == Some(REF))
            .map(|(n, (rt, _))| (n.clone(), rt.clone()))
            .collect();
        cx.caps.push(HashSet::new());
        cx.scopes.push(HashMap::new());
        for (n, rt) in refs {
            cx.declare(&n, rt, 0);
        }
        cx.declare(param, prt.clone(), 0);
        let saved = (cx.ret, cx.ret_rt.clone(), cx.ret_te.take(), std::mem::take(&mut cx.rets), cx.keep.take());
        cx.ret = Ret::Plain;
        let kept;
        let (b, r) = match body {
            LambdaBody::Expr(x) => {
                let (c, t) = self.ex(x, cx);
                let r = match want {
                    Some(w) => w.clone(),
                    None if t.dynamic() => Rt::Dyn,
                    None => t.clone(),
                };
                kept = keep && r == *prt;
                match &x.kind {
                    ExprKind::Ident(n) if kept && n == param => (format!("{{ std::mem::forget({}); Ok(None) }}", v(param)), r),
                    _ if kept => (format!("Ok(Some({}))", coerce(c, &t, &r)), r),
                    _ => (format!("Ok({})", coerce(c, &t, &r)), r),
                }
            }
            LambdaBody::Block(stmts) => {
                let r = match want {
                    Some(w) => w.clone(),
                    None => {
                        // Pierwszy przebieg tylko zbiera typy wartości w return.
                        cx.ret_rt = Rt::Dyn;
                        cx.rets.clear();
                        let tmp = cx.tmp;
                        self.block(stmts, cx, &mut String::new(), 3);
                        cx.tmp = tmp;
                        let mut r = cx.rets.iter().fold(Rt::Never, |a, t| join(&a, t));
                        if !always_returns(stmts) {
                            r = Rt::Dyn;
                        }
                        if r.dynamic() { Rt::Dyn } else { r }
                    }
                };
                cx.ret_rt = r.clone();
                cx.rets.clear();
                kept = keep && r == *prt;
                if kept {
                    cx.ret = Ret::Keep;
                    cx.keep = if set.contains(param) { None } else { Some(param.to_string()) };
                }
                let mut b = String::from("\n");
                self.block(stmts, cx, &mut b, 3);
                let last = coerce("V::Unit".into(), &Rt::Dyn, &r);
                let last = if kept { format!("Some({})", last) } else { last };
                let _ = write!(b, "            Ok({})\n        ", last);
                (b, r)
            }
        };
        (cx.ret, cx.ret_rt, cx.ret_te, cx.rets, cx.keep) = saved;
        cx.scopes.pop();
        let caps = cx.pop_caps();
        // Klon na wejściu, żeby `var` z otoczenia dało się zmienić w lambdzie (jak kopia).
        let inner: String =
            caps.iter().filter(|n| set.contains(*n)).map(|n| format!("let mut {} = {}.clone(); ", v(n), v(n))).collect();
        // Każde domknięcie ma jedno miejsce wywołania, więc wklejenie nic nie kosztuje.
        let res = if kept { format!("Option<{}>", r.rs()) } else { r.rs() };
        let clo = if by_ref {
            format!(
                "#[inline(always)] move |e_: &{}| -> Result<{}, Ctl> {{ let mut {}: {} = e_.clone(); {}{} }}",
                prt.rs(),
                res,
                v(param),
                prt.rs(),
                inner,
                b
            )
        } else {
            format!("#[inline(always)] move |mut {}: {}| -> Result<{}, Ctl> {{ {}{} }}", v(param), prt.rs(), res, inner, b)
        };
        (clones(&caps), clo, r)
    }

    fn bin(&mut self, op: &str, l: &Expr, r: &Expr, cx: &mut Cx) -> (String, Rt) {
        let (mut a, at) = self.ex(l, cx);
        // Lewa zmienna czytana przez referencję, a prawa strona ją przenosi: najpierw wartość.
        let mut pre = String::new();
        if let ExprKind::Ident(n) = &l.kind {
            if moved_in(r, n, &self.moves) {
                let t = cx.tmp();
                let _ = write!(pre, "let {} = {}; ", t, a);
                a = t;
            }
        }
        let (b, bt) = self.ex(r, cx);
        let dyn2 = |a: String, b: String| (coerce(a, &at, &Rt::Dyn), coerce(b, &bt, &Rt::Dyn));
        let (code, rt) = match op {
            "+" => match (&at, &bt) {
                (Rt::Str, Rt::Str) => {
                    let mut parts = vec![];
                    for (x, c) in [(l, &a), (r, &b)] {
                        match c.strip_prefix("scat(&[").and_then(|c| c.strip_suffix("])")) {
                            Some(inner) => parts.push(inner.to_string()),
                            None => parts.push(str_ref(x, c)),
                        }
                    }
                    (format!("scat(&[{}])", parts.join(", ")), Rt::Str)
                }
                (Rt::List(x), Rt::List(y)) if x == y && matches!(r.kind, ExprKind::List(_)) && b.starts_with("Rc::new(vec![") => {
                    // `xs + [a, b]`: elementy wprost do xs, bez listy pośredniej.
                    let items = &b["Rc::new(".len()..b.len() - 1];
                    (format!("lext({}, {})", a, items), at.clone())
                }
                (Rt::List(x), Rt::List(y)) => {
                    let t = Rt::List(Box::new(join(x, y)));
                    (format!("lcat({}, {})", coerce(a, &at, &t), coerce(b, &bt, &t)), t)
                }
                (Rt::Int, Rt::Int) => (format!("iadd({}, {})?", a, b), Rt::Int),
                _ => {
                    let (a, b) = dyn2(a, b);
                    (format!("add({}, {})?", a, b), Rt::Dyn)
                }
            },
            "-" | "*" | "/" | "%" => {
                let (fi, fd) = match op {
                    "-" => ("isub", "sub"),
                    "*" => ("imul", "mul"),
                    "/" => ("idiv", "div"),
                    _ => ("irem", "rem"),
                };
                if at == Rt::Int && bt == Rt::Int {
                    (format!("{}({}, {})?", fi, a, b), Rt::Int)
                } else {
                    let (a, b) = dyn2(a, b);
                    (format!("{}({}, {})?", fd, a, b), Rt::Dyn)
                }
            }
            "==" | "!=" => {
                let neg = if op == "!=" { "!" } else { "" };
                let code = match (&at, &bt) {
                    (Rt::Int, Rt::Int) | (Rt::Bool, Rt::Bool) => format!("({} {} {})", a, op, b),
                    (Rt::Str, Rt::Str) => format!("({} {} {})", str_ref(l, &a), op, str_ref(r, &b)),
                    (x, y) if x == y && !x.dynamic() => format!("{}{}.eqv({})", neg, by_ref(&a), by_ref(&b)),
                    _ => {
                        let (a, b) = dyn2(a, b);
                        format!("{}eq(&{}, &{})", neg, a, b)
                    }
                };
                (code, Rt::Bool)
            }
            "<" | "<=" | ">" | ">=" => {
                let code = match (&at, &bt) {
                    (Rt::Int, Rt::Int) => format!("({} {} {})", a, op, b),
                    (Rt::Str, Rt::Str) => format!("({} {} {})", str_ref(l, &a), op, str_ref(r, &b)),
                    _ => {
                        let (a, b) = dyn2(a, b);
                        format!("(cmp(&{}, &{})? {} 0)", a, b, op)
                    }
                };
                (code, Rt::Bool)
            }
            "&&" | "||" => (format!("({} {} {})", bool_code(a, &at), op, bool_code(b, &bt)), Rt::Bool),
            _ => unreachable!(),
        };
        (wrap(pre, code), rt)
    }

    // Lista argumentów. Gdy któraś zmienna jest przenoszona albo argumenty nazwane zmieniają
    // kolejność, wartości trafiają do zmiennych tymczasowych w kolejności z args_order,
    // tej samej, którą przyjmuje analiza przeniesień.
    fn items(&mut self, items: &[&Expr], reorder: bool, cx: &mut Cx) -> (String, Vec<(String, Rt)>) {
        self.items_ref(items, reorder, &[], cx)
    }
    // refs[i]: argument idzie do parametru przez referencję, więc zmienna tymczasowa trzyma `&T`.
    fn items_ref(&mut self, items: &[&Expr], reorder: bool, refs: &[bool], cx: &mut Cx) -> (String, Vec<(String, Rt)>) {
        // Rust liczy argumenty po kolei, więc zmienne tymczasowe są potrzebne tylko wtedy, gdy
        // przenoszona zmienna stoi przed argumentem, który nie jest zmienną.
        let is_var = |x: &Expr| matches!(x.kind, ExprKind::Ident(_));
        let moved = items.iter().enumerate().any(|(i, x)| {
            is_var(x) && self.moves.contains(&key(x)) && items[i + 1..].iter().any(|y| !is_var(y))
        });
        if !(moved || reorder && items.len() > 1) {
            return (String::new(), items.iter().map(|x| self.ex(x, cx)).collect());
        }
        let mut pre = String::new();
        let mut out = vec![(String::new(), Rt::Dyn); items.len()];
        for x in args_order(items) {
            let i = items.iter().position(|y| std::ptr::eq(*y, x)).unwrap();
            let t = cx.tmp();
            let (c, rt) = self.ex(x, cx);
            if refs.get(i) == Some(&true) && !rt.dynamic() {
                let _ = write!(pre, "let {} = {}; ", t, ref_arg(&c));
                out[i] = (format!("(*{})", t), rt);
                continue;
            }
            let _ = write!(pre, "let {} = {}; ", t, c);
            out[i] = (t, rt);
        }
        (pre, out)
    }

    // Zmienna z typem przekazywana przez referencję nie musi być przenoszona.
    fn unmove(&mut self, e: &Expr, cx: &Cx) {
        if let ExprKind::Ident(n) = &e.kind {
            if let Some((rt, _)) = cx.get(n) {
                if !rt.dynamic() {
                    self.moves.remove(&key(e));
                }
            }
        }
    }

    fn method(&mut self, obj: &Expr, name: &str, targs: &[TypeExpr], args: &[Arg], line: usize, cx: &mut Cx) -> (String, Rt) {
        let positional = targs.is_empty() && args.iter().all(|a| a.name.is_none());
        // map i filter z lambdą na liście z typem.
        if positional && args.len() == 1 && (name == "map" || name == "filter") {
            if let ExprKind::Lambda { param, body } = &args[0].value.kind {
                let tmp = cx.tmp;
                let (o, ot) = self.ex(obj, cx);
                if let Rt::List(et) = &ot {
                    if **et != Rt::Never {
                        let f = cx.tmp();
                        if name == "map" {
                            let (pre, clo, r) = self.lambda_t(param, body, et, None, false, true, cx);
                            let lm = if r == **et { "lmap_same" } else { "lmap" };
                            let code = format!("{{ let {} = {{ {}{} }}; {}({}, {})? }}", f, pre, clo, lm, o, f);
                            return (code, Rt::List(Box::new(r)));
                        }
                        let (pre, clo, _) = self.lambda_t(param, body, et, Some(&Rt::Bool), true, false, cx);
                        let code = format!("{{ let {} = {{ {}{} }}; lfilter({}, {})? }}", f, pre, clo, o, f);
                        return (code, ot.clone());
                    }
                }
                cx.tmp = tmp;
            }
        }
        if positional && name == "write" && args.len() == 1 && fusable(&args[0].value) && matches!(obj.kind, ExprKind::Ident(_)) {
            let tmp = cx.tmp;
            let b = cx.tmp();
            let caps = expr_names(&args[0].value).iter().any(|n| matches!(cx.get(n), Some((Rt::Cap(_), _))));
            if let Some(s) = self.sink(&args[0].value, &b, cx) {
                let (o, ot) = self.ex(obj, cx);
                if ot == Rt::Cap("Terminal".into()) {
                    if !caps {
                        let st = cx.tmp();
                        return (
                            format!(
                                "{{ let mut {b} = out_take(); let {st} = {b}.len(); let ok = (|| -> Result<(), Ctl> {{ {s}Ok(()) }})(); term_put({}, {b}, {st}, ok, {})? }}",
                                by_ref(&o),
                                line
                            ),
                            Rt::Dyn,
                        );
                    }
                    return (
                        format!("{{ let mut {b} = String::new(); {s}term_write({}, &{b}, {})? }}", by_ref(&o), line),
                        Rt::Dyn,
                    );
                }
            }
            cx.tmp = tmp;
        }
        let items: Vec<&Expr> = std::iter::once(obj).chain(args.iter().map(|a| &a.value)).collect();
        let reorder = args.iter().any(|a| a.name.is_some());
        let (pre, mut xs) = self.items(&items, reorder, cx);
        let (o, ot) = xs.remove(0);
        if positional {
            let tys: Vec<&Rt> = xs.iter().map(|(_, t)| t).collect();
            let fast = match (&ot, name, tys.as_slice()) {
                (Rt::List(_), "reverse", []) => Some((format!("lrev({})", o), ot.clone())),
                (Rt::Cap(c), "int", [Rt::Int, Rt::Int]) if c == "Random" => {
                    Some((format!("rnd_int({}, {}, {}, {})?", by_ref(&o), xs[0].0, xs[1].0, line), Rt::Int))
                }
                (Rt::Cap(c), "choice", [Rt::List(t)]) if c == "Random" => {
                    let t = if t.dynamic() { Rt::Dyn } else { (**t).clone() };
                    Some((format!("rnd_choice({}, {}, {})?", by_ref(&o), by_ref(&xs[0].0), line), t))
                }
                (Rt::Cap(c), "write", [Rt::Str]) if c == "Terminal" => {
                    Some((format!("term_write({}, {}, {})?", by_ref(&o), by_ref(&xs[0].0), line), Rt::Dyn))
                }
                _ => None,
            };
            if let Some((code, rt)) = fast {
                return (wrap(pre, code), rt);
            }
        }
        let mut pos = vec![];
        let mut named = vec![];
        for (a, (c, t)) in args.iter().zip(xs) {
            let c = coerce(c, &t, &Rt::Dyn);
            match &a.name {
                Some(n) => named.push(format!("({}, {})", q(n), c)),
                None => pos.push(c),
            }
        }
        let ts: Vec<String> = targs.iter().map(|t| self.ty(t, cx)).collect();
        let code = format!(
            "call({}, {}, vec![{}], vec![{}], vec![{}], {})?",
            coerce(o, &ot, &Rt::Dyn),
            q(name),
            pos.join(", "),
            named.join(", "),
            ts.join(", "),
            line
        );
        let rt = match (&ot, name) {
            (Rt::Cap(c), "read") if c == "Terminal" => Rt::Str,
            _ => Rt::Dyn,
        };
        (wrap(pre, if rt.dynamic() { code } else { format!("from_v::<{}>({})?", rt.rs(), code) }), rt)
    }

    fn call(&mut self, name: &str, args: &[Arg], _line: usize, cx: &mut Cx) -> (String, Rt) {
        let env = self.env;
        if let Some(info) = env.fns.get(name) {
            let d = info.decl();
            let (prm, ret) = self.sigs.get(name).cloned().unwrap_or((vec![Rt::Dyn; d.params.len()], Rt::Dyn));
            let brw = self.borrowed.get(name).cloned().unwrap_or_default();
            let mut i = 0;
            let mut refs = vec![];
            for a in args {
                let k = match &a.name {
                    Some(n) => d.params.iter().position(|p| &p.name == n),
                    None => {
                        i += 1;
                        Some(i - 1)
                    }
                };
                let r = k.and_then(|k| brw.get(k)) == Some(&true);
                if r {
                    self.unmove(&a.value, cx);
                }
                refs.push(r);
            }
            let mut slots: Vec<Option<(String, Rt)>> = vec![None; d.params.len()];
            let mut i = 0;
            let items: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
            let (pre, xs) = self.items_ref(&items, args.iter().any(|a| a.name.is_some()), &refs, cx);
            for (a, x) in args.iter().zip(xs) {
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
            let mut a = vec![];
            for (k, ((s, p), pr)) in slots.into_iter().zip(&d.params).zip(&prm).enumerate() {
                let (c, t) = s.unwrap_or_else(|| ("V::Unit".into(), Rt::Dyn));
                let arg = if pr.dynamic() || compatible(&t, pr) {
                    coerce(c, &t, pr)
                } else {
                    let ty = self.ty_static(&p.ty);
                    let wh = q(&format!("{}: parametr {}", name, p.name));
                    format!("from_v::<{}>(conform(&{}, {}, &Wh::S({}))?)?", pr.rs(), ty, coerce(c, &t, &Rt::Dyn), wh)
                };
                a.push(if brw.get(k) == Some(&true) { ref_arg(&arg) } else { arg });
            }
            return (wrap(pre, format!("f_{}({})?", ident(name), a.join(", "))), ret);
        }
        if let Some(fs) = self.recs.get(name).cloned() {
            let exact = args.len() == fs.len()
                && fs.iter().all(|(f, _, _)| args.iter().filter(|a| a.name.as_deref() == Some(f.as_str())).count() == 1);
            if exact {
                let items: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
                let (pre, xs) = self.items(&items, true, cx);
                let mut parts = vec![];
                for (f, te, frt) in &fs {
                    let k = args.iter().position(|a| a.name.as_deref() == Some(f.as_str())).unwrap();
                    let (c, t) = xs[k].clone();
                    let wh = format!("Wh::Field({}, {})", q(name), q(f));
                    parts.push(format!("f_{}: {}", ident(f), self.conform_to(c, &t, te, frt, &wh)));
                }
                return (wrap(pre, format!("S_{} {{ {} }}", ident(name), parts.join(", "))), Rt::Rec(name.to_string()));
            }
        }
        if env.record_fields(name).is_some() || env.variant_fields(name).is_some() {
            let items: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
            let (pre, xs) = self.items(&items, false, cx);
            let fs: Vec<String> = args
                .iter()
                .zip(xs)
                .map(|(a, (c, t))| format!("({}, {})", q(&a.name.clone().unwrap_or_default()), coerce(c, &t, &Rt::Dyn)))
                .collect();
            let f = if env.record_fields(name).is_some() { "mk" } else { "mkv" };
            return (wrap(pre, format!("{}({}, vec![{}])?", f, q(name), fs.join(", "))), Rt::Dyn);
        }
        if let Some((arity, _)) = builtin_fn(name) {
            let items: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
            let (pre, xs) = self.items(&items, false, cx);
            if xs.len() == arity {
                let tys: Vec<&Rt> = xs.iter().map(|(_, t)| t).collect();
                let c = |k: usize| xs[k].0.clone();
                let s = |k: usize| str_ref(items[k], &xs[k].0);
                let fast = match (name, tys.as_slice()) {
                    ("len", [Rt::Str]) => Some((format!("slen({})", s(0)), Rt::Int)),
                    ("len", [Rt::List(_)]) => Some((format!("(({}).len() as i64)", by_ref(&c(0))), Rt::Int)),
                    ("at", [Rt::List(t), Rt::Int]) if !t.dynamic() => {
                        Some((format!("at_({}, {})?", by_ref(&c(0)), c(1)), (**t).clone()))
                    }
                    ("join", [Rt::List(t), Rt::Str]) if **t == Rt::Str => {
                        Some((format!("join_({}, {})", by_ref(&c(0)), s(1)), Rt::Str))
                    }
                    ("char", [Rt::Int]) => Some((format!("char_({})?", c(0)), Rt::Str)),
                    ("to_string", [Rt::Int]) => Some((format!("Rc::<str>::from({}.to_string())", c(0)), Rt::Str)),
                    ("contains", [Rt::Str, Rt::Str]) => Some((format!("str_contains({}, {})", s(0), s(1)), Rt::Bool)),
                    ("starts_with", [Rt::Str, Rt::Str]) => Some((format!("({}).starts_with({})", s(0), s(1)), Rt::Bool)),
                    ("trim", [Rt::Str]) => Some((format!("trim_({})", s(0)), Rt::Str)),
                    ("split", [Rt::Str, Rt::Str]) => {
                        Some((format!("split_({}, {})", s(0), s(1)), Rt::List(Box::new(Rt::Str))))
                    }
                    ("chars", [Rt::Str]) => Some((format!("chars_({})", s(0)), Rt::List(Box::new(Rt::Str)))),
                    _ => None,
                };
                if let Some((code, rt)) = fast {
                    return (wrap(pre, code), rt);
                }
            }
            let mut a: Vec<String> = xs.into_iter().map(|(c, t)| coerce(c, &t, &Rt::Dyn)).collect();
            a.truncate(arity);
            while a.len() < arity {
                a.push("V::Unit".into());
            }
            let rt = match name {
                "to_string" | "to_json" | "trim" | "lower" | "upper" | "remove" | "drop_prefix" | "pad_left" | "join"
                | "char" => Rt::Str,
                "len" => Rt::Int,
                "starts_with" | "contains" | "matches" | "only_digits" | "nip_checksum_ok" | "valid_email" => Rt::Bool,
                "segments" | "split" | "chars" => Rt::List(Box::new(Rt::Str)),
                _ => Rt::Dyn,
            };
            let code = format!("b_{}({})?", name, a.join(", "));
            let code = if rt.dynamic() { code } else { format!("from_v::<{}>({})?", rt.rs(), code) };
            return (wrap(pre, code), rt);
        }
        ("V::Unit".into(), Rt::Dyn)
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

// Zasoby z sekcji jako Spec dla runtime; zasoby z `fakes` biorą opis z [resources.test].
fn spec_rs(project: &Project, section: &str, fakes: &[String]) -> String {
    let entries = project.toml.get(section).cloned().unwrap_or_default();
    let test = project.toml.get("resources.test").cloned().unwrap_or_default();
    let mut items = vec![];
    for e in &entries {
        let e = match test.iter().find(|t| fakes.contains(&e.key) && t.key == e.key) {
            Some(t) => t,
            None => e,
        };
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
        let seed = match e.value.get("seed") {
            Some(toml::Value::Int(n)) => Some(n.to_string()),
            _ => get("seed"),
        };
        items.push(format!(
            "({}, Spec {{ ty: {}, url: {}, now: {}, fake: {}, seed: {}, seed_env: {} }})",
            q(&e.key),
            q(&get("type").unwrap_or_default()),
            opt(get("url")),
            opt(get("now")),
            fake,
            opt(seed),
            opt(get("seed_env"))
        ));
    }
    format!("fn spec() -> Vec<(&'static str, Spec)> {{\n    vec![{}]\n}}\n", items.join(", "))
}
