// Solver dla `sowa review`: czy stary warunek wynika z nowego.
//
// Warunek to wyrażenie logiczne nad α, jego polami, len(...) i parametrami funkcji. Porównania
// liniowe (+, -, mnożenie przez stałą) solver rozumie. Równość z wariantem albo tekstem
// (α.status == Issued) to przypisanie wartości. Reszta (valid_email(α), mnożenie dwóch
// zmiennych) to zdanie, o którym solver wie tylko tyle, że nie może być naraz prawdziwe i fałszywe.
//
// Pytanie „czy nowe ⇒ stare” solver zamienia na „czy nowe ∧ ¬stare ma rozwiązanie”. Formułę
// rozkłada na alternatywę koniunkcji (DNF), a każdą koniunkcję nierówności sprawdza eliminacją
// Fouriera–Motzkina na liczbach całkowitych. Money liczy się w groszach, więc też jest całkowite.
// Dec to liczba wymierna bez zaokrąglania (dla dowodów, gdzie Money nie musi być w groszach):
// wiersz z Dec zostaje ostry, a model z Dec daje Unknown zamiast kontrprzykładu.
// Rozwiązanie to kontrprzykład: wartość, którą nowy warunek przepuszcza, a stary nie.

use crate::ast::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, PartialEq)]
pub enum Verdict {
    // Nowy warunek jest taki sam albo ostrzejszy.
    Implied,
    // Nowy warunek przepuszcza wartość, której stary nie przepuszczał.
    Counter(String),
    // Solver nie umie rozstrzygnąć.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Num {
    Int,
    Money,
    Dec,
}

// ---------- ułamki ----------

#[derive(Debug, Clone, Copy, PartialEq)]
struct Rat(i128, i128);

fn gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Rat {
    fn int(n: i128) -> Rat {
        Rat(n, 1)
    }
    fn norm(n: i128, d: i128) -> Rat {
        let g = gcd(n, d).max(1);
        let s = if d < 0 { -1 } else { 1 };
        Rat(s * n / g, s * d / g)
    }
    fn add(self, o: Rat) -> Rat {
        Rat::norm(self.0 * o.1 + o.0 * self.1, self.1 * o.1)
    }
    fn mul(self, o: Rat) -> Rat {
        Rat::norm(self.0 * o.0, self.1 * o.1)
    }
    fn is_zero(self) -> bool {
        self.0 == 0
    }
    fn dec(s: &str) -> Option<Rat> {
        let (i, f) = s.split_once('.').unwrap_or((s, ""));
        let d = 10i128.checked_pow(f.len() as u32)?;
        let n: i128 = format!("{}{}", i, f).parse().ok()?;
        Some(Rat::norm(n, d))
    }
}

// ---------- literały ----------

// Suma współczynnik·zmienna + stała.
#[derive(Debug, Clone, PartialEq)]
struct Lin {
    terms: BTreeMap<String, Rat>,
    c: Rat,
}

impl Lin {
    fn konst(r: Rat) -> Lin {
        Lin { terms: BTreeMap::new(), c: r }
    }
    fn add(mut self, o: Lin, sign: i128) -> Lin {
        for (k, v) in o.terms {
            let e = self.terms.entry(k).or_insert(Rat::int(0));
            *e = e.add(v.mul(Rat::int(sign)));
        }
        self.terms.retain(|_, v| !v.is_zero());
        self.c = self.c.add(o.c.mul(Rat::int(sign)));
        self
    }
    fn scale(mut self, r: Rat) -> Lin {
        for v in self.terms.values_mut() {
            *v = v.mul(r);
        }
        self.terms.retain(|_, v| !v.is_zero());
        self.c = self.c.mul(r);
        self
    }
    fn as_const(&self) -> Option<Rat> {
        self.terms.is_empty().then_some(self.c)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Rel {
    Le,
    Lt,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq)]
enum Lit {
    // lin rel 0
    Cmp(Lin, Rel),
    // term == wartość (wariant albo tekst), pos = false znaczy !=
    Is(String, String, bool),
    // zdanie bez znaczenia dla solvera; arith: porównanie, którego solver nie umie policzyć
    Opaque(String, bool),
}

fn negate(l: Lit) -> Lit {
    match l {
        Lit::Cmp(e, Rel::Le) => Lit::Cmp(e.scale(Rat::int(-1)), Rel::Lt),
        Lit::Cmp(e, Rel::Lt) => Lit::Cmp(e.scale(Rat::int(-1)), Rel::Le),
        Lit::Cmp(e, Rel::Eq) => Lit::Cmp(e, Rel::Ne),
        Lit::Cmp(e, Rel::Ne) => Lit::Cmp(e, Rel::Eq),
        Lit::Is(t, v, p) => Lit::Is(t, v, !p),
        Lit::Opaque(k, p) => Lit::Opaque(k, !p),
    }
}

// Formuła po rozkładzie: alternatywa koniunkcji. None, gdy rozkład jest za duży.
type Dnf = Vec<Vec<Lit>>;
const MAX_CONJ: usize = 512;

fn and(a: Dnf, b: Dnf) -> Option<Dnf> {
    if a.len() * b.len() > MAX_CONJ {
        return None;
    }
    let mut out = vec![];
    for x in &a {
        for y in &b {
            let mut c = x.clone();
            c.extend(y.iter().cloned());
            out.push(c);
        }
    }
    Some(out)
}

struct Conv<'a> {
    num: &'a dyn Fn(&Expr) -> Option<Num>,
    dom: &'a dyn Fn(&Expr) -> Option<Vec<String>>,
    // Wszystkie możliwe warianty termu, gdy jego typ to unia wariantów bez danych.
    domains: BTreeMap<String, Vec<String>>,
    // Zmienne i ich rodzaj liczby, zebrane przy zamianie.
    vars: BTreeMap<String, Num>,
    // Zmienne len(...), które nie mogą być ujemne.
    nonneg: BTreeSet<String>,
}

impl Conv<'_> {
    fn is_term(e: &Expr) -> bool {
        match &e.kind {
            ExprKind::Ident(_) => true,
            ExprKind::Field { obj, .. } => Conv::is_term(obj),
            ExprKind::Call { name, args } => name == "len" && args.len() == 1,
            _ => false,
        }
    }

    fn lin(&mut self, e: &Expr) -> Option<Lin> {
        match &e.kind {
            ExprKind::Int(n) => Some(Lin::konst(Rat::int(*n as i128))),
            ExprKind::Dec(s) => Rat::dec(s).map(Lin::konst),
            ExprKind::Neg(x) => Some(self.lin(x)?.scale(Rat::int(-1))),
            ExprKind::Bin { op, l, r } if *op == "+" || *op == "-" => {
                let a = self.lin(l)?;
                let b = self.lin(r)?;
                Some(a.add(b, if *op == "+" { 1 } else { -1 }))
            }
            ExprKind::Bin { op: "*", l, r } => {
                let a = self.lin(l)?;
                let b = self.lin(r)?;
                match (a.as_const(), b.as_const()) {
                    (Some(k), _) => Some(b.scale(k)),
                    (_, Some(k)) => Some(a.scale(k)),
                    _ => None,
                }
            }
            _ if Conv::is_term(e) => {
                let n = if matches!(&e.kind, ExprKind::Call { .. }) { Some(Num::Int) } else { (self.num)(e) }?;
                let key = e.to_string();
                if n == Num::Int && matches!(&e.kind, ExprKind::Call { .. }) {
                    self.nonneg.insert(key.clone());
                }
                self.vars.insert(key.clone(), n);
                let mut terms = BTreeMap::new();
                terms.insert(key, Rat::int(1));
                Some(Lin { terms, c: Rat::int(0) })
            }
            _ => None,
        }
    }

    fn value_atom(e: &Expr) -> Option<String> {
        match &e.kind {
            ExprKind::Ident(n) if n.starts_with(|c: char| c.is_uppercase()) => Some(n.clone()),
            ExprKind::Str(s) => Some(quote(s)),
            _ => None,
        }
    }

    fn cmp(&mut self, op: &str, l: &Expr, r: &Expr) -> Lit {
        if op == "==" || op == "!=" {
            let pos = op == "==";
            for (t, v) in [(l, r), (r, l)] {
                if let Some(val) = Conv::value_atom(v) {
                    if Conv::is_term(t) {
                        if let Some(d) = (self.dom)(t) {
                            self.domains.insert(t.to_string(), d);
                        }
                        return Lit::Is(t.to_string(), val, pos);
                    }
                }
            }
        }
        let text = format!("{} {} {}", l, op, r);
        let (Some(a), Some(b)) = (self.lin(l), self.lin(r)) else {
            return Lit::Opaque(text, true);
        };
        match op {
            "<" => Lit::Cmp(a.add(b, -1), Rel::Lt),
            "<=" => Lit::Cmp(a.add(b, -1), Rel::Le),
            ">" => Lit::Cmp(b.add(a, -1), Rel::Lt),
            ">=" => Lit::Cmp(b.add(a, -1), Rel::Le),
            "==" => Lit::Cmp(a.add(b, -1), Rel::Eq),
            _ => Lit::Cmp(a.add(b, -1), Rel::Ne),
        }
    }

    // DNF wyrażenia albo jego zaprzeczenia (pos = false).
    fn dnf(&mut self, e: &Expr, pos: bool) -> Option<Dnf> {
        match &e.kind {
            ExprKind::Bool(b) => Some(if *b == pos { vec![vec![]] } else { vec![] }),
            ExprKind::Not(x) => self.dnf(x, !pos),
            ExprKind::Bin { op, l, r } if *op == "&&" || *op == "||" => {
                let a = self.dnf(l, pos)?;
                let b = self.dnf(r, pos)?;
                // ¬(a && b) = ¬a || ¬b
                if (*op == "&&") == pos {
                    and(a, b)
                } else {
                    let mut v = a;
                    v.extend(b);
                    (v.len() <= MAX_CONJ).then_some(v)
                }
            }
            ExprKind::Bin { op, l, r } if matches!(*op, "<" | "<=" | ">" | ">=" | "==" | "!=") => {
                let lit = self.cmp(op, l, r);
                Some(vec![vec![if pos { lit } else { negate(lit) }]])
            }
            _ => Some(vec![vec![Lit::Opaque(e.to_string(), pos)]]),
        }
    }

    fn all(&mut self, es: &[Expr], pos: bool) -> Option<Dnf> {
        if pos {
            let mut acc: Dnf = vec![vec![]];
            for e in es {
                acc = and(acc, self.dnf(e, true)?)?;
            }
            Some(acc)
        } else {
            // ¬(a ∧ b) = ¬a ∨ ¬b
            let mut acc: Dnf = vec![];
            for e in es {
                acc.extend(self.dnf(e, false)?);
            }
            (acc.len() <= MAX_CONJ).then_some(acc)
        }
    }
}

fn opaque_keys(e: &Expr, out: &mut BTreeSet<String>) {
    match &e.kind {
        ExprKind::Not(x) => opaque_keys(x, out),
        ExprKind::Bin { op, l, r } if *op == "&&" || *op == "||" => {
            opaque_keys(l, out);
            opaque_keys(r, out);
        }
        ExprKind::Bin { op, l, r } if matches!(*op, "<" | "<=" | ">" | ">=" | "==" | "!=") => {
            out.insert(format!("{} {} {}", l, op, r));
        }
        _ => {
            out.insert(e.to_string());
        }
    }
}

// ---------- Fourier–Motzkin na liczbach całkowitych ----------

// Σ a·x + c <= 0, a z trzecim polem Σ a·x + c < 0.
type Row = (BTreeMap<String, i128>, i128, bool);

fn ceil_div(a: i128, b: i128) -> i128 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) == (b < 0)) {
        q + 1
    } else {
        q
    }
}

fn floor_div(a: i128, b: i128) -> i128 {
    let q = a / b;
    if (a % b != 0) && ((a < 0) != (b < 0)) {
        q - 1
    } else {
        q
    }
}

// Dzieli przez NWD współczynników i zaokrągla stałą: na liczbach całkowitych to ten sam zbiór.
// Σ < 0 na liczbach całkowitych to Σ + 1 <= 0. Wiersz ze zmienną Dec dzieli się tylko dokładnie.
fn tighten(mut r: Row, dec: &BTreeSet<String>) -> Row {
    if r.0.keys().any(|k| dec.contains(k)) {
        let g = r.0.values().fold(r.1, |g, a| gcd(g, *a));
        if g > 1 {
            for a in r.0.values_mut() {
                *a /= g;
            }
            r.1 /= g;
        }
        return r;
    }
    if r.2 {
        r = (r.0, r.1 + 1, false);
    }
    let g = r.0.values().fold(0, |g, a| gcd(g, *a));
    if g > 1 {
        for a in r.0.values_mut() {
            *a /= g;
        }
        r.1 = ceil_div(r.1, g);
    }
    r
}

// Wiersz całkowity z literału: mnoży przez wspólny mianownik, a Money zamienia na grosze.
fn row(e: &Lin, vars: &BTreeMap<String, Num>, strict: bool, dec: &BTreeSet<String>) -> Row {
    let mut coefs: Vec<(String, Rat)> = vec![];
    for (k, v) in &e.terms {
        let s = if vars.get(k) == Some(&Num::Money) { Rat(1, 100) } else { Rat::int(1) };
        coefs.push((k.clone(), v.mul(s)));
    }
    let mut l = e.c.1;
    for (_, r) in &coefs {
        l = l / gcd(l, r.1) * r.1;
    }
    let mut m = BTreeMap::new();
    for (k, r) in coefs {
        m.insert(k, r.0 * (l / r.1));
    }
    let c = e.c.0 * (l / e.c.1);
    tighten((m, c, strict), dec)
}

enum Fm {
    Unsat,
    Sat(BTreeMap<String, i128>),
    GiveUp,
}

const MAX_ROWS: usize = 4000;

fn fm(rows: Vec<Row>, order: &[String], dec: &BTreeSet<String>) -> Fm {
    let mut stages: Vec<Vec<Row>> = vec![];
    let mut cur = rows;
    for v in order {
        stages.push(cur.clone());
        let (mut pos, mut neg, mut rest) = (vec![], vec![], vec![]);
        for r in cur {
            match r.0.get(v).copied().unwrap_or(0) {
                0 => rest.push(r),
                a if a > 0 => pos.push(r),
                _ => neg.push(r),
            }
        }
        for p in &pos {
            for n in &neg {
                let ap = p.0[v];
                let an = -n.0[v];
                let mut m: BTreeMap<String, i128> = BTreeMap::new();
                for (k, a) in &p.0 {
                    *m.entry(k.clone()).or_insert(0) += a * an;
                }
                for (k, a) in &n.0 {
                    *m.entry(k.clone()).or_insert(0) += a * ap;
                }
                m.retain(|_, a| *a != 0);
                rest.push(tighten((m, p.1 * an + n.1 * ap, p.2 || n.2), dec));
            }
        }
        rest.sort();
        rest.dedup();
        if rest.len() > MAX_ROWS {
            return Fm::GiveUp;
        }
        cur = rest;
    }
    if cur.iter().any(|r| r.0.is_empty() && (r.1 > 0 || r.1 == 0 && r.2)) {
        return Fm::Unsat;
    }
    if !dec.is_empty() {
        return Fm::GiveUp;
    }
    // Od ostatniej zmiennej: przedział z wierszy etapu, na którym ją eliminowano,
    // i wartość najbliższa zera, czyli zwykle granica, którą nowy warunek przesunął.
    let mut model = BTreeMap::new();
    for (i, v) in order.iter().enumerate().rev() {
        let (mut lo, mut hi): (Option<i128>, Option<i128>) = (None, None);
        for r in &stages[i] {
            let a = r.0.get(v).copied().unwrap_or(0);
            if a == 0 {
                continue;
            }
            let mut rest = r.1;
            for (k, b) in &r.0 {
                if k != v {
                    rest += b * model.get(k).copied().unwrap_or(0);
                }
            }
            // a·v + rest <= 0
            if a > 0 {
                let h = floor_div(-rest, a);
                hi = Some(hi.map_or(h, |x| x.min(h)));
            } else {
                let l = ceil_div(rest, -a);
                lo = Some(lo.map_or(l, |x| x.max(l)));
            }
        }
        if let (Some(l), Some(h)) = (lo, hi) {
            if l > h {
                return Fm::GiveUp;
            }
        }
        let x = match (lo, hi) {
            (Some(l), _) if l > 0 => l,
            (_, Some(h)) if h < 0 => h,
            _ => 0,
        };
        model.insert(v.clone(), x);
    }
    Fm::Sat(model)
}

fn show(v: i128, n: Num) -> String {
    match n {
        Num::Int | Num::Dec => v.to_string(),
        Num::Money if v % 100 == 0 => (v / 100).to_string(),
        Num::Money => format!("{}{}.{:02}", if v < 0 { "-" } else { "" }, (v / 100).abs(), (v % 100).abs()),
    }
}

// Model spełnia literał? Money jest w groszach.
fn holds(l: &Lit, model: &BTreeMap<String, i128>, vars: &BTreeMap<String, Num>) -> bool {
    let Lit::Cmp(e, rel) = l else { return true };
    let mut s = e.c;
    for (k, a) in &e.terms {
        let v = model.get(k).copied().unwrap_or(0);
        let v = if vars.get(k) == Some(&Num::Money) { Rat::norm(v, 100) } else { Rat::int(v) };
        s = s.add(a.mul(v));
    }
    match rel {
        Rel::Le => s.0 <= 0,
        Rel::Lt => s.0 < 0,
        Rel::Eq => s.0 == 0,
        Rel::Ne => s.0 != 0,
    }
}

enum Conj {
    Unsat,
    Sat(String),
    Unknown,
}

fn solve_conj(lits: &[Lit], vars: &BTreeMap<String, Num>, nonneg: &BTreeSet<String>, domains: &BTreeMap<String, Vec<String>>) -> Conj {
    // Zdania i przypisania: sprzeczność tylko wtedy, gdy to samo jest naraz prawdą i fałszem.
    let mut is_vals: BTreeMap<&str, (Option<&str>, Vec<&str>)> = BTreeMap::new();
    for l in lits {
        match l {
            Lit::Opaque(k, p) => {
                if lits.contains(&Lit::Opaque(k.clone(), !p)) {
                    return Conj::Unsat;
                }
            }
            Lit::Is(t, v, p) => {
                let e = is_vals.entry(t).or_insert((None, vec![]));
                if *p {
                    if e.0.is_some_and(|x| x != v) {
                        return Conj::Unsat;
                    }
                    e.0 = Some(v);
                } else {
                    e.1.push(v);
                }
            }
            Lit::Cmp(..) => {}
        }
    }
    // Przy znanej dziedzinie `!= Issued` wybiera inny wariant, a gdy żadnego nie ma, nie da się.
    let mut picked: BTreeMap<&str, String> = BTreeMap::new();
    for (t, (eq, ne)) in &is_vals {
        if eq.is_some_and(|x| ne.contains(&x)) {
            return Conj::Unsat;
        }
        if let Some(d) = domains.get(*t) {
            match eq {
                Some(v) if !d.iter().any(|x| x == v) => return Conj::Unsat,
                Some(_) => {}
                None => match d.iter().find(|x| !ne.contains(&x.as_str())) {
                    Some(v) => {
                        picked.insert(t, v.clone());
                    }
                    None => return Conj::Unsat,
                },
            }
        }
    }
    // != rozbija się na < albo >.
    let cmps: Vec<&Lit> = lits.iter().filter(|l| matches!(l, Lit::Cmp(..))).collect();
    let nes: Vec<&Lin> = cmps
        .iter()
        .filter_map(|l| match l {
            Lit::Cmp(e, Rel::Ne) => Some(e),
            _ => None,
        })
        .collect();
    if nes.len() > 8 {
        return Conj::Unknown;
    }
    let used: BTreeSet<String> = cmps
        .iter()
        .flat_map(|l| match l {
            Lit::Cmp(e, _) => e.terms.keys().cloned().collect::<Vec<_>>(),
            _ => vec![],
        })
        .collect();
    // Eliminacja od końca alfabetu, więc α zostaje na koniec i dostaje wartość najbliższą granicy.
    let order: Vec<String> = used.iter().rev().cloned().collect();
    let dec: BTreeSet<String> = used.iter().filter(|k| vars.get(*k) == Some(&Num::Dec)).cloned().collect();
    let mut gave_up = false;
    for mask in 0..(1u32 << nes.len()) {
        let mut rows = vec![];
        let mut ni = 0;
        for l in &cmps {
            let Lit::Cmp(e, rel) = l else { continue };
            match rel {
                Rel::Le => rows.push(row(e, vars, false, &dec)),
                Rel::Lt => rows.push(row(e, vars, true, &dec)),
                Rel::Eq => {
                    rows.push(row(e, vars, false, &dec));
                    rows.push(row(&e.clone().scale(Rat::int(-1)), vars, false, &dec));
                }
                Rel::Ne => {
                    let e = if mask & (1 << ni) == 0 { e.clone() } else { e.clone().scale(Rat::int(-1)) };
                    rows.push(row(&e, vars, true, &dec));
                    ni += 1;
                }
            }
        }
        for v in nonneg.iter().filter(|v| used.contains(*v)) {
            let mut m = BTreeMap::new();
            m.insert(v.clone(), -1);
            rows.push((m, 0, false));
        }
        match fm(rows, &order, &dec) {
            Fm::Unsat => continue,
            Fm::GiveUp => gave_up = true,
            Fm::Sat(model) => {
                if !lits.iter().all(|l| holds(l, &model, vars)) {
                    gave_up = true;
                    continue;
                }
                let mut parts = vec![];
                for (k, v) in &model {
                    parts.push((k.clone(), show(*v, vars.get(k).copied().unwrap_or(Num::Int))));
                }
                for (t, (eq, ne)) in &is_vals {
                    match (eq, picked.get(t)) {
                        (Some(v), _) => parts.push((t.to_string(), v.to_string())),
                        (None, Some(v)) => parts.push((t.to_string(), v.clone())),
                        (None, None) => parts.push((format!("{} !=", t), ne.join(", "))),
                    }
                }
                if parts.len() == 1 && parts[0].0 == "α" {
                    return Conj::Sat(parts[0].1.clone());
                }
                let s: Vec<String> = parts
                    .iter()
                    .map(|(k, v)| if k.ends_with(" !=") { format!("{} {}", k, v) } else { format!("{} = {}", k, v) })
                    .collect();
                return Conj::Sat(s.join(", "));
            }
        }
    }
    if gave_up { Conj::Unknown } else { Conj::Unsat }
}

// Czy koniunkcja `new` pociąga koniunkcję `old`? Warunki muszą mieć parametr nazwany α.
// `num` mówi, czy term (α, α.pole, parametr) jest liczbą, a jeśli tak, to jaką.
// `dom` podaje warianty termu, którego typ to unia wariantów bez danych.
pub fn implies(
    new: &[Expr],
    old: &[Expr],
    num: &dyn Fn(&Expr) -> Option<Num>,
    dom: &dyn Fn(&Expr) -> Option<Vec<String>>,
) -> Verdict {
    let mut cv = Conv {
        num,
        dom,
        domains: BTreeMap::new(),
        vars: BTreeMap::new(),
        nonneg: BTreeSet::new(),
    };
    let (Some(a), Some(b)) = (cv.all(new, true), cv.all(old, false)) else {
        return Verdict::Unknown;
    };
    let Some(conjs) = and(a, b) else {
        return Verdict::Unknown;
    };
    let mut old_keys = BTreeSet::new();
    for e in old {
        opaque_keys(e, &mut old_keys);
    }
    let mut shared: Option<String> = None;
    let mut unknown = false;
    for c in &conjs {
        match solve_conj(c, &cv.vars, &cv.nonneg, &cv.domains) {
            Conj::Unsat => {}
            Conj::Unknown => unknown = true,
            Conj::Sat(s) => {
                let opaque: Vec<&Lit> = c.iter().filter(|l| matches!(l, Lit::Opaque(..))).collect();
                if opaque.is_empty() {
                    return Verdict::Counter(s);
                }
                // Zdanie, które stoi w obu warunkach i jest tu prawdziwe, nie tworzy kontrprzykładu:
                // Nip z len(α) == 11 zamiast 10 przepuszcza 11 cyfr niezależnie od sumy kontrolnej.
                let only_shared = opaque.iter().all(|l| matches!(l, Lit::Opaque(k, true) if old_keys.contains(k)));
                let has_values = c.iter().any(|l| !matches!(l, Lit::Opaque(..)));
                if only_shared && has_values {
                    shared.get_or_insert(s);
                } else {
                    unknown = true;
                }
            }
        }
    }
    match shared {
        Some(s) => Verdict::Counter(s),
        None if unknown => Verdict::Unknown,
        None => Verdict::Implied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_file;

    // Warunki z typu `type T = Int(...)` w małym pliku.
    fn cond(src: &str) -> Vec<Expr> {
        let f = parse_file(&format!("type T = {}\n", src), "t.sowa", "t", false).unwrap();
        let Item::Type(t) = &f.items[0] else { panic!() };
        let TypeBody::Rhs(ts) = &t.body else { panic!() };
        let TypeExpr::Name { cond, .. } = &ts[0] else { panic!() };
        cond.iter().map(|c| c.expr.clone()).collect()
    }

    fn int(e: &Expr) -> Option<Num> {
        match &e.kind {
            ExprKind::Field { name, .. } if name == "status" => None,
            _ => Some(Num::Int),
        }
    }

    fn money(_: &Expr) -> Option<Num> {
        Some(Num::Money)
    }

    fn none(_: &Expr) -> Option<Vec<String>> {
        None
    }

    fn status(e: &Expr) -> Option<Vec<String>> {
        matches!(&e.kind, ExprKind::Field { name, .. } if name == "status").then(|| vec!["Issued".into(), "Paid".into()])
    }

    fn v(new: &str, old: &str) -> Verdict {
        implies(&cond(new), &cond(old), &int, &none)
    }

    #[test]
    fn looser_bound() {
        assert_eq!(v("Int(α >= 0 && α <= 1000)", "Int(α >= 0 && α <= 100)"), Verdict::Counter("101".into()));
        assert_eq!(v("Int(α >= 0)", "Int(α > 0)"), Verdict::Counter("0".into()));
        assert_eq!(v("Int", "Int(α >= 0)"), Verdict::Counter("-1".into()));
    }

    #[test]
    fn stricter_or_same() {
        assert_eq!(v("Int(α >= 0 && α <= 50)", "Int(α >= 0 && α <= 100)"), Verdict::Implied);
        assert_eq!(v("Int(α > 0)", "Int(α >= 1)"), Verdict::Implied);
        assert_eq!(v("Int(0 <= α && 100 >= α)", "Int(α >= 0 && α <= 100)"), Verdict::Implied);
        assert_eq!(v("Int(α >= 0)", "Int(α >= 0 || α == -5)"), Verdict::Implied);
    }

    #[test]
    fn len_and_opaque() {
        assert_eq!(v("String(len(α) <= 30)", "String(len(α) <= 20)"), Verdict::Counter("len(α) = 21".into()));
        assert_eq!(v("String", "String(len(α) > 0)"), Verdict::Counter("len(α) = 0".into()));
        assert_eq!(
            v("String(len(α) == 11 && only_digits(α))", "String(len(α) == 10 && only_digits(α))"),
            Verdict::Counter("len(α) = 11".into())
        );
        assert_eq!(v("String(valid_email(α))", "String(len(α) > 0)"), Verdict::Unknown);
        assert_eq!(v("String(valid_email(α) && len(α) > 0)", "String(valid_email(α))"), Verdict::Implied);
        assert_eq!(v("Int(α * α < 10)", "Int(α < 4)"), Verdict::Unknown);
    }

    #[test]
    fn variants_and_fields() {
        assert_eq!(v("Invoice(α.status == Paid)", "Invoice(α.status == Issued)"), Verdict::Counter("α.status = Paid".into()));
        assert_eq!(v("Invoice(α.status == Issued)", "Invoice(α.status == Issued)"), Verdict::Implied);
        assert_eq!(v("Invoice", "Invoice(α.status == Issued)"), Verdict::Counter("α.status != Issued".into()));
        // Przy dziedzinie Issued | Paid `!= Issued` to to samo co `== Paid`.
        let w = |n: &str, o: &str| implies(&cond(n), &cond(o), &int, &status);
        assert_eq!(w("Invoice(α.status != Issued)", "Invoice(α.status == Paid)"), Verdict::Implied);
        assert_eq!(w("Invoice", "Invoice(α.status == Issued)"), Verdict::Counter("α.status = Paid".into()));
    }

    #[test]
    fn money_in_grosze() {
        let r = implies(&cond("Money(α >= 0)"), &cond("Money(α > 0)"), &money, &none);
        assert_eq!(r, Verdict::Counter("0".into()));
        let r = implies(&cond("Money(α > 0.5)"), &cond("Money(α > 1)"), &money, &none);
        assert_eq!(r, Verdict::Counter("0.51".into()));
        let r = implies(&cond("Totals(α.gross == α.net + α.vat)"), &cond("Totals(α.gross >= α.net)"), &money, &none);
        assert_eq!(r, Verdict::Counter("α.gross = 0, α.net = 0.01, α.vat = -0.01".into()));
    }
}
