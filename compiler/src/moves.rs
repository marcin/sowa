// Przenoszenie zamiast klonowania w backendzie Rust.
//
// Wartości Sowy są niezmienne i liczone referencjami, więc `map` i `with` mogą zmienić listę albo
// rekord w miejscu, gdy nikt inny go nie trzyma. Żeby tak było, ostatni odczyt zmiennej przenosi
// wartość (std::mem::take) zamiast ją klonować. Analiza idzie od końca bloku i pamięta zmienne,
// które będą jeszcze czytane. W pętli za żywe uznaje wszystkie zmienne z jej ciała, poza `x = f(x)`,
// gdzie stara wartość x i tak znika.
//
// Odczyt zmiennej nie ma skutków ubocznych, więc w liście argumentów generator liczy najpierw
// argumenty, które nie są samą zmienną, a zmienne na końcu (args_order). Analiza przyjmuje tę samą
// kolejność. Zmienne z otoczenia lambdy nie są przenoszone, bo domknięcie trzyma je dalej.

use crate::ast::*;
use std::collections::HashSet;

pub type Moves = HashSet<usize>;

pub const BY_REF: &[&str] = &["len", "at", "join", "contains", "starts_with", "trim", "split", "chars"];

pub fn key(e: &Expr) -> usize {
    e as *const Expr as usize
}

struct Occ<'e> {
    name: &'e str,
    at: usize,
    movable: bool,
}

// Kolejność listy argumentów: najpierw wyrażenia, potem same zmienne.
pub fn args_order<'e>(items: &[&'e Expr]) -> Vec<&'e Expr> {
    let (vars, rest): (Vec<&Expr>, Vec<&Expr>) = items.iter().partition(|e| matches!(e.kind, ExprKind::Ident(_)));
    rest.into_iter().chain(vars).collect()
}

fn occs<'e>(e: &'e Expr, out: &mut Vec<Occ<'e>>) {
    let list = |items: Vec<&'e Expr>, out: &mut Vec<Occ<'e>>| {
        for x in args_order(&items) {
            occs(x, out);
        }
    };
    match &e.kind {
        ExprKind::Int(_) | ExprKind::Dec(_) | ExprKind::Str(_) | ExprKind::Bool(_) => {}
        ExprKind::Ident(n) => out.push(Occ { name: n, at: key(e), movable: true }),
        ExprKind::Html(segs) => {
            for s in segs {
                if let HtmlSeg::Expr(x) = s {
                    occs(x, out);
                }
            }
        }
        ExprKind::List(xs) => list(xs.iter().collect(), out),
        // Te funkcje czytają argument przez referencję.
        ExprKind::Call { name, args } if BY_REF.contains(&name.as_str()) => {
            for x in args_order(&args.iter().map(|a| &a.value).collect::<Vec<_>>()) {
                match &x.kind {
                    ExprKind::Ident(n) => out.push(Occ { name: n, at: key(x), movable: false }),
                    _ => occs(x, out),
                }
            }
        }
        ExprKind::Call { args, .. } => list(args.iter().map(|a| &a.value).collect(), out),
        ExprKind::Method { obj, targs, args, .. } => {
            list(std::iter::once(&**obj).chain(args.iter().map(|a| &a.value)).collect(), out);
            for t in targs {
                ty_names(t, out);
            }
        }
        ExprKind::Field { obj, .. } => match &obj.kind {
            // Pole zmiennej generator czyta przez referencję.
            ExprKind::Ident(n) => out.push(Occ { name: n, at: key(obj), movable: false }),
            _ => occs(obj, out),
        },
        ExprKind::Bin { l, r, .. } => {
            occs(l, out);
            occs(r, out);
        }
        ExprKind::Not(x) | ExprKind::Neg(x) | ExprKind::Try(x) => occs(x, out),
        ExprKind::Is { e: x, ty, .. } => {
            occs(x, out);
            ty_names(ty, out);
        }
        ExprKind::As { e: x, ty, alt } => {
            occs(x, out);
            ty_names(ty, out);
            if let Some(alt) = alt {
                match &**alt {
                    Alt::Value(v) | Alt::Return(Some(v)) => pinned(v, out),
                    Alt::Return(None) => {}
                    Alt::Block(ss) => pinned_stmts(ss, out),
                }
            }
        }
        ExprKind::With { e: x, fields } => list(std::iter::once(&**x).chain(fields.iter().map(|(_, v)| v)).collect(), out),
        ExprKind::Lambda { body, .. } => match body {
            LambdaBody::Expr(x) => pinned(x, out),
            LambdaBody::Block(ss) => pinned_stmts(ss, out),
        },
    }
}

// Odczyty, których nie wolno przenieść: w lambdzie, w warunku typu, w bloku `or`.
fn pinned<'e>(e: &'e Expr, out: &mut Vec<Occ<'e>>) {
    let from = out.len();
    occs(e, out);
    for o in &mut out[from..] {
        o.movable = false;
    }
}
fn pinned_stmts<'e>(ss: &'e [Stmt], out: &mut Vec<Occ<'e>>) {
    for s in ss {
        match s {
            Stmt::Set { expr, .. } | Stmt::Expr { expr, .. } | Stmt::Return { expr: Some(expr), .. } => pinned(expr, out),
            Stmt::Return { expr: None, .. } => {}
            Stmt::If { cond, then, els, .. } => {
                pinned(cond, out);
                pinned_stmts(then, out);
                if let Some(e) = els {
                    pinned_stmts(e, out);
                }
            }
            Stmt::While { cond, body, .. } => {
                pinned(cond, out);
                pinned_stmts(body, out);
            }
            Stmt::For { iter, body, .. } => {
                pinned(iter, out);
                pinned_stmts(body, out);
            }
            Stmt::Match { subjects, arms, .. } => {
                for x in subjects {
                    pinned(x, out);
                }
                for a in arms {
                    pinned_stmts(&a.body, out);
                }
            }
        }
    }
}
fn ty_names<'e>(t: &'e TypeExpr, out: &mut Vec<Occ<'e>>) {
    match t {
        TypeExpr::Union(xs) => {
            for x in xs {
                ty_names(x, out);
            }
        }
        TypeExpr::Name { args, cond, fields, .. } => {
            for a in args {
                ty_names(a, out);
            }
            if let Some(c) = cond {
                pinned(&c.expr, out);
            }
            for f in fields.iter().flatten() {
                ty_names(&f.ty, out);
            }
        }
    }
}

// Czy w wyrażeniu jest przenoszony odczyt zmiennej n.
pub fn moved_in(e: &Expr, n: &str, moves: &Moves) -> bool {
    let mut o = vec![];
    occs(e, &mut o);
    o.iter().any(|x| x.name == n && moves.contains(&x.at))
}

fn names(ss: &[Stmt]) -> HashSet<String> {
    let mut o = vec![];
    pinned_stmts(ss, &mut o);
    o.into_iter().map(|x| x.name.to_string()).collect()
}
pub fn expr_names(e: &Expr) -> HashSet<String> {
    let mut o = vec![];
    occs(e, &mut o);
    o.into_iter().map(|x| x.name.to_string()).collect()
}

// Nazwy przypisywane w bloku, bez zagnieżdżonych lambd.
pub fn assigned(ss: &[Stmt], out: &mut HashSet<String>) {
    for s in ss {
        match s {
            Stmt::Set { name, .. } => {
                out.insert(name.clone());
            }
            Stmt::If { then, els, .. } => {
                assigned(then, out);
                if let Some(e) = els {
                    assigned(e, out);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => assigned(body, out),
            Stmt::Match { arms, .. } => {
                for a in arms {
                    assigned(&a.body, out);
                }
            }
            Stmt::Return { .. } | Stmt::Expr { .. } => {}
        }
    }
}

pub struct Live<'a> {
    // Nazwy z otoczenia lambdy.
    pub outer: &'a HashSet<String>,
    pub moves: &'a mut Moves,
}

impl Live<'_> {
    pub fn body(&mut self, ss: &[Stmt]) {
        self.block(ss, HashSet::new());
    }
    pub fn expr_body(&mut self, e: &Expr) {
        self.mark(e, |_| true);
    }

    // Zaznacza ostatnie odczyty w wyrażeniu i zwraca wszystkie czytane nazwy.
    fn mark(&mut self, e: &Expr, dead: impl Fn(&str) -> bool) -> HashSet<String> {
        let mut o = vec![];
        occs(e, &mut o);
        for (i, x) in o.iter().enumerate() {
            if x.movable && !self.outer.contains(x.name) && dead(x.name) && !o[i + 1..].iter().any(|y| y.name == x.name) {
                self.moves.insert(x.at);
            }
        }
        o.into_iter().map(|x| x.name.to_string()).collect()
    }

    fn block(&mut self, ss: &[Stmt], mut live: HashSet<String>) -> HashSet<String> {
        for s in ss.iter().rev() {
            live = self.stmt(s, live);
        }
        live
    }

    fn stmt(&mut self, s: &Stmt, live: HashSet<String>) -> HashSet<String> {
        match s {
            Stmt::Set { name, expr, .. } => {
                let used = self.mark(expr, |y| y == name || !live.contains(y));
                let mut l = live;
                l.remove(name);
                l.extend(used);
                l
            }
            Stmt::Return { expr, .. } => match expr {
                Some(e) => self.mark(e, |_| true),
                None => HashSet::new(),
            },
            Stmt::Expr { expr, .. } => {
                let used = self.mark(expr, |y| !live.contains(y));
                let mut l = live;
                l.extend(used);
                l
            }
            Stmt::If { cond, then, els, .. } => {
                let mut both = self.block(then, live.clone());
                match els {
                    Some(e) => both.extend(self.block(e, live)),
                    None => both.extend(live),
                }
                let used = self.mark(cond, |y| !both.contains(y));
                both.extend(used);
                both
            }
            Stmt::While { cond, body, .. } => {
                let mut l = live;
                l.extend(expr_names(cond));
                l.extend(names(body));
                self.block(body, l.clone());
                l
            }
            Stmt::For { iter, body, .. } => {
                let mut l = live;
                l.extend(names(body));
                self.block(body, l.clone());
                let used = self.mark(iter, |y| !l.contains(y));
                l.extend(used);
                l
            }
            Stmt::Match { subjects, arms, .. } => {
                let mut l = HashSet::new();
                for a in arms {
                    l.extend(self.block(&a.body, live.clone()));
                }
                for x in subjects {
                    l.extend(expr_names(x));
                }
                l
            }
        }
    }
}

// Czy któryś odczyt zmiennej n w bloku przenosi wartość.
pub fn moved_any(ss: &[Stmt], n: &str, moves: &Moves) -> bool {
    let mut o = vec![];
    pinned_stmts(ss, &mut o);
    o.iter().any(|x| x.name == n && moves.contains(&x.at))
}

fn sets<'e>(ss: &'e [Stmt], out: &mut Vec<&'e Stmt>) {
    for s in ss {
        match s {
            Stmt::Set { .. } => out.push(s),
            Stmt::If { then, els, .. } => {
                sets(then, out);
                if let Some(e) = els {
                    sets(e, out);
                }
            }
            Stmt::While { body, .. } | Stmt::For { body, .. } => sets(body, out),
            Stmt::Match { arms, .. } => {
                for a in arms {
                    sets(&a.body, out);
                }
            }
            Stmt::Return { .. } | Stmt::Expr { .. } => {}
        }
    }
}

// Przypisania `x = at(xs, i)`, które mogą trzymać referencję do elementu zamiast kopii: x i xs nie
// są w bloku zmieniane ani przenoszone. Zwraca adresy instrukcji.
pub fn ref_lets(ss: &[Stmt], moves: &Moves) -> Vec<usize> {
    let mut all = vec![];
    sets(ss, &mut all);
    let mut ch = HashSet::new();
    assigned(ss, &mut ch);
    let mut out = vec![];
    for s in &all {
        let Stmt::Set { name, expr, .. } = s else { continue };
        let ExprKind::Call { name: f, args } = &expr.kind else { continue };
        if f != "at" || args.len() != 2 || args.iter().any(|a| a.name.is_some()) {
            continue;
        }
        let ExprKind::Ident(src) = &args[0].value.kind else { continue };
        let once = all.iter().filter(|t| matches!(t, Stmt::Set { name: m, .. } if m == name)).count() == 1;
        if once && !ch.contains(src) && !moved_any(ss, name, moves) && !moved_any(ss, src, moves) {
            out.push(*s as *const Stmt as usize);
        }
    }
    out
}
