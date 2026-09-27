// Runtime Sowy w Ruście dla `sowa test --rust` i `sowa run --rust`. To nie jest moduł kompilatora: codegen_rs.rs
// dokleja ten plik na początek programu, a rustc kompiluje całość do jednego pliku wykonywalnego.
// Zachowanie jest takie samo jak w runtime.js: te same wartości, typy w runtime, JSON, formularze,
// baza (SQLite z systemu przez FFI, bez zależności) i ten sam generator liczb losowych w property.
//
// Wartości: Int to i64 w zakresie ±(2⁵³-1) jak w JS, Money to i128 ze skalą 10²⁰ (jak Dec w JS),
// rekord i wariant to nazwa i pola w kolejności z definicji typu.
#![allow(unused, unreachable_code, unused_mut, unused_parens, dead_code, non_snake_case, unused_braces)]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_long, c_uchar, CStr, CString};
use std::fmt::Write as _;
use std::rc::Rc;

// ---------- błędy ----------

pub enum Ctl {
    // Wynik `try` w teście: błąd wołanej funkcji.
    Ret(V),
    // Wartość niezgodna z typem: błąd programu.
    Type(String),
    // Niespełniony przykład.
    Fail(String),
    // Błąd SQLite; w transakcji zamienia się na DbError.
    Db(String),
}
pub type R = Result<V, Ctl>;

fn terr<T>(msg: impl Into<String>) -> Result<T, Ctl> {
    Err(Ctl::Type(msg.into()))
}

// ---------- wartości ----------

#[derive(Clone)]
pub enum V {
    Unit,
    Int(i64),
    Dec(i128),
    Str(Rc<str>),
    Bool(bool),
    Html(Rc<str>),
    Date(i64, i64, i64),
    DT(i64, i64, i64, i64, i64, i64),
    List(Rc<Vec<V>>),
    Rec(Rc<Obj>),
    Var(Rc<Obj>),
    Cap(Rc<Cap>),
    Fn(Rc<dyn Fn(V) -> R>),
}

#[derive(Clone)]
pub struct Obj {
    pub n: &'static str,
    pub f: Vec<(&'static str, V)>,
}

impl Obj {
    fn get(&self, k: &str) -> Option<&V> {
        match self.f.iter().find(|(n, _)| std::ptr::eq(n.as_ptr(), k.as_ptr()) && n.len() == k.len()) {
            Some((_, v)) => Some(v),
            None => self.f.iter().find(|(n, _)| *n == k).map(|(_, v)| v),
        }
    }
}

pub fn s(x: &str) -> V {
    V::Str(Rc::from(x))
}
pub fn list(v: Vec<V>) -> V {
    V::List(Rc::new(v))
}

// ---------- liczby ----------

const S: i128 = 100_000_000_000_000_000_000;
const MAX_SAFE: i64 = 9_007_199_254_740_991;

fn int(n: Option<i64>) -> R {
    match n {
        Some(n) if (-MAX_SAFE..=MAX_SAFE).contains(&n) => Ok(V::Int(n)),
        Some(n) => terr(format!("liczba poza zakresem Int: {}", n)),
        None => terr("liczba poza zakresem Int"),
    }
}

pub fn dec(t: &str) -> R {
    let t = t.trim();
    let (neg, body) = match t.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, t),
    };
    let (ip, fp) = match body.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (body, None),
    };
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    if !digits(ip) || fp.is_some_and(|f| !digits(f)) {
        return terr(format!("to nie jest kwota: {}", js_quote(t)));
    }
    let mut frac: String = fp.unwrap_or("").chars().take(20).collect();
    while frac.len() < 20 {
        frac.push('0');
    }
    let i: i128 = match ip.parse::<i128>() {
        Ok(i) if i < i128::MAX / S => i,
        _ => return terr(format!("kwota poza zakresem: {}", t)),
    };
    let v = i * S + frac.parse::<i128>().unwrap();
    Ok(V::Dec(if neg { -v } else { v }))
}

pub fn dec_lit(t: &str) -> V {
    dec(t).unwrap_or(V::Unit)
}

fn isnum(x: &V) -> bool {
    matches!(x, V::Int(_) | V::Dec(_))
}
fn dm(x: &V) -> Result<i128, Ctl> {
    match x {
        V::Dec(m) => Ok(*m),
        V::Int(n) => Ok(*n as i128 * S),
        _ => terr(format!("oczekiwano liczby, jest {}", show(x))),
    }
}

fn mul128(a: u128, b: u128) -> (u128, u128) {
    const M: u128 = u64::MAX as u128;
    let (a0, a1, b0, b1) = (a & M, a >> 64, b & M, b >> 64);
    let (p00, p01, p10, p11) = (a0 * b0, a0 * b1, a1 * b0, a1 * b1);
    let mid = (p00 >> 64) + (p01 & M) + (p10 & M);
    let lo = (p00 & M) | ((mid & M) << 64);
    let hi = p11 + (p01 >> 64) + (p10 >> 64) + (mid >> 64);
    (hi, lo)
}

fn div256(hi: u128, lo: u128, d: u128) -> Option<u128> {
    if hi >= d {
        return None;
    }
    let (mut rem, mut q) = (hi, 0u128);
    for i in (0..128).rev() {
        let carry = rem >> 127;
        rem = (rem << 1) | ((lo >> i) & 1);
        if carry == 1 || rem >= d {
            rem = rem.wrapping_sub(d);
            q |= 1 << i;
        }
    }
    Some(q)
}

// a * b / c z obcięciem do zera, jak dzielenie bigintów w JS.
fn mul_div(a: i128, b: i128, c: i128) -> Result<i128, Ctl> {
    let neg = (a < 0) ^ (b < 0) ^ (c < 0);
    let (hi, lo) = mul128(a.unsigned_abs(), b.unsigned_abs());
    match div256(hi, lo, c.unsigned_abs()) {
        Some(q) if q <= i128::MAX as u128 => Ok(if neg { -(q as i128) } else { q as i128 }),
        _ => terr("kwota poza zakresem"),
    }
}

fn dec_mul(a: i128, b: i128) -> Result<i128, Ctl> {
    if b % S == 0 {
        if let Some(r) = a.checked_mul(b / S) {
            return Ok(r);
        }
    }
    if a % S == 0 {
        if let Some(r) = b.checked_mul(a / S) {
            return Ok(r);
        }
    }
    mul_div(a, b, S)
}

fn round_dec(m: i128, places: i64) -> i128 {
    let unit = 10i128.pow((20 - places.clamp(0, 20)) as u32);
    let a = m.abs();
    let mut q = a / unit;
    if (a % unit) * 2 >= unit {
        q += 1;
    }
    let r = q * unit;
    if m < 0 { -r } else { r }
}

fn dec_text(m: i128, min: usize, max: Option<i64>) -> String {
    let x = match max {
        Some(p) => round_dec(m, p),
        None => m,
    };
    let a = x.unsigned_abs();
    let ip = a / S as u128;
    let mut fp = format!("{:020}", a % S as u128);
    if let Some(p) = max {
        fp.truncate(p as usize);
    }
    while fp.ends_with('0') {
        fp.pop();
    }
    while fp.len() < min {
        fp.push('0');
    }
    let s = if fp.is_empty() { ip.to_string() } else { format!("{}.{}", ip, fp) };
    if x < 0 && s.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        format!("-{}", s)
    } else {
        s
    }
}

pub fn add(a: V, b: V) -> R {
    match (&a, &b) {
        (V::Str(x), V::Str(y)) => {
            let mut o = String::with_capacity(x.len() + y.len());
            o.push_str(x);
            o.push_str(y);
            Ok(V::Str(Rc::from(o)))
        }
        (V::List(x), V::List(y)) => {
            let mut o = Vec::with_capacity(x.len() + y.len());
            o.extend(x.iter().cloned());
            o.extend(y.iter().cloned());
            Ok(list(o))
        }
        (V::Int(x), V::Int(y)) => int(x.checked_add(*y)),
        _ if isnum(&a) && isnum(&b) => match dm(&a)?.checked_add(dm(&b)?) {
            Some(m) => Ok(V::Dec(m)),
            None => terr("kwota poza zakresem"),
        },
        _ => terr(format!("nie da się dodać {} i {}", show(&a), show(&b))),
    }
}
pub fn sub(a: V, b: V) -> R {
    match (&a, &b) {
        (V::Int(x), V::Int(y)) => int(x.checked_sub(*y)),
        _ if isnum(&a) && isnum(&b) => match dm(&a)?.checked_sub(dm(&b)?) {
            Some(m) => Ok(V::Dec(m)),
            None => terr("kwota poza zakresem"),
        },
        _ => terr(format!("nie da się odjąć {} od {}", show(&b), show(&a))),
    }
}
pub fn mul(a: V, b: V) -> R {
    match (&a, &b) {
        (V::Int(x), V::Int(y)) => int(x.checked_mul(*y)),
        _ if isnum(&a) && isnum(&b) => Ok(V::Dec(dec_mul(dm(&a)?, dm(&b)?)?)),
        _ => terr(format!("nie da się pomnożyć {} i {}", show(&a), show(&b))),
    }
}
pub fn div(a: V, b: V) -> R {
    match (&a, &b) {
        (V::Int(_), V::Int(0)) => terr("dzielenie przez zero"),
        (V::Int(x), V::Int(y)) => Ok(V::Int(x / y)),
        _ if isnum(&a) && isnum(&b) => {
            let d = dm(&b)?;
            if d == 0 {
                return terr("dzielenie przez zero");
            }
            Ok(V::Dec(mul_div(dm(&a)?, S, d)?))
        }
        _ => terr(format!("nie da się podzielić {} przez {}", show(&a), show(&b))),
    }
}
pub fn rem(a: V, b: V) -> R {
    match (&a, &b) {
        (V::Int(x), V::Int(y)) if *y != 0 => Ok(V::Int(x % y)),
        _ => terr(format!("reszta z dzielenia tylko dla Int: {} % {}", show(&a), show(&b))),
    }
}
pub fn neg(a: V) -> R {
    match a {
        V::Int(n) => Ok(V::Int(-n)),
        V::Dec(m) => Ok(V::Dec(-m)),
        _ => terr(format!("nie da się zanegować {}", show(&a))),
    }
}

// ---------- daty ----------

fn valid_date(y: i64, m: i64, d: i64) -> bool {
    if !(1..=12).contains(&m) || d < 1 {
        return false;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    d <= days[(m - 1) as usize]
}
fn num_at(s: &[u8], i: usize, n: usize) -> Option<i64> {
    let p = s.get(i..i + n)?;
    if !p.iter().all(|b| b.is_ascii_digit()) {
        return None;
    }
    std::str::from_utf8(p).ok()?.parse().ok()
}
fn parse_date(t: &str) -> R {
    let b = t.as_bytes();
    let ok = b.len() == 10 && b[4] == b'-' && b[7] == b'-';
    if let (true, Some(y), Some(m), Some(d)) = (ok, num_at(b, 0, 4), num_at(b, 5, 2), num_at(b, 8, 2)) {
        if valid_date(y, m, d) {
            return Ok(V::Date(y, m, d));
        }
    }
    terr(format!("to nie jest data: {}", js_quote(t)))
}
fn parse_dt(t: &str) -> R {
    let b = t.as_bytes();
    let bad = || terr(format!("to nie jest data z godziną: {}", js_quote(t)));
    if !(b.len() == 16 || b.len() == 19) || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') || b[13] != b':' {
        return bad();
    }
    let sec = if b.len() == 19 {
        if b[16] != b':' {
            return bad();
        }
        num_at(b, 17, 2)
    } else {
        Some(0)
    };
    match (num_at(b, 0, 4), num_at(b, 5, 2), num_at(b, 8, 2), num_at(b, 11, 2), num_at(b, 14, 2), sec) {
        (Some(y), Some(mo), Some(d), Some(h), Some(mi), Some(s)) if valid_date(y, mo, d) && h <= 23 && mi <= 59 && s <= 59 => {
            Ok(V::DT(y, mo, d, h, mi, s))
        }
        _ => bad(),
    }
}
fn date_text(y: i64, m: i64, d: i64) -> String {
    format!("{:04}-{:02}-{:02}", y, m, d)
}
fn dt_text(y: i64, mo: i64, d: i64, h: i64, mi: i64, s: i64, sep: char) -> String {
    format!("{:04}-{:02}-{:02}{}{:02}:{:02}:{:02}", y, mo, d, sep, h, mi, s)
}

// ---------- Html ----------

pub fn esc_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
}
pub fn html_part(out: &mut String, x: &V) -> Result<(), Ctl> {
    match x {
        V::Html(h) => out.push_str(h),
        V::List(xs) => {
            for x in xs.iter() {
                html_part(out, x)?;
            }
        }
        x => esc_into(out, &disp(x)),
    }
    Ok(())
}

// ---------- wyświetlanie ----------

fn js_quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

// to_string: tekst dla człowieka.
pub fn disp(x: &V) -> String {
    match x {
        V::Str(s) => s.to_string(),
        V::Int(n) => n.to_string(),
        V::Bool(b) => b.to_string(),
        V::Dec(m) => dec_text(*m, 2, Some(2)),
        V::Date(y, m, d) => date_text(*y, *m, *d),
        V::DT(y, mo, d, h, mi, s) => dt_text(*y, *mo, *d, *h, *mi, *s, ' '),
        V::Html(h) => h.to_string(),
        V::Var(o) if o.f.is_empty() => o.n.to_string(),
        x => show(x),
    }
}

// Zapis wartości w komunikatach testów: jak w kodzie Sowy.
pub fn show(x: &V) -> String {
    match x {
        V::Unit => "(brak wartości)".into(),
        V::Str(s) => js_quote(s),
        V::Int(n) => n.to_string(),
        V::Bool(b) => b.to_string(),
        V::Dec(m) => dec_text(*m, 2, None),
        V::Date(y, m, d) => date_text(*y, *m, *d),
        V::DT(y, mo, d, h, mi, s) => dt_text(*y, *mo, *d, *h, *mi, *s, 'T'),
        V::Html(h) => format!("html{}", js_quote(h)),
        V::List(xs) => format!("[{}]", xs.iter().map(show).collect::<Vec<_>>().join(", ")),
        V::Fn(_) => "(funkcja)".into(),
        V::Cap(c) => format!("({})", c.name()),
        V::Rec(o) | V::Var(o) => {
            if o.f.is_empty() {
                o.n.to_string()
            } else {
                let fs: Vec<String> = o.f.iter().map(|(k, v)| format!("{}: {}", k, show(v))).collect();
                format!("{}({})", o.n, fs.join(", "))
            }
        }
    }
}

// ---------- porównania ----------

pub fn eq(a: &V, b: &V) -> bool {
    match (a, b) {
        (V::Unit, V::Unit) => true,
        (V::Int(x), V::Int(y)) => x == y,
        (V::Str(x), V::Str(y)) => x == y,
        (V::Bool(x), V::Bool(y)) => x == y,
        _ if isnum(a) && isnum(b) => dm(a).ok() == dm(b).ok(),
        (V::Date(..), V::Date(..)) | (V::DT(..), V::DT(..)) => date_key(a) == date_key(b),
        (V::Html(x), V::Html(y)) => x == y,
        (V::List(x), V::List(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|(p, q)| eq(p, q)),
        (V::Rec(x), V::Rec(y)) | (V::Var(x), V::Var(y)) => {
            Rc::ptr_eq(x, y) || (x.n == y.n && x.f.len() == y.f.len() && x.f.iter().all(|(k, v)| y.get(k).is_some_and(|w| eq(v, w))))
        }
        (V::Cap(x), V::Cap(y)) => Rc::ptr_eq(x, y),
        (V::Fn(x), V::Fn(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

fn date_key(x: &V) -> i64 {
    match x {
        V::Date(y, m, d) => y * 10000 + m * 100 + d,
        V::DT(y, mo, d, h, mi, s) => ((((y * 100 + mo) * 100 + d) * 100 + h) * 100 + mi) * 100 + s,
        _ => 0,
    }
}

pub fn cmp(a: &V, b: &V) -> Result<i32, Ctl> {
    let o = |c: std::cmp::Ordering| c as i32;
    match (a, b) {
        (V::Int(x), V::Int(y)) => Ok(o(x.cmp(y))),
        _ if isnum(a) && isnum(b) => Ok(o(dm(a)?.cmp(&dm(b)?))),
        (V::Str(x), V::Str(y)) => Ok(o(x.cmp(y))),
        (V::Date(..), V::Date(..)) | (V::DT(..), V::DT(..)) => Ok(o(date_key(a).cmp(&date_key(b)))),
        _ => terr(format!("nie da się porównać {} i {}", show(a), show(b))),
    }
}

pub fn bool_(x: V) -> Result<bool, Ctl> {
    match x {
        V::Bool(b) => Ok(b),
        x => terr(format!("oczekiwano Bool, jest {}", show(&x))),
    }
}
pub fn iter_(x: V) -> Result<Rc<Vec<V>>, Ctl> {
    match x {
        V::List(l) => Ok(l),
        x => terr(format!("for działa na liście, a dostał {}", show(&x))),
    }
}

// ---------- typy w runtime ----------

#[derive(Clone, Copy, PartialEq)]
pub enum P {
    Int,
    Money,
    String,
    Bool,
    Html,
    Date,
    DateTime,
}

#[derive(Default)]
pub struct Hints {
    pub min: Option<f64>,
    pub min_ex: bool,
    pub max: Option<f64>,
    pub max_ex: bool,
    pub len: Option<i64>,
    pub minlen: Option<i64>,
    pub maxlen: Option<i64>,
    pub re: Option<&'static str>,
    pub prefix: Option<&'static str>,
    pub digits: bool,
    pub email: bool,
}

pub type Fields = Vec<(&'static str, T)>;

pub enum Ty {
    Any,
    Prim(P),
    List(T),
    Cap(&'static str),
    Ref(&'static str),
    Var(&'static str),
    Union(Vec<T>),
    Refine(T, Rc<dyn Fn(V) -> R>, &'static str, Hints),
    Rec(&'static str, Fields),
    Named(&'static str, T),
}
pub type T = Rc<Ty>;

pub fn tp(p: P) -> T {
    Rc::new(Ty::Prim(p))
}
pub fn tany() -> T {
    Rc::new(Ty::Any)
}
pub fn tlist(e: T) -> T {
    Rc::new(Ty::List(e))
}
pub fn tcap(n: &'static str) -> T {
    Rc::new(Ty::Cap(n))
}
pub fn tref(n: &'static str) -> T {
    Rc::new(Ty::Ref(n))
}
pub fn tvar(n: &'static str) -> T {
    Rc::new(Ty::Var(n))
}
pub fn tunion(a: Vec<T>) -> T {
    Rc::new(Ty::Union(a))
}
pub fn trefine(b: T, c: Rc<dyn Fn(V) -> R>, src: &'static str, h: Hints) -> T {
    Rc::new(Ty::Refine(b, c, src, h))
}

thread_local! {
    static TYPES: RefCell<HashMap<&'static str, T>> = RefCell::new(HashMap::new());
    // Pola wariantu; None to wariant bez danych.
    static VD: RefCell<HashMap<&'static str, Option<Rc<Fields>>>> = RefCell::new(HashMap::new());
    static VS: RefCell<HashMap<&'static str, V>> = RefCell::new(HashMap::new());
}

pub fn deftype(n: &'static str, t: T) {
    TYPES.with(|m| m.borrow_mut().insert(n, t));
}
pub fn defrec(n: &'static str, f: Fields) {
    deftype(n, Rc::new(Ty::Rec(n, f)));
}
pub fn defnamed(n: &'static str, d: T) {
    deftype(n, Rc::new(Ty::Named(n, d)));
}
pub fn defvar(n: &'static str, f: Option<Fields>) {
    VD.with(|m| m.borrow_mut().insert(n, f.map(Rc::new)));
}

pub fn init_builtins() {
    defrec("HttpRequest", vec![("method", tref("Method")), ("path", tp(P::String)), ("body", tp(P::String))]);
    defrec("HttpResponse", vec![("status", tp(P::Int)), ("body", tp(P::String))]);
    defnamed("Method", tunion(["Get", "Post", "Put", "Patch", "Delete"].iter().map(|n| tvar(n)).collect()));
    for n in ["Get", "Post", "Put", "Patch", "Delete", "HttpError", "DbError", "NoRow", "NotANumber"] {
        defvar(n, None);
    }
}

pub fn init_variants() {
    let names: Vec<&'static str> = VD.with(|m| m.borrow().iter().filter(|(_, f)| f.is_none()).map(|(n, _)| *n).collect());
    VS.with(|m| {
        let mut m = m.borrow_mut();
        for n in names {
            m.insert(n, V::Var(Rc::new(Obj { n, f: vec![] })));
        }
    });
}

// Wariant bez danych: jedna wspólna wartość.
pub fn vsing(n: &str) -> V {
    VS.with(|m| m.borrow().get(n).cloned()).unwrap_or(V::Unit)
}

fn lookup(n: &str) -> Result<T, Ctl> {
    match TYPES.with(|m| m.borrow().get(n).cloned()) {
        Some(t) => Ok(t),
        None => terr(format!("nieznany typ {}", n)),
    }
}
fn vfields(n: &str) -> Option<Rc<Fields>> {
    VD.with(|m| m.borrow().get(n).cloned().flatten())
}

pub fn tname(d: &Ty) -> String {
    match d {
        Ty::Any => "Any".into(),
        Ty::Prim(p) => pname(*p).into(),
        Ty::List(e) => format!("List<{}>", tname(e)),
        Ty::Cap(n) | Ty::Ref(n) | Ty::Var(n) | Ty::Rec(n, _) | Ty::Named(n, _) => n.to_string(),
        Ty::Union(a) => a.iter().map(|t| tname(t)).collect::<Vec<_>>().join(" | "),
        Ty::Refine(_, _, src, _) => src.to_string(),
    }
}
fn pname(p: P) -> &'static str {
    match p {
        P::Int => "Int",
        P::Money => "Money",
        P::String => "String",
        P::Bool => "Bool",
        P::Html => "Html",
        P::Date => "Date",
        P::DateTime => "DateTime",
    }
}

fn prim_is(p: P, v: &V) -> bool {
    matches!(
        (p, v),
        (P::Int, V::Int(_))
            | (P::Money, V::Dec(_) | V::Int(_))
            | (P::String, V::Str(_))
            | (P::Bool, V::Bool(_))
            | (P::Html, V::Html(_))
            | (P::Date, V::Date(..))
            | (P::DateTime, V::DT(..))
    )
}

fn cap_ok(n: &str, v: &V) -> bool {
    match v {
        V::Cap(c) => c.name() == n || (n == "DbRead" && c.name() == "Db"),
        _ => false,
    }
}

// `is` i dopasowanie w match.
pub fn is(d: &Ty, v: &V) -> bool {
    match d {
        Ty::Any => true,
        Ty::Prim(p) => prim_is(*p, v),
        Ty::List(e) => matches!(v, V::List(xs) if xs.iter().all(|x| is(e, x))),
        Ty::Cap(n) => cap_ok(n, v),
        Ty::Ref(n) => lookup(n).is_ok_and(|t| is(&t, v)),
        Ty::Named(_, t) => is(t, v),
        Ty::Rec(n, _) => matches!(v, V::Rec(o) if o.n == *n),
        Ty::Var(n) => matches!(v, V::Var(o) if o.n == *n),
        Ty::Union(a) => a.iter().any(|t| is(t, v)),
        Ty::Refine(b, c, _, _) => {
            if !is(b, v) {
                return false;
            }
            match conform(b, v.clone(), &Wh::S("")) {
                Ok(x) => matches!(c(x), Ok(V::Bool(true))),
                Err(_) => false,
            }
        }
    }
}

// Opis miejsca w komunikacie o błędzie typu; składany tylko przy błędzie.
pub enum Wh<'a> {
    S(&'a str),
    Idx(&'a Wh<'a>, usize),
    Field(&'a str, &'a str),
}
impl std::fmt::Display for Wh<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Wh::S(s) => write!(f, "{}", s),
            Wh::Idx(w, i) => write!(f, "{}[{}]", w, i),
            Wh::Field(n, k) => write!(f, "{}.{}", n, k),
        }
    }
}

// Sprawdza wartość z typem i zwraca ją po zamianie Int na Money.
pub fn conform(d: &Ty, v: V, what: &Wh) -> R {
    let fail = |v: &V, why: &str| -> R {
        let w = what.to_string();
        terr(format!(
            "{}{} nie jest {}{}",
            if w.is_empty() { String::new() } else { format!("{}: ", w) },
            show(v),
            tname(d),
            if why.is_empty() { String::new() } else { format!(" ({})", why) }
        ))
    };
    match d {
        Ty::Any => Ok(v),
        Ty::Prim(p) => {
            if !prim_is(*p, &v) {
                return fail(&v, "");
            }
            if *p == P::Money {
                return Ok(V::Dec(dm(&v)?));
            }
            Ok(v)
        }
        Ty::List(e) => {
            let V::List(xs) = &v else { return fail(&v, "") };
            // Lista bez Int na miejscu Money zostaje ta sama.
            let mut out: Option<Vec<V>> = None;
            for (i, x) in xs.iter().enumerate() {
                let y = conform(e, x.clone(), &Wh::Idx(what, i))?;
                if let Some(o) = out.as_mut() {
                    o.push(y);
                } else if matches!((x, &y), (V::Int(_), V::Dec(_))) {
                    let mut o: Vec<V> = xs[..i].to_vec();
                    o.push(y);
                    out = Some(o);
                }
            }
            Ok(match out {
                Some(o) => list(o),
                None => v,
            })
        }
        Ty::Cap(n) => {
            if !cap_ok(n, &v) {
                return fail(&v, "");
            }
            Ok(v)
        }
        Ty::Ref(n) => conform(&*lookup(n)?, v, what),
        Ty::Named(_, t) => match conform(t, v.clone(), what) {
            Err(Ctl::Type(_)) => fail(&v, ""),
            r => r,
        },
        Ty::Rec(n, _) => match &v {
            V::Rec(o) if o.n == *n => Ok(v),
            _ => fail(&v, ""),
        },
        Ty::Var(n) => match &v {
            V::Var(o) if o.n == *n => Ok(v),
            _ => fail(&v, ""),
        },
        Ty::Union(a) => {
            for t in a {
                match conform(t, v.clone(), what) {
                    Err(Ctl::Type(_)) => continue,
                    r => return r,
                }
            }
            fail(&v, "")
        }
        Ty::Refine(b, c, _, _) => {
            let x = conform(b, v, what)?;
            match c(x.clone())? {
                V::Bool(true) => Ok(x),
                _ => fail(&x, "warunek"),
            }
        }
    }
}

pub fn conform_s(d: &Ty, v: V, what: &str) -> R {
    conform(d, v, &Wh::S(what))
}

fn target(d: &Ty) -> Result<T, Ctl> {
    let mut d: T = match d {
        Ty::Ref(n) => lookup(n)?,
        Ty::Named(_, t) | Ty::Refine(t, ..) => t.clone(),
        _ => return Ok(Rc::new(Ty::Any)),
    };
    for _ in 0..20 {
        let next = match &*d {
            Ty::Ref(n) => lookup(n)?,
            Ty::Named(_, t) | Ty::Refine(t, ..) => t.clone(),
            _ => break,
        };
        d = next;
    }
    Ok(d)
}
fn target_rec(d: &Ty) -> Option<T> {
    if let Ty::Rec(..) = d {
        return None;
    }
    target(d).ok().filter(|t| matches!(**t, Ty::Rec(..)))
}
fn is_rec(d: &Ty) -> bool {
    matches!(d, Ty::Rec(..)) || target_rec(d).is_some()
}
fn target_list(d: &Ty) -> bool {
    matches!(d, Ty::List(_)) || target(d).is_ok_and(|t| matches!(*t, Ty::List(_)))
}

// `x as T`: None, gdy wartość nie pasuje. Tekst na rekord to JSON albo formularz.
pub fn as_(d: &Ty, v: V) -> Result<Option<V>, Ctl> {
    let r = (|| {
        if let V::Str(t) = &v {
            if is_rec(d) {
                let r: T = match d {
                    Ty::Rec(..) => return as_rec(d, d, t),
                    _ => target_rec(d).unwrap(),
                };
                return as_rec(d, &r, t);
            }
        }
        conform_s(d, v.clone(), "")
    })();
    match r {
        Ok(x) => Ok(Some(x)),
        Err(Ctl::Type(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
fn as_rec(d: &Ty, r: &Ty, t: &str) -> R {
    let st = t.trim_start();
    let data = if st.starts_with('{') || st.starts_with('[') { jparse(t)? } else { form_parse(t)? };
    let x = from_json(r, &data, true)?;
    conform_s(d, x, "")
}
pub fn as_strict(d: &Ty, v: V, src: &str) -> R {
    match as_(d, v.clone())? {
        Some(x) => Ok(x),
        None => terr(format!("{} as {}: wartość nie pasuje do typu", show(&v), src)),
    }
}

fn build(n: &'static str, fs: &Fields, mut given: Vec<(&'static str, V)>) -> Result<Vec<(&'static str, V)>, Ctl> {
    let mut out = Vec::with_capacity(fs.len());
    for (f, fd) in fs.iter() {
        let Some(i) = given.iter().position(|(k, _)| k == f) else {
            return terr(format!("{}: brak pola {}", n, f));
        };
        let x = given.swap_remove(i).1;
        out.push((*f, conform(fd, x, &Wh::Field(n, f))?));
    }
    if let Some((k, _)) = given.first() {
        return terr(format!("{} nie ma pola {}", n, k));
    }
    Ok(out)
}
pub fn mk(n: &'static str, given: Vec<(&'static str, V)>) -> R {
    let t = lookup(n)?;
    let Ty::Rec(_, fs) = &*t else { return terr(format!("{} nie jest rekordem", n)) };
    Ok(V::Rec(Rc::new(Obj { n, f: build(n, fs, given)? })))
}
pub fn mkv(n: &'static str, given: Vec<(&'static str, V)>) -> R {
    let Some(fs) = vfields(n) else { return terr(format!("{} nie ma pól", n)) };
    Ok(V::Var(Rc::new(Obj { n, f: build(n, &fs, given)? })))
}
// Pozostałe pola były sprawdzone przy budowie rekordu, więc sprawdza tylko zmienione.
// Rekord bez innych referencji zmienia w miejscu.
pub fn with(v: V, upd: Vec<(&'static str, V)>) -> R {
    let (mut o, rec) = match v {
        V::Rec(o) => (o, true),
        V::Var(o) if !o.f.is_empty() => (o, false),
        v => return terr(format!("with działa na rekordzie, a dostał {}", show(&v))),
    };
    let n = o.n;
    let (t, vf);
    let fs: &Fields = if rec {
        t = lookup(n)?;
        match &*t {
            Ty::Rec(_, fs) => fs,
            _ => return terr(format!("{} nie jest rekordem", n)),
        }
    } else {
        vf = vfields(n);
        match &vf {
            Some(fs) => fs,
            None => return terr(format!("{} nie ma pól", n)),
        }
    };
    let obj = Rc::make_mut(&mut o);
    for (k, x) in upd {
        let Some(i) = obj.f.iter().position(|(f, _)| *f == k) else { return terr(format!("{} nie ma pola {}", n, k)) };
        let Some((_, fd)) = fs.iter().find(|(f, _)| *f == k) else { return terr(format!("{} nie ma pola {}", n, k)) };
        obj.f[i].1 = conform(fd, x, &Wh::Field(n, k))?;
    }
    Ok(if rec { V::Rec(o) } else { V::Var(o) })
}

// Ostatni odczyt zmiennej: wartość przechodzi dalej bez klonu (zob. moves.rs w kompilatorze).
#[inline]
pub fn mv(x: &mut V) -> V {
    std::mem::replace(x, V::Unit)
}

pub fn field(o: V, name: &str) -> R {
    field_ref(&o, name)
}

pub fn field_ref(o: &V, name: &str) -> R {
    let got = match o {
        V::Date(y, m, d) => match name {
            "year" => Some(V::Int(*y)),
            "month" => Some(V::Int(*m)),
            "day" => Some(V::Int(*d)),
            _ => None,
        },
        V::DT(y, mo, d, h, mi, s) => match name {
            "year" => Some(V::Int(*y)),
            "month" => Some(V::Int(*mo)),
            "day" => Some(V::Int(*d)),
            "hour" => Some(V::Int(*h)),
            "minute" => Some(V::Int(*mi)),
            "second" => Some(V::Int(*s)),
            _ => None,
        },
        V::Rec(x) | V::Var(x) => x.get(name).cloned(),
        _ => None,
    };
    match got {
        Some(v) => Ok(v),
        None => terr(format!("{} nie ma pola {}", show(o), name)),
    }
}

pub fn isv(v: &V, n: &str) -> bool {
    matches!(v, V::Var(o) if o.n == n)
}
pub fn is_str(v: &V, lit: &str) -> bool {
    matches!(v, V::Str(s) if &**s == lit)
}
pub fn list_len(v: &V) -> Option<usize> {
    match v {
        V::List(xs) => Some(xs.len()),
        _ => None,
    }
}
pub fn idx(v: &V, i: usize) -> V {
    match v {
        V::List(xs) => xs.get(i).cloned().unwrap_or(V::Unit),
        _ => V::Unit,
    }
}
pub fn nomatch(vals: Vec<V>, file: &str, line: usize) -> Ctl {
    Ctl::Type(format!(
        "{}:{}: match bez pasującej gałęzi dla {}",
        file,
        line,
        vals.iter().map(show).collect::<Vec<_>>().join(", ")
    ))
}
pub fn orblock_end() -> Ctl {
    Ctl::Type("blok po `or` musi zakończyć się return".into())
}
pub fn apply(f: &V, x: V) -> R {
    match f {
        V::Fn(f) => f(x),
        _ => terr(format!("{} nie jest funkcją", show(f))),
    }
}

// ---------- JSON i formularze ----------

// JSON z liczbami jako tekst, żeby kwoty nie traciły groszy.
pub enum J {
    Null,
    Bool(bool),
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(kv) => kv.iter().rev().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
}

fn j_text(j: &J) -> String {
    match j {
        J::Null => "null".into(),
        J::Bool(b) => b.to_string(),
        J::Num(t) => t.clone(),
        J::Str(s) => js_quote(s),
        J::Arr(a) => format!("[{}]", a.iter().map(j_text).collect::<Vec<_>>().join(",")),
        J::Obj(kv) => format!("{{{}}}", kv.iter().map(|(k, v)| format!("{}:{}", js_quote(k), j_text(v))).collect::<Vec<_>>().join(",")),
    }
}

pub fn json(x: &V) -> Result<String, Ctl> {
    let mut o = String::new();
    json_into(&mut o, x)?;
    Ok(o)
}
fn json_into(o: &mut String, x: &V) -> Result<(), Ctl> {
    match x {
        V::Str(s) => o.push_str(&js_quote(s)),
        V::Int(n) => {
            let _ = write!(o, "{}", n);
        }
        V::Bool(b) => {
            let _ = write!(o, "{}", b);
        }
        V::Dec(m) => o.push_str(&dec_text(*m, 2, None)),
        V::Date(y, m, d) => o.push_str(&js_quote(&date_text(*y, *m, *d))),
        V::DT(y, mo, d, h, mi, s) => o.push_str(&js_quote(&dt_text(*y, *mo, *d, *h, *mi, *s, 'T'))),
        V::Html(h) => o.push_str(&js_quote(h)),
        V::List(xs) => {
            o.push('[');
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                json_into(o, x)?;
            }
            o.push(']');
        }
        V::Rec(r) => {
            o.push('{');
            for (i, (k, v)) in r.f.iter().enumerate() {
                if i > 0 {
                    o.push(',');
                }
                o.push_str(&js_quote(k));
                o.push(':');
                json_into(o, v)?;
            }
            o.push('}');
        }
        V::Var(r) if r.f.is_empty() => o.push_str(&js_quote(r.n)),
        V::Var(r) => {
            o.push_str("{\"$v\":");
            o.push_str(&js_quote(r.n));
            for (k, v) in r.f.iter() {
                o.push(',');
                o.push_str(&js_quote(k));
                o.push(':');
                json_into(o, v)?;
            }
            o.push('}');
        }
        x => return terr(format!("nie da się zapisać w JSON: {}", show(x))),
    }
    Ok(())
}

pub fn jparse(s: &str) -> Result<J, Ctl> {
    struct Pz<'a> {
        b: &'a [u8],
        s: &'a str,
        i: usize,
    }
    impl Pz<'_> {
        fn bad<X>(&self) -> Result<X, Ctl> {
            terr(format!("niepoprawny JSON na pozycji {}", self.i))
        }
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
                self.i += 1;
            }
        }
        fn val(&mut self) -> Result<J, Ctl> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut kv: Vec<(String, J)> = vec![];
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(J::Obj(kv));
                    }
                    loop {
                        self.ws();
                        if self.b.get(self.i) != Some(&b'"') {
                            return self.bad();
                        }
                        let k = self.str()?;
                        self.ws();
                        if self.b.get(self.i) != Some(&b':') {
                            return self.bad();
                        }
                        self.i += 1;
                        let v = self.val()?;
                        kv.retain(|(n, _)| *n != k);
                        kv.push((k, v));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(J::Obj(kv));
                            }
                            _ => return self.bad(),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = vec![];
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(J::Arr(a));
                    }
                    loop {
                        a.push(self.val()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(J::Arr(a));
                            }
                            _ => return self.bad(),
                        }
                    }
                }
                Some(b'"') => Ok(J::Str(self.str()?)),
                Some(c) if *c == b'-' || c.is_ascii_digit() => {
                    let st = self.i;
                    let b = self.b;
                    let mut i = self.i;
                    if b[i] == b'-' {
                        i += 1;
                    }
                    let d0 = i;
                    while i < b.len() && b[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == d0 {
                        return self.bad();
                    }
                    if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
                        i += 1;
                        while i < b.len() && b[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                        let mut k = i + 1;
                        if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                            k += 1;
                        }
                        if k < b.len() && b[k].is_ascii_digit() {
                            while k < b.len() && b[k].is_ascii_digit() {
                                k += 1;
                            }
                            i = k;
                        }
                    }
                    self.i = i;
                    Ok(J::Num(self.s[st..i].to_string()))
                }
                _ => {
                    for (w, v) in [("true", Some(true)), ("false", Some(false)), ("null", None)] {
                        if self.s[self.i..].starts_with(w) {
                            self.i += w.len();
                            return Ok(match v {
                                Some(b) => J::Bool(b),
                                None => J::Null,
                            });
                        }
                    }
                    self.bad()
                }
            }
        }
        fn hex4(&mut self) -> Result<u32, Ctl> {
            let h = self.s.get(self.i..self.i + 4).and_then(|h| u32::from_str_radix(h, 16).ok());
            match h {
                Some(h) => {
                    self.i += 4;
                    Ok(h)
                }
                None => self.bad(),
            }
        }
        fn str(&mut self) -> Result<String, Ctl> {
            self.i += 1;
            let mut out = String::new();
            loop {
                let Some(&c) = self.b.get(self.i) else { return self.bad() };
                match c {
                    b'"' => {
                        self.i += 1;
                        return Ok(out);
                    }
                    b'\\' => {
                        self.i += 1;
                        let Some(&e) = self.b.get(self.i) else { return self.bad() };
                        self.i += 1;
                        match e {
                            b'"' => out.push('"'),
                            b'\\' => out.push('\\'),
                            b'/' => out.push('/'),
                            b'b' => out.push('\u{8}'),
                            b'f' => out.push('\u{c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let mut h = self.hex4()?;
                                if (0xD800..0xDC00).contains(&h) && self.s[self.i..].starts_with("\\u") {
                                    self.i += 2;
                                    let l = self.hex4()?;
                                    h = 0x10000 + ((h - 0xD800) << 10) + (l.wrapping_sub(0xDC00) & 0x3FF);
                                }
                                out.push(char::from_u32(h).unwrap_or('\u{FFFD}'));
                            }
                            _ => return self.bad(),
                        }
                    }
                    c if c < 0x20 => return self.bad(),
                    _ => {
                        let st = self.i;
                        while self.i < self.b.len() && !matches!(self.b[self.i], b'"' | b'\\') && self.b[self.i] >= 0x20 {
                            self.i += 1;
                        }
                        out.push_str(&self.s[st..self.i]);
                    }
                }
            }
        }
    }
    let mut p = Pz { b: s.as_bytes(), s, i: 0 };
    let v = p.val()?;
    p.ws();
    if p.i != s.len() {
        return p.bad();
    }
    Ok(v)
}

fn pct_decode(x: &str) -> Result<String, Ctl> {
    let bad = || terr("niepoprawne kodowanie formularza");
    let b = x.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' => {
                let h = x.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok());
                match h {
                    Some(h) => out.push(h),
                    None => return bad(),
                }
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8(out).or_else(|_| bad())
}

fn jobj_slot<'a>(o: &'a mut J, k: &str) -> &'a mut J {
    let J::Obj(kv) = o else { unreachable!() };
    let i = match kv.iter().position(|(n, _)| n == k) {
        Some(i) => i,
        None => {
            kv.push((k.to_string(), J::Null));
            kv.len() - 1
        }
    };
    &mut kv[i].1
}

// buyer_name=Firma&lines[0].name=Usługa → {buyer_name: "Firma", lines: {"0": {name: "Usługa"}}}
pub fn form_parse(s: &str) -> Result<J, Ctl> {
    let mut root = J::Obj(vec![]);
    for pair in s.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (k, v) = match pair.find('=') {
            Some(e) => (&pair[..e], pct_decode(&pair[e + 1..])?),
            None => (pair, String::new()),
        };
        let key = pct_decode(k)?;
        let mut path: Vec<String> = vec![];
        let kc: Vec<char> = key.chars().collect();
        let mut i = 0;
        while i < kc.len() {
            let c = kc[i];
            if c != '.' && c != '[' && c != ']' {
                let st = i;
                while i < kc.len() && !matches!(kc[i], '.' | '[' | ']') {
                    i += 1;
                }
                path.push(kc[st..i].iter().collect());
                continue;
            }
            if c == '[' {
                let mut j = i + 1;
                while j < kc.len() && kc[j].is_ascii_digit() {
                    j += 1;
                }
                if j > i + 1 && j < kc.len() && kc[j] == ']' {
                    let n: String = kc[i + 1..j].iter().collect();
                    path.push(n.trim_start_matches('0').to_string().chars().next().map_or("0".into(), |_| n.trim_start_matches('0').to_string()));
                    i = j + 1;
                    continue;
                }
            }
            i += 1;
        }
        if path.is_empty() {
            continue;
        }
        let mut o = &mut root;
        for p in &path[..path.len() - 1] {
            let slot = jobj_slot(o, p);
            if !matches!(slot, J::Obj(_) | J::Arr(_)) {
                *slot = J::Obj(vec![]);
            }
            if let J::Arr(_) = slot {
                *slot = J::Obj(vec![]);
            }
            o = slot;
        }
        *jobj_slot(o, path.last().unwrap()) = J::Str(v);
    }
    Ok(root)
}

// Obiekt z kluczami 0, 1, ... z formularza albo tablica z JSON.
fn as_array(j: &J) -> Option<Vec<&J>> {
    match j {
        J::Arr(a) => Some(a.iter().collect()),
        J::Obj(kv) => {
            if !kv.iter().all(|(k, _)| !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit())) {
                return None;
            }
            let mut ks: Vec<(u64, &J)> = kv.iter().map(|(k, v)| (k.parse::<u64>().unwrap_or(u64::MAX), v)).collect();
            ks.sort_by_key(|(k, _)| *k);
            Some(ks.into_iter().map(|(_, v)| v).collect())
        }
        _ => None,
    }
}

fn j_any(j: &J) -> V {
    match j {
        J::Null => V::Unit,
        J::Bool(b) => V::Bool(*b),
        J::Num(t) => t.parse::<i64>().map(V::Int).unwrap_or_else(|_| dec_lit(t)),
        J::Str(x) => s(x),
        J::Arr(a) => list(a.iter().map(j_any).collect()),
        J::Obj(_) => V::Unit,
    }
}

// `form`: dane z formularza, gdzie liczby przychodzą jako tekst.
fn from_json(d: &Ty, j: &J, form: bool) -> R {
    let bad = || -> R { terr(format!("dane nie pasują do {}: {}", tname(d), j_text(j))) };
    match d {
        Ty::Any => Ok(j_any(j)),
        Ty::Prim(p) => {
            let text = match j {
                J::Num(t) => Some(t.as_str()),
                J::Str(t) if form => Some(t.as_str()),
                _ => None,
            };
            match p {
                P::Int => match text {
                    Some(t) if !t.is_empty() && t.strip_prefix('-').unwrap_or(t).bytes().all(|b| b.is_ascii_digit()) && t != "-" => {
                        int(t.parse::<i64>().ok())
                    }
                    _ => bad(),
                },
                P::Money => match j {
                    J::Num(t) | J::Str(t) => dec(t),
                    _ => bad(),
                },
                P::String => match j {
                    J::Str(t) => Ok(s(t)),
                    _ => bad(),
                },
                P::Bool => match j {
                    J::Bool(b) => Ok(V::Bool(*b)),
                    _ => bad(),
                },
                P::Date => match j {
                    J::Str(t) => parse_date(t),
                    _ => bad(),
                },
                P::DateTime => match j {
                    J::Str(t) => parse_dt(t),
                    _ => bad(),
                },
                P::Html => bad(),
            }
        }
        Ty::List(e) => {
            let Some(a) = as_array(j) else { return bad() };
            let mut out = Vec::with_capacity(a.len());
            for x in a {
                out.push(from_json(e, x, form)?);
            }
            Ok(list(out))
        }
        Ty::Ref(n) => from_json(&*lookup(n)?, j, form),
        Ty::Named(_, t) => from_json(t, j, form),
        Ty::Refine(b, ..) => {
            let x = from_json(b, j, form)?;
            conform_s(d, x, "")
        }
        Ty::Rec(n, fs) => {
            if !matches!(j, J::Obj(_)) {
                return bad();
            }
            let mut out = Vec::with_capacity(fs.len());
            for (f, fd) in fs.iter() {
                match j.get(f) {
                    None if target_list(fd) => out.push((*f, list(vec![]))),
                    None => return terr(format!("{}: brak pola {}", n, f)),
                    Some(x) => out.push((*f, from_json(fd, x, form)?)),
                }
            }
            mk(n, out)
        }
        Ty::Var(n) => {
            let Some(fs) = vfields(n) else {
                return match j {
                    J::Str(t) if t == n => Ok(vsing(n)),
                    _ => bad(),
                };
            };
            match j.get("$v") {
                Some(J::Str(t)) if t == n => {}
                _ => return bad(),
            }
            let mut out = Vec::with_capacity(fs.len());
            for (f, fd) in fs.iter() {
                out.push((*f, from_json(fd, j.get(f).unwrap_or(&J::Null), form)?));
            }
            mkv(n, out)
        }
        Ty::Union(a) => {
            for t in a {
                match from_json(t, j, form) {
                    Err(Ctl::Type(_)) => continue,
                    r => return r,
                }
            }
            bad()
        }
        Ty::Cap(_) => bad(),
    }
}

// ---------- wyrażenia regularne ----------

// Mały silnik z nawrotami: znaki, ., klasy [...] z zakresami i negacją, \d \w \s (i wielkie),
// kotwice ^ $, grupy (...) z |, kwantyfikatory ? * + {n} {n,} {n,m}.
enum Cl {
    R(char, char),
    D(bool),
    W(bool),
    Sp(bool),
}
enum Node {
    Ch(char),
    Any,
    Class(Vec<Cl>, bool),
    Start,
    End,
    Group(Vec<Vec<Piece>>),
}
struct Piece {
    node: Node,
    min: usize,
    max: usize,
}
pub struct Regex(Vec<Vec<Piece>>);

fn is_space(c: char) -> bool {
    c.is_whitespace() || c == '\u{feff}'
}
fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}
fn cl_match(c: char, it: &Cl) -> bool {
    match it {
        Cl::R(a, b) => *a <= c && c <= *b,
        Cl::D(n) => c.is_ascii_digit() != *n,
        Cl::W(n) => is_word(c) != *n,
        Cl::Sp(n) => is_space(c) != *n,
    }
}

impl Regex {
    pub fn new(src: &str) -> Result<Regex, String> {
        let cs: Vec<char> = src.chars().collect();
        let mut i = 0;
        let alts = Self::alts(&cs, &mut i)?;
        if i != cs.len() {
            return Err(format!("niepoprawne wyrażenie {}", src));
        }
        Ok(Regex(alts))
    }
    fn esc(c: char) -> Option<Cl> {
        match c {
            'd' => Some(Cl::D(false)),
            'D' => Some(Cl::D(true)),
            'w' => Some(Cl::W(false)),
            'W' => Some(Cl::W(true)),
            's' => Some(Cl::Sp(false)),
            'S' => Some(Cl::Sp(true)),
            _ => None,
        }
    }
    fn lit_esc(c: char) -> char {
        match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            c => c,
        }
    }
    fn alts(cs: &[char], i: &mut usize) -> Result<Vec<Vec<Piece>>, String> {
        let mut alts = vec![vec![]];
        while *i < cs.len() {
            let c = cs[*i];
            let node = match c {
                '|' => {
                    *i += 1;
                    alts.push(vec![]);
                    continue;
                }
                ')' => break,
                '(' => {
                    *i += 1;
                    if cs.get(*i) == Some(&'?') && cs.get(*i + 1) == Some(&':') {
                        *i += 2;
                    }
                    let g = Self::alts(cs, i)?;
                    if cs.get(*i) != Some(&')') {
                        return Err("brak )".into());
                    }
                    *i += 1;
                    Node::Group(g)
                }
                '[' => {
                    *i += 1;
                    let negd = cs.get(*i) == Some(&'^');
                    if negd {
                        *i += 1;
                    }
                    let mut items = vec![];
                    let mut first = true;
                    loop {
                        let Some(&c) = cs.get(*i) else { return Err("brak ]".into()) };
                        if c == ']' && !first {
                            *i += 1;
                            break;
                        }
                        first = false;
                        let lo = if c == '\\' {
                            *i += 1;
                            let e = *cs.get(*i).ok_or("zły \\")?;
                            *i += 1;
                            if let Some(cl) = Self::esc(e) {
                                items.push(cl);
                                continue;
                            }
                            Self::lit_esc(e)
                        } else {
                            *i += 1;
                            c
                        };
                        if cs.get(*i) == Some(&'-') && cs.get(*i + 1).is_some_and(|c| *c != ']') {
                            let mut hi = cs[*i + 1];
                            *i += 2;
                            if hi == '\\' {
                                hi = Self::lit_esc(*cs.get(*i).ok_or("zły \\")?);
                                *i += 1;
                            }
                            items.push(Cl::R(lo, hi));
                        } else {
                            items.push(Cl::R(lo, lo));
                        }
                    }
                    Node::Class(items, negd)
                }
                '\\' => {
                    let e = *cs.get(*i + 1).ok_or("zły \\")?;
                    *i += 2;
                    match Self::esc(e) {
                        Some(cl) => Node::Class(vec![cl], false),
                        None => Node::Ch(Self::lit_esc(e)),
                    }
                }
                '.' => {
                    *i += 1;
                    Node::Any
                }
                '^' => {
                    *i += 1;
                    Node::Start
                }
                '$' => {
                    *i += 1;
                    Node::End
                }
                c => {
                    *i += 1;
                    Node::Ch(c)
                }
            };
            let (mut min, mut max) = (1, 1);
            match cs.get(*i) {
                Some('?') => {
                    (min, max) = (0, 1);
                    *i += 1;
                }
                Some('*') => {
                    (min, max) = (0, usize::MAX);
                    *i += 1;
                }
                Some('+') => {
                    (min, max) = (1, usize::MAX);
                    *i += 1;
                }
                Some('{') => {
                    let mut j = *i + 1;
                    let num = |j: &mut usize| {
                        let st = *j;
                        while *j < cs.len() && cs[*j].is_ascii_digit() {
                            *j += 1;
                        }
                        cs[st..*j].iter().collect::<String>().parse::<usize>().ok()
                    };
                    if let Some(a) = num(&mut j) {
                        let b = if cs.get(j) == Some(&',') {
                            j += 1;
                            num(&mut j).unwrap_or(usize::MAX)
                        } else {
                            a
                        };
                        if cs.get(j) == Some(&'}') {
                            (min, max) = (a, b);
                            *i = j + 1;
                        }
                    }
                }
                _ => {}
            }
            if *i < cs.len() && cs[*i] == '?' && (min, max) != (1, 1) {
                *i += 1;
            }
            alts.last_mut().unwrap().push(Piece { node, min, max });
        }
        Ok(alts)
    }

    pub fn test(&self, s: &str) -> bool {
        let cs: Vec<char> = s.chars().collect();
        (0..=cs.len()).any(|st| self.0.iter().any(|a| m_seq(a, &cs, st, &|_| true)))
    }
}

fn m_seq(p: &[Piece], s: &[char], i: usize, k: &dyn Fn(usize) -> bool) -> bool {
    match p.split_first() {
        None => k(i),
        Some((pc, rest)) => m_rep(pc, rest, s, i, 0, k),
    }
}
fn m_rep(pc: &Piece, rest: &[Piece], s: &[char], i: usize, count: usize, k: &dyn Fn(usize) -> bool) -> bool {
    if count < pc.max && m_node(&pc.node, s, i, &|j| !(j == i && count >= pc.min) && m_rep(pc, rest, s, j, count + 1, k)) {
        return true;
    }
    count >= pc.min && m_seq(rest, s, i, k)
}
fn m_node(n: &Node, s: &[char], i: usize, k: &dyn Fn(usize) -> bool) -> bool {
    match n {
        Node::Ch(c) => i < s.len() && s[i] == *c && k(i + 1),
        Node::Any => i < s.len() && !matches!(s[i], '\n' | '\r' | '\u{2028}' | '\u{2029}') && k(i + 1),
        Node::Class(items, negd) => i < s.len() && items.iter().any(|it| cl_match(s[i], it)) != *negd && k(i + 1),
        Node::Start => i == 0 && k(i),
        Node::End => i == s.len() && k(i),
        Node::Group(alts) => alts.iter().any(|a| m_seq(a, s, i, k)),
    }
}

thread_local! {
    static REGEXES: RefCell<HashMap<String, Rc<Regex>>> = RefCell::new(HashMap::new());
}
fn regex(src: &str) -> Result<Rc<Regex>, Ctl> {
    if let Some(r) = REGEXES.with(|m| m.borrow().get(src).cloned()) {
        return Ok(r);
    }
    let r = Rc::new(Regex::new(src).map_err(Ctl::Type)?);
    REGEXES.with(|m| m.borrow_mut().insert(src.to_string(), r.clone()));
    Ok(r)
}

// ---------- funkcje wbudowane ----------

fn st(x: &V, f: &str) -> Result<Rc<str>, Ctl> {
    match x {
        V::Str(s) => Ok(s.clone()),
        _ => terr(format!("{}: oczekiwano String, jest {}", f, show(x))),
    }
}
fn only_digits(t: &str) -> bool {
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

pub fn b_segments(p: V) -> R {
    let p = st(&p, "segments")?;
    Ok(list(p.split('/').filter(|x| !x.is_empty()).map(s).collect()))
}
pub fn b_parse_int(x: V) -> R {
    let t = st(&x, "parse_int")?;
    let t = t.trim();
    let body = t.strip_prefix('-').unwrap_or(t);
    if !only_digits(body) {
        return Ok(vsing("NotANumber"));
    }
    match t.parse::<i64>() {
        Ok(n) if (-MAX_SAFE..=MAX_SAFE).contains(&n) => Ok(V::Int(n)),
        _ => Ok(vsing("NotANumber")),
    }
}
pub fn b_parse_money(x: V) -> R {
    let t = st(&x, "parse_money")?;
    let t = t.trim().replacen(',', ".", 1);
    let body = t.strip_prefix('-').unwrap_or(&t);
    let ok = match body.split_once('.') {
        Some((a, b)) => only_digits(a) && only_digits(b),
        None => only_digits(body),
    };
    if !ok {
        return Ok(vsing("NotANumber"));
    }
    dec(&t)
}
pub fn b_to_string(x: V) -> R {
    Ok(s(&disp(&x)))
}
pub fn b_to_json(x: V) -> R {
    Ok(s(&json(&x)?))
}
pub fn b_round(x: V, places: V) -> R {
    match (&x, &places) {
        (V::Dec(m), V::Int(p)) => Ok(V::Dec(round_dec(*m, *p))),
        _ => Ok(x),
    }
}
pub fn b_sum(xs: V) -> R {
    let xs = iter_(xs)?;
    let mut acc = V::Int(0);
    for x in xs.iter() {
        acc = add(acc, x.clone())?;
    }
    Ok(acc)
}
pub fn b_distinct(xs: V) -> R {
    let xs = iter_(xs)?;
    let mut out: Vec<V> = vec![];
    for x in xs.iter() {
        if !out.iter().any(|y| eq(x, y)) {
            out.push(x.clone());
        }
    }
    Ok(list(out))
}
pub fn b_sort_by(xs: V, f: V) -> R {
    let xs = iter_(xs)?;
    let mut keyed = Vec::with_capacity(xs.len());
    for x in xs.iter() {
        keyed.push((apply(&f, x.clone())?, x.clone()));
    }
    let mut err = None;
    keyed.sort_by(|a, b| match cmp(&a.0, &b.0) {
        Ok(c) => c.cmp(&0),
        Err(e) => {
            err.get_or_insert(e);
            std::cmp::Ordering::Equal
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(list(keyed.into_iter().map(|k| k.1).collect()))
}
pub fn b_len(x: V) -> R {
    match &x {
        V::Str(t) => Ok(V::Int(t.chars().count() as i64)),
        V::List(xs) => Ok(V::Int(xs.len() as i64)),
        _ => terr(format!("len działa na tekście i liście, a dostał {}", show(&x))),
    }
}
pub fn b_at(xs: V, i: V) -> R {
    let (V::List(xs), V::Int(i)) = (&xs, &i) else { return terr(format!("at działa na liście i indeksie, a dostał {} i {}", show(&xs), show(&i))) };
    match xs.get(*i as usize) {
        Some(x) if *i >= 0 => Ok(x.clone()),
        _ => terr(format!("at: indeks {} poza listą o długości {}", i, xs.len())),
    }
}
pub fn b_join(xs: V, sep: V) -> R {
    let V::List(xs) = &xs else { return terr(format!("join działa na liście, a dostał {}", show(&xs))) };
    let sep = st(&sep, "join")?;
    let mut out = String::new();
    for (k, x) in xs.iter().enumerate() {
        if k > 0 {
            out.push_str(&sep);
        }
        out.push_str(&st(x, "join")?);
    }
    Ok(s(&out))
}
pub fn b_split(x: V, sep: V) -> R {
    let (t, p) = (st(&x, "split")?, st(&sep, "split")?);
    if p.is_empty() {
        return Ok(list(t.encode_utf16().map(|u| s(&String::from_utf16_lossy(&[u]))).collect()));
    }
    Ok(list(t.split(&*p).map(s).collect()))
}
pub fn b_chars(x: V) -> R {
    let t = st(&x, "chars")?;
    Ok(list(t.chars().map(|c| V::Str(Rc::from(c.encode_utf8(&mut [0; 4]) as &str))).collect()))
}
pub fn b_char(code: V) -> R {
    let V::Int(n) = code else { return terr("char: oczekiwano Int") };
    match u32::try_from(n).ok().and_then(char::from_u32) {
        Some(c) => Ok(s(c.encode_utf8(&mut [0; 4]))),
        None => terr(format!("char: {} nie jest kodem znaku", n)),
    }
}
pub fn b_trim(x: V) -> R {
    Ok(s(st(&x, "trim")?.trim_matches(is_space)))
}
pub fn b_lower(x: V) -> R {
    Ok(s(&st(&x, "lower")?.to_lowercase()))
}
pub fn b_upper(x: V) -> R {
    Ok(s(&st(&x, "upper")?.to_uppercase()))
}
pub fn b_remove(x: V, part: V) -> R {
    let (t, p) = (st(&x, "remove")?, st(&part, "remove")?);
    if p.is_empty() {
        return Ok(x);
    }
    Ok(s(&t.replace(&*p, "")))
}
pub fn b_drop_prefix(x: V, p: V) -> R {
    let (t, p) = (st(&x, "drop_prefix")?, st(&p, "drop_prefix")?);
    match t.strip_prefix(&*p) {
        Some(r) => Ok(s(r)),
        None => Ok(x),
    }
}
pub fn b_pad_left(x: V, n: V, ch: V) -> R {
    let (t, c) = (st(&x, "pad_left")?, st(&ch, "pad_left")?);
    let V::Int(n) = n else { return terr("pad_left: oczekiwano Int") };
    let have = t.encode_utf16().count() as i64;
    if have >= n || c.is_empty() {
        return Ok(x);
    }
    let fill: Vec<u16> = c.encode_utf16().cycle().take((n - have) as usize).collect();
    let mut out = String::from_utf16_lossy(&fill);
    out.push_str(&t);
    Ok(s(&out))
}
pub fn b_starts_with(x: V, p: V) -> R {
    Ok(V::Bool(st(&x, "starts_with")?.starts_with(&*st(&p, "starts_with")?)))
}
pub fn b_contains(x: V, part: V) -> R {
    match &x {
        V::List(xs) => Ok(V::Bool(xs.iter().any(|y| eq(y, &part)))),
        _ => Ok(V::Bool(st(&x, "contains")?.contains(&*st(&part, "contains")?))),
    }
}
pub fn b_matches(x: V, re: V) -> R {
    let t = st(&x, "matches")?;
    Ok(V::Bool(regex(&st(&re, "matches")?)?.test(&t)))
}
pub fn b_only_digits(x: V) -> R {
    Ok(V::Bool(only_digits(&st(&x, "only_digits")?)))
}
pub fn b_nip_checksum_ok(x: V) -> R {
    let t = st(&x, "nip_checksum_ok")?;
    let b = t.as_bytes();
    if b.len() != 10 || !only_digits(&t) {
        return Ok(V::Bool(false));
    }
    let w = [6, 5, 7, 2, 3, 4, 5, 6, 7];
    let sum: u32 = w.iter().zip(b).map(|(w, d)| w * (d - b'0') as u32).sum();
    Ok(V::Bool(sum % 11 == (b[9] - b'0') as u32))
}
// ^[^\s@]+@[^\s@]+\.[^\s@]+$
pub fn b_valid_email(x: V) -> R {
    let t = st(&x, "valid_email")?;
    if t.chars().any(is_space) {
        return Ok(V::Bool(false));
    }
    let mut parts = t.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return Ok(V::Bool(false));
    };
    let dc: Vec<char> = domain.chars().collect();
    let dot = dc.len() >= 3 && dc[1..dc.len() - 1].contains(&'.');
    Ok(V::Bool(!local.is_empty() && dot))
}

// ---------- uprawnienia ----------

pub struct DbShared {
    db: *mut Sqlite3,
    get: *mut Stmt3,
    all: *mut Stmt3,
    save: *mut Stmt3,
    sp: Cell<u32>,
}

pub enum Cap {
    Db(Rc<DbShared>, u32),
    Clock(Option<V>),
    Http(Option<fn(V) -> R>),
    Random(RefCell<Xoshiro>),
    Terminal,
}
impl Cap {
    fn name(&self) -> &'static str {
        match self {
            Cap::Db(..) => "Db",
            Cap::Clock(_) => "Clock",
            Cap::Http(_) => "Http",
            Cap::Random(_) => "Random",
            Cap::Terminal => "Terminal",
        }
    }
}

// Random: xoshiro256++ z ziarnem rozwiniętym przez SplitMix64, ten sam co w runtime.js i w ttfx.
pub struct Xoshiro([u64; 4]);
impl Xoshiro {
    fn seeded(seed: u64) -> Xoshiro {
        let mut sm = seed;
        let mut next = || {
            sm = sm.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = sm;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^ (z >> 31)
        };
        Xoshiro([next(), next(), next(), next()])
    }
    fn next(&mut self) -> u64 {
        let s = &mut self.0;
        let r = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        r
    }
    fn below(&mut self, n: u64) -> u64 {
        let bits = 64 - (n - 1).leading_zeros();
        loop {
            let r = self.next() >> (64 - bits.max(1));
            if r < n {
                return r;
            }
        }
    }
}

// Terminal: stdout z buforem, opróżnianym co 64 KB, przy exit i na końcu run_main.
thread_local! {
    static OUT: RefCell<String> = RefCell::new(String::with_capacity(1 << 17));
}
fn out_flush() {
    use std::io::Write;
    OUT.with(|o| {
        let mut o = o.borrow_mut();
        let _ = std::io::stdout().lock().write_all(o.as_bytes());
        o.clear();
    });
}
fn out_write(t: &str) {
    let full = OUT.with(|o| {
        let mut o = o.borrow_mut();
        o.push_str(t);
        o.len() > 1 << 16
    });
    if full {
        out_flush();
    }
}

#[repr(C)]
pub struct Sqlite3 {
    _p: [u8; 0],
}
#[repr(C)]
pub struct Stmt3 {
    _p: [u8; 0],
}
#[link(name = "sqlite3")]
unsafe extern "C" {
    fn sqlite3_open(filename: *const c_char, db: *mut *mut Sqlite3) -> c_int;
    fn sqlite3_close(db: *mut Sqlite3) -> c_int;
    fn sqlite3_exec(db: *mut Sqlite3, sql: *const c_char, cb: *const u8, arg: *mut u8, err: *mut *mut c_char) -> c_int;
    fn sqlite3_prepare_v2(db: *mut Sqlite3, sql: *const c_char, n: c_int, st: *mut *mut Stmt3, tail: *mut *const c_char) -> c_int;
    fn sqlite3_bind_text(st: *mut Stmt3, i: c_int, s: *const c_char, n: c_int, destructor: isize) -> c_int;
    fn sqlite3_step(st: *mut Stmt3) -> c_int;
    fn sqlite3_column_text(st: *mut Stmt3, i: c_int) -> *const c_uchar;
    fn sqlite3_column_bytes(st: *mut Stmt3, i: c_int) -> c_int;
    fn sqlite3_reset(st: *mut Stmt3) -> c_int;
    fn sqlite3_finalize(st: *mut Stmt3) -> c_int;
    fn sqlite3_errmsg(db: *mut Sqlite3) -> *const c_char;
}
const SQLITE_ROW: c_int = 100;
const SQLITE_DONE: c_int = 101;
const SQLITE_TRANSIENT: isize = -1;

impl Drop for DbShared {
    fn drop(&mut self) {
        unsafe {
            sqlite3_finalize(self.get);
            sqlite3_finalize(self.all);
            sqlite3_finalize(self.save);
            sqlite3_close(self.db);
        }
    }
}

impl DbShared {
    fn err(&self) -> Ctl {
        Ctl::Db(unsafe { CStr::from_ptr(sqlite3_errmsg(self.db)) }.to_string_lossy().into_owned())
    }
    fn exec(&self, sql: &str) -> Result<(), Ctl> {
        let c = CString::new(sql).unwrap();
        let rc = unsafe { sqlite3_exec(self.db, c.as_ptr(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut()) };
        if rc != 0 { Err(self.err()) } else { Ok(()) }
    }
    fn bind(&self, st: *mut Stmt3, args: &[&str]) {
        for (i, a) in args.iter().enumerate() {
            unsafe { sqlite3_bind_text(st, i as c_int + 1, a.as_ptr() as *const c_char, a.len() as c_int, SQLITE_TRANSIENT) };
        }
    }
    // Wiersze z jedną kolumną tekstu.
    fn rows(&self, st: *mut Stmt3, args: &[&str]) -> Result<Vec<String>, Ctl> {
        self.bind(st, args);
        let mut out = vec![];
        loop {
            let rc = unsafe { sqlite3_step(st) };
            if rc == SQLITE_ROW {
                let t = unsafe {
                    let p = sqlite3_column_text(st, 0);
                    let n = sqlite3_column_bytes(st, 0) as usize;
                    String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned()
                };
                out.push(t);
            } else if rc == SQLITE_DONE {
                break;
            } else {
                let e = self.err();
                unsafe { sqlite3_reset(st) };
                return Err(e);
            }
        }
        unsafe { sqlite3_reset(st) };
        Ok(out)
    }
}

fn mkdb(path: &str) -> Result<V, Ctl> {
    let c = CString::new(path).unwrap();
    let mut db = std::ptr::null_mut();
    if unsafe { sqlite3_open(c.as_ptr(), &mut db) } != 0 {
        return terr(format!("Db: nie da się otworzyć {}", path));
    }
    let mut sh = DbShared {
        db,
        get: std::ptr::null_mut(),
        all: std::ptr::null_mut(),
        save: std::ptr::null_mut(),
        sp: Cell::new(0),
    };
    if path != ":memory:" {
        sh.exec("PRAGMA journal_mode = WAL")?;
    }
    sh.exec("CREATE TABLE IF NOT EXISTS sowa_kv (collection TEXT NOT NULL, key TEXT NOT NULL, value TEXT NOT NULL, PRIMARY KEY (collection, key))")?;
    let prep = |sql: &str| -> Result<*mut Stmt3, Ctl> {
        let c = CString::new(sql).unwrap();
        let mut st = std::ptr::null_mut();
        if unsafe { sqlite3_prepare_v2(db, c.as_ptr(), -1, &mut st, std::ptr::null_mut()) } != 0 {
            return Err(Ctl::Db(unsafe { CStr::from_ptr(sqlite3_errmsg(db)) }.to_string_lossy().into_owned()));
        }
        Ok(st)
    };
    sh.get = prep("SELECT value FROM sowa_kv WHERE collection = ?1 AND key = ?2")?;
    sh.all = prep("SELECT value FROM sowa_kv WHERE collection = ?1 ORDER BY key")?;
    sh.save = prep(
        "INSERT INTO sowa_kv (collection, key, value) VALUES (?1, ?2, ?3) ON CONFLICT (collection, key) DO UPDATE SET value = excluded.value",
    )?;
    Ok(V::Cap(Rc::new(Cap::Db(Rc::new(sh), 0))))
}

#[repr(C)]
struct Tm {
    sec: c_int,
    min: c_int,
    hour: c_int,
    mday: c_int,
    mon: c_int,
    year: c_int,
    wday: c_int,
    yday: c_int,
    isdst: c_int,
    gmtoff: c_long,
    zone: *const c_char,
}
unsafe extern "C" {
    fn time(t: *mut i64) -> i64;
    fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
}
fn now_dt() -> V {
    unsafe {
        let t = time(std::ptr::null_mut());
        let mut tm: Tm = std::mem::zeroed();
        localtime_r(&t, &mut tm);
        V::DT(tm.year as i64 + 1900, tm.mon as i64 + 1, tm.mday as i64, tm.hour as i64, tm.min as i64, tm.sec as i64)
    }
}

pub struct Spec {
    pub ty: &'static str,
    pub url: Option<&'static str>,
    pub now: Option<&'static str>,
    pub fake: Option<fn(V) -> R>,
    pub seed: Option<&'static str>,
    pub seed_env: Option<&'static str>,
}

fn mkres(sp: &Spec) -> R {
    match sp.ty {
        "Db" => match sp.url.unwrap_or("memory") {
            "memory" => mkdb(":memory:"),
            u if u.starts_with("sqlite:") => mkdb(&u["sqlite:".len()..]),
            u => terr(format!("Db: nieobsługiwany adres {} (tylko sqlite:plik i memory)", u)),
        },
        "Clock" => Ok(V::Cap(Rc::new(Cap::Clock(match sp.now {
            Some(t) => Some(parse_dt(t)?),
            None => None,
        })))),
        "Http" => match sp.fake {
            Some(f) => Ok(V::Cap(Rc::new(Cap::Http(Some(f))))),
            None => terr("Http: backend Rust obsługuje w testach tylko atrapę (fake)"),
        },
        "Random" => {
            let text = sp.seed.map(String::from).or_else(|| sp.seed_env.and_then(|k| std::env::var(k).ok()));
            let seed = match text {
                Some(t) => match t.trim().parse::<i128>() {
                    Ok(n) => n as u64,
                    Err(_) => return terr(format!("Random: ziarno {} nie jest liczbą", t)),
                },
                None => std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1),
            };
            Ok(V::Cap(Rc::new(Cap::Random(RefCell::new(Xoshiro::seeded(seed))))))
        }
        "Terminal" => Ok(V::Cap(Rc::new(Cap::Terminal))),
        t => terr(format!("zasób {} nie jest obsługiwany w backendzie Rust", t)),
    }
}

// Metody list i uprawnień.
pub fn call(o: V, name: &str, pos: Vec<V>, named: Vec<(&'static str, V)>, targs: Vec<T>, line: usize) -> R {
    let arg = |i: usize, key: &str| -> V {
        named.iter().find(|(k, _)| *k == key).map(|(_, v)| v.clone()).or_else(|| pos.get(i).cloned()).unwrap_or(V::Unit)
    };
    // Lista bez innych referencji: map zapisuje wyniki w tej samej tablicy.
    let o = match (o, name) {
        (V::List(mut xs), "map") if Rc::strong_count(&xs) == 1 && Rc::weak_count(&xs) == 0 => {
            let f = arg(0, "");
            let out = Rc::get_mut(&mut xs).unwrap();
            for x in out.iter_mut() {
                let y = apply(&f, std::mem::replace(x, V::Unit))?;
                *x = y;
            }
            return Ok(V::List(xs));
        }
        (o, _) => o,
    };
    let nomethod = || terr(format!("linia {}: {} nie ma metody {}", line, show(&o), name));
    match &o {
        V::List(xs) => match name {
            "map" => {
                let f = arg(0, "");
                let mut out = Vec::with_capacity(xs.len());
                for x in xs.iter() {
                    out.push(apply(&f, x.clone())?);
                }
                Ok(list(out))
            }
            "filter" => {
                let f = arg(0, "");
                let mut out = vec![];
                for x in xs.iter() {
                    if bool_(apply(&f, x.clone())?)? {
                        out.push(x.clone());
                    }
                }
                Ok(list(out))
            }
            "reverse" => Ok(list(xs.iter().rev().cloned().collect())),
            _ => nomethod(),
        },
        V::Cap(c) => match (&**c, name) {
            (Cap::Db(sh, _), "get") => {
                let key = disp(&arg(1, "key"));
                let coll = disp(&arg(0, "collection"));
                let rows = sh.rows(sh.get, &[&coll, &key])?;
                let Some(row) = rows.first() else { return Ok(vsing("NoRow")) };
                let t = targs.first().cloned().unwrap_or_else(tany);
                match jparse(row).and_then(|j| from_json(&t, &j, false)) {
                    Err(Ctl::Type(_)) => Ok(vsing("DbError")),
                    r => r,
                }
            }
            (Cap::Db(sh, _), "all") => {
                let coll = disp(&arg(0, "collection"));
                let t = targs.first().cloned().unwrap_or_else(tany);
                let mut out = vec![];
                for row in sh.rows(sh.all, &[&coll])? {
                    out.push(from_json(&t, &jparse(&row)?, false)?);
                }
                Ok(list(out))
            }
            (Cap::Db(sh, _), "save") => {
                let (coll, key, val) = (disp(&arg(0, "collection")), disp(&arg(1, "key")), json(&arg(2, "value"))?);
                sh.rows(sh.save, &[&coll, &key, &val])?;
                Ok(V::Unit)
            }
            (Cap::Db(sh, depth), "transaction") => {
                let f = arg(0, "");
                let n = sh.sp.get() + 1;
                sh.sp.set(n);
                sh.exec(&format!("SAVEPOINT sp{}", n))?;
                let inner = V::Cap(Rc::new(Cap::Db(sh.clone(), depth + 1)));
                match apply(&f, inner) {
                    Ok(r) => {
                        sh.exec(&format!("RELEASE sp{}", n))?;
                        Ok(r)
                    }
                    Err(e) => {
                        sh.exec(&format!("ROLLBACK TO sp{}", n))?;
                        sh.exec(&format!("RELEASE sp{}", n))?;
                        match e {
                            Ctl::Db(_) => Ok(vsing("DbError")),
                            e => Err(e),
                        }
                    }
                }
            }
            (Cap::Clock(now), "now") => Ok(now.clone().unwrap_or_else(now_dt)),
            (Cap::Clock(now), "today") => match now.clone().unwrap_or_else(now_dt) {
                V::DT(y, m, d, ..) => Ok(V::Date(y, m, d)),
                _ => unreachable!(),
            },
            (Cap::Random(g), "int") => {
                let (V::Int(lo), V::Int(hi)) = (arg(0, "min"), arg(1, "max")) else { return terr("random.int: oczekiwano Int") };
                if lo > hi {
                    return terr(format!("random.int: pusty przedział {}..{}", lo, hi));
                }
                Ok(V::Int(lo + g.borrow_mut().below((hi - lo + 1) as u64) as i64))
            }
            (Cap::Random(g), "choice") => match arg(0, "list") {
                V::List(xs) if !xs.is_empty() => Ok(xs[g.borrow_mut().below(xs.len() as u64) as usize].clone()),
                _ => terr("random.choice: pusta lista"),
            },
            (Cap::Terminal, "read") => {
                use std::io::Read;
                let mut t = String::new();
                if let Err(e) = std::io::stdin().read_to_string(&mut t) {
                    return terr(format!("terminal.read: {}", e));
                }
                Ok(s(&t))
            }
            (Cap::Terminal, "write") => {
                out_write(&st(&arg(0, "text"), "terminal.write")?);
                Ok(V::Unit)
            }
            (Cap::Terminal, "exit") => {
                let V::Int(code) = arg(0, "code") else { return terr("terminal.exit: oczekiwano Int") };
                out_flush();
                std::process::exit(code as i32)
            }
            (Cap::Http(Some(fake)), "post" | "get") => {
                let (method, path, body) = if name == "post" { ("Post", arg(0, "path"), arg(1, "body")) } else { ("Get", arg(0, "path"), s("")) };
                fake(mk("HttpRequest", vec![("method", vsing(method)), ("path", path), ("body", body)])?)
            }
            _ => nomethod(),
        },
        _ => nomethod(),
    }
}

// ---------- testy ----------

pub fn check(v: V, src: &str) -> Result<(), Ctl> {
    match v {
        V::Bool(true) => Ok(()),
        v => Err(Ctl::Fail(format!("{}\n    wynik: {}", src, show(&v)))),
    }
}
pub fn check_eq(a: V, b: V, src: &str) -> Result<(), Ctl> {
    if eq(&a, &b) {
        return Ok(());
    }
    Err(Ctl::Fail(format!("{}\n    lewa strona:  {}\n    prawa strona: {}", src, show(&a), show(&b))))
}
pub fn check_is(a: V, d: &Ty, negd: bool, src: &str) -> Result<(), Ctl> {
    if is(d, &a) == negd {
        return Err(Ctl::Fail(format!("{}\n    wartość: {}", src, show(&a))));
    }
    Ok(())
}

// Ten sam generator co w runtime.js (mulberry32), więc property dostają te same przypadki.
pub struct Rng(u32);
impl Rng {
    fn f(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        (t ^ (t >> 14)) as f64 / 4294967296.0
    }
    fn int(&mut self, lo: i64, hi: i64) -> i64 {
        lo + (self.f() * (hi - lo + 1) as f64).floor() as i64
    }
    fn pick<'a, X>(&mut self, a: &'a [X]) -> &'a X {
        &a[(self.f() * a.len() as f64).floor() as usize]
    }
}
fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for c in s.chars() {
        h = (h ^ c as u32).wrapping_mul(16777619);
    }
    h
}

const STR_POOL: [&str; 11] = [
    "",
    "a",
    "Usługa",
    "Zażółć gęślą jaźń",
    "<script>alert(1)</script>",
    "a&b",
    "\"cudzysłów\"",
    "'",
    " spacja ",
    "FV/2026/0001",
    "1234563218",
];
const CHARS: &str = "abcXYZ019 ąęłśżź<>&\"'/-.@";

thread_local! {
    static CHARV: Vec<char> = CHARS.chars().collect();
}

fn pool_pick(g: &mut Rng) -> String {
    let i = (g.f() * 12.0).floor() as usize;
    if i < STR_POOL.len() { STR_POOL[i].to_string() } else { "x".repeat(201) }
}
fn rand_str(g: &mut Rng, lo: i64, hi: i64) -> String {
    let n = g.int(lo, hi);
    CHARV.with(|cs| (0..n).map(|_| *g.pick(cs)).collect())
}

// Mały generator tekstu z wyrażenia regularnego: znaki, klasy [...], \d, kwantyfikatory {n}, {n,}, {n,m}, ?, *, +.
fn gen_re(re: &str, g: &mut Rng) -> String {
    let r: Vec<char> = re.chars().collect();
    let mut i = 0;
    let mut out = String::new();
    if r.first() == Some(&'^') {
        i += 1;
    }
    while i < r.len() {
        if r[i] == '$' && i == r.len() - 1 {
            break;
        }
        let chars: Vec<char>;
        if r[i] == '[' {
            let j = (i..r.len()).find(|&j| r[j] == ']').unwrap_or(r.len());
            let body = &r[i + 1..j];
            let mut cs = vec![];
            let mut k = 0;
            while k < body.len() {
                if body.get(k + 1) == Some(&'-') && k + 2 < body.len() {
                    for c in body[k] as u32..=body[k + 2] as u32 {
                        cs.extend(char::from_u32(c));
                    }
                    k += 3;
                } else {
                    cs.push(body[k]);
                    k += 1;
                }
            }
            chars = cs;
            i = j + 1;
        } else if r[i] == '\\' {
            let c = r.get(i + 1).copied().unwrap_or('\\');
            chars = if c == 'd' { "0123456789".chars().collect() } else { vec![c] };
            i += 2;
        } else {
            chars = vec![r[i]];
            i += 1;
        }
        let (mut lo, mut hi) = (1i64, 1i64);
        let mut parsed = false;
        if r.get(i) == Some(&'{') {
            let mut j = i + 1;
            let st = j;
            while j < r.len() && r[j].is_ascii_digit() {
                j += 1;
            }
            if j > st {
                let a: i64 = r[st..j].iter().collect::<String>().parse().unwrap();
                let (mut b, mut ok) = (a, true);
                if r.get(j) == Some(&',') {
                    j += 1;
                    let s2 = j;
                    while j < r.len() && r[j].is_ascii_digit() {
                        j += 1;
                    }
                    b = if j > s2 { r[s2..j].iter().collect::<String>().parse().unwrap() } else { a + 3 };
                }
                if r.get(j) == Some(&'}') && ok {
                    lo = a;
                    hi = b;
                    i = j + 1;
                    parsed = true;
                }
            }
        }
        if !parsed {
            match r.get(i) {
                Some('?') => {
                    (lo, hi) = (0, 1);
                    i += 1;
                }
                Some('*') => {
                    (lo, hi) = (0, 4);
                    i += 1;
                }
                Some('+') => {
                    (lo, hi) = (1, 4);
                    i += 1;
                }
                _ => {}
            }
        }
        let n = g.int(lo, hi);
        for _ in 0..n {
            out.push(*g.pick(&chars));
        }
    }
    out
}

fn resolve(d: &T) -> Result<T, Ctl> {
    let mut d = d.clone();
    for _ in 0..20 {
        let next = match &*d {
            Ty::Ref(n) => lookup(n)?,
            Ty::Named(_, t) => t.clone(),
            _ => break,
        };
        d = next;
    }
    Ok(d)
}

fn is_int_f(x: f64) -> bool {
    x.fract() == 0.0
}

fn gen_base(d: &T, g: &mut Rng, h: &Hints, depth: usize) -> R {
    let r = resolve(d)?;
    match &*r {
        Ty::Prim(P::Int) if h.min.is_some() || h.max.is_some() => {
            let lo = match h.min {
                Some(m) => m.ceil() + if h.min_ex && is_int_f(m) { 1.0 } else { 0.0 },
                None => h.max.unwrap() - 1000.0,
            };
            let hi = match h.max {
                Some(m) => m.floor() - if h.max_ex && is_int_f(m) { 1.0 } else { 0.0 },
                None => lo + 1000.0,
            };
            let (lo, hi) = (lo as i64, hi as i64);
            return Ok(V::Int(if g.f() < 0.2 { *g.pick(&[lo, hi]) } else { g.int(lo, hi) }));
        }
        Ty::Prim(P::Money) if h.min.is_some() || h.max.is_some() => {
            let lo = h.min.unwrap_or_else(|| h.max.unwrap() - 1000.0);
            let hi = h.max.unwrap_or(lo + 1000.0);
            let cents = g.int((lo * 100.0).ceil() as i64, (hi * 100.0).floor() as i64);
            return div(V::Int(cents), dec_lit("100"));
        }
        Ty::Prim(P::String) => {
            if let Some(re) = h.re {
                return Ok(s(&gen_re(re, g)));
            }
            if h.digits {
                let n = match h.len {
                    Some(n) => n,
                    None => g.int(1, 12),
                };
                let t: String = (0..n).map(|_| char::from(b'0' + g.int(0, 9) as u8)).collect();
                return Ok(s(&t));
            }
            if h.email {
                let mut local: String = rand_str(g, 1, 6).chars().filter(|c| !is_space(*c) && !"@<>&\"'".contains(*c)).collect();
                if local.is_empty() {
                    local = "a".into();
                }
                return Ok(s(&format!("{}@firma.pl", local)));
            }
            if let Some(p) = h.prefix {
                return Ok(s(&format!("{}{}", p, rand_str(g, 0, 12))));
            }
            if h.len.is_some() || h.minlen.is_some() || h.maxlen.is_some() {
                let lo = h.len.or(h.minlen).unwrap_or(0);
                let hi = h.len.unwrap_or_else(|| h.maxlen.unwrap_or(lo + 12).min(lo + 30));
                return Ok(s(&if g.f() < 0.3 { pool_pick(g) } else { rand_str(g, lo, hi) }));
            }
        }
        Ty::List(e) if h.len.is_some() || h.minlen.is_some() || h.maxlen.is_some() => {
            let lo = h.len.or(h.minlen).unwrap_or(0);
            let hi = h.len.unwrap_or_else(|| h.maxlen.unwrap_or(lo + 3).min(lo + 3));
            let n = g.int(lo, hi);
            let mut out = vec![];
            for _ in 0..n {
                out.push(gen(e, g, depth + 1)?);
            }
            return Ok(list(out));
        }
        _ => {}
    }
    gen(d, g, depth)
}

pub fn gen(d: &T, g: &mut Rng, depth: usize) -> R {
    match &**d {
        Ty::Prim(p) => Ok(match p {
            P::Int => V::Int(if g.f() < 0.3 { *g.pick(&[0, 1, -1, 2, 7, 42, 100, 2026]) } else { g.int(-1000, 10000) }),
            P::Money => {
                if g.f() < 0.3 {
                    dec_lit(g.pick(&["0", "0.01", "1", "33.33", "100", "999999.99"]))
                } else {
                    div(V::Int(g.int(-10000, 1000000)), dec_lit("100"))?
                }
            }
            P::String => s(&if g.f() < 0.5 { pool_pick(g) } else { rand_str(g, 0, 12) }),
            P::Bool => V::Bool(g.f() < 0.5),
            P::Html => {
                let mut o = String::new();
                esc_into(&mut o, &rand_str(g, 0, 12));
                V::Html(Rc::from(o))
            }
            P::Date => {
                let (y, m) = (g.int(2000, 2030), g.int(1, 12));
                V::Date(y, m, g.int(1, 28))
            }
            P::DateTime => {
                let (y, m) = (g.int(2000, 2030), g.int(1, 12));
                let (d, h, mi) = (g.int(1, 28), g.int(0, 23), g.int(0, 59));
                V::DT(y, m, d, h, mi, g.int(0, 59))
            }
        }),
        Ty::List(e) => {
            let n = if depth > 3 { g.int(0, 1) } else { g.int(0, 4) };
            let mut out = vec![];
            for _ in 0..n {
                out.push(gen(e, g, depth + 1)?);
            }
            Ok(list(out))
        }
        Ty::Ref(n) => gen(&lookup(n)?, g, depth),
        Ty::Named(_, t) => gen(t, g, depth),
        Ty::Rec(n, fs) => {
            let mut out = vec![];
            for (f, fd) in fs.iter() {
                out.push((*f, gen(fd, g, depth + 1)?));
            }
            mk(n, out)
        }
        Ty::Var(n) => {
            let Some(fs) = vfields(n) else { return Ok(vsing(n)) };
            let mut out = vec![];
            for (f, fd) in fs.iter() {
                out.push((*f, gen(fd, g, depth + 1)?));
            }
            mkv(n, out)
        }
        Ty::Union(a) => {
            let t = g.pick(a).clone();
            gen(&t, g, depth)
        }
        Ty::Refine(b, c, src, h) => {
            for _ in 0..2000 {
                let x = match gen_base(b, g, h, depth).and_then(|x| conform_s(b, x, "")) {
                    Ok(x) => x,
                    Err(Ctl::Type(_)) => continue,
                    Err(e) => return Err(e),
                };
                match c(x.clone()) {
                    Ok(V::Bool(true)) => return Ok(x),
                    Ok(_) | Err(Ctl::Type(_)) => continue,
                    Err(e) => return Err(e),
                }
            }
            terr(format!("property: nie udało się wylosować wartości typu {}", src))
        }
        d => terr(format!("property: nie da się wylosować wartości typu {}", tname(d))),
    }
}

pub type Res = Vec<(&'static str, V)>;

pub fn res(r: &Res, n: &str) -> V {
    r.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone()).unwrap_or(V::Unit)
}

fn test_res(spec: &[(&'static str, Spec)]) -> Result<Res, Ctl> {
    let mut r = vec![];
    for (n, sp) in spec {
        r.push((*n, mkres(sp)?));
    }
    Ok(r)
}

pub struct Test {
    pub kind: &'static str,
    pub fname: &'static str,
    pub file: &'static str,
    pub line: usize,
    pub src: &'static str,
    pub gens: Option<fn() -> Vec<(&'static str, T)>>,
    pub run: fn(&Res, &[V]) -> R,
}

fn explain(e: &Ctl) -> String {
    match e {
        Ctl::Fail(m) => m.clone(),
        Ctl::Ret(v) => format!("try: wynik to błąd {}", show(v)),
        Ctl::Type(m) => format!("błąd programu: {}", m),
        Ctl::Db(m) => format!("błąd: SQLiteError: {}", m),
    }
}

// `sowa run --rust`: zasoby z [resources] według nazw parametrów main.
pub fn run_main(main: fn(&Res) -> R, spec: Vec<(&'static str, Spec)>) {
    let r = test_res(&spec).and_then(|r| main(&r));
    out_flush();
    if let Err(e) = r {
        let m = explain(&e);
        eprintln!("{}", if m.starts_with("błąd") { m } else { format!("błąd programu: {}", m) });
        std::process::exit(1);
    }
}

pub fn run_tests(tests: Vec<Test>, spec: Vec<(&'static str, Spec)>) {
    let (mut ex, mut pr, mut doc) = (0, 0, 0);
    let mut failed: Vec<String> = vec![];
    for t in &tests {
        let wher = format!("{}:{}", t.file, t.line);
        if t.kind == "property" {
            let mut g = Rng(hash(&wher));
            let gens = t.gens.unwrap();
            let mut ok = true;
            let mut i = 0;
            while i < 100 && ok {
                let mut vals: Vec<(&'static str, V)> = vec![];
                let r = (|| {
                    for (n, d) in gens() {
                        let x = gen(&d, &mut g, 0)?;
                        vals.push((n, x));
                    }
                    let p: Vec<V> = vals.iter().map(|(_, v)| v.clone()).collect();
                    (t.run)(&test_res(&spec)?, &p)
                })();
                if let Err(e) = r {
                    ok = false;
                    let shown: String = vals.iter().map(|(k, x)| format!("\n    {} = {}", k, show(x))).collect();
                    failed.push(format!(
                        "{}: property {}\n    przypadek {} ze 100:{}\n    {}",
                        wher,
                        t.src,
                        i + 1,
                        shown,
                        explain(&e).replace('\n', "\n    ")
                    ));
                }
                i += 1;
            }
            if ok {
                pr += 1;
            }
        } else {
            match test_res(&spec).and_then(|r| (t.run)(&r, &[])) {
                Ok(_) => {
                    if t.kind == "doc" {
                        doc += 1
                    } else {
                        ex += 1
                    }
                }
                Err(e) => failed.push(format!(
                    "{}: {}{}\n    {}",
                    wher,
                    if t.kind == "doc" { "blok sowa" } else { "przykład" },
                    if t.fname.is_empty() { String::new() } else { format!(" ({})", t.fname) },
                    explain(&e)
                )),
            }
        }
    }
    for f in &failed {
        println!("BŁĄD {}\n", f);
    }
    let summary = format!("przykłady: {}, property: {} (po 100 przypadków), bloki sowa w docs/: {}", ex, pr, doc);
    if !failed.is_empty() {
        println!("{} z {} testów nie przechodzi. Przechodzą: {}", failed.len(), tests.len(), summary);
        std::process::exit(1);
    }
    println!("Wszystkie testy przechodzą ({}): {}", tests.len(), summary);
}

// ---------- wartości o typie znanym w kompilacji ----------
// Generator zamienia Int na i64, Bool na bool, String na Rc<str>, List<T> na Rc<Vec<T>>, a rekord
// użytkownika na struct. Val przenosi je do V i z powrotem tam, gdzie typ nie jest znany.

pub trait Val: Clone {
    fn tv(self) -> V;
    fn fv(v: V) -> Result<Self, Ctl>;
    fn eqv(&self, o: &Self) -> bool;
    fn list_tv(xs: Rc<Vec<Self>>) -> V {
        V::List(Rc::new(match Rc::try_unwrap(xs) {
            Ok(v) => v.into_iter().map(Val::tv).collect(),
            Err(rc) => rc.iter().cloned().map(Val::tv).collect(),
        }))
    }
    fn list_fv(xs: Rc<Vec<V>>) -> Result<Rc<Vec<Self>>, Ctl> {
        let out: Result<Vec<Self>, Ctl> = match Rc::try_unwrap(xs) {
            Ok(v) => v.into_iter().map(Self::fv).collect(),
            Err(rc) => rc.iter().cloned().map(Self::fv).collect(),
        };
        Ok(Rc::new(out?))
    }
}

pub fn bad<T>(v: &V) -> Result<T, Ctl> {
    terr(format!("wewnętrzny błąd typu: {}", show(v)))
}
#[inline]
pub fn to_v<T: Val>(x: T) -> V {
    x.tv()
}
#[inline]
pub fn from_v<T: Val>(v: V) -> Result<T, Ctl> {
    T::fv(v)
}
pub fn fget(o: &Obj, k: &str) -> V {
    o.get(k).cloned().unwrap_or(V::Unit)
}

impl Val for V {
    fn tv(self) -> V {
        self
    }
    fn fv(v: V) -> Result<Self, Ctl> {
        Ok(v)
    }
    fn eqv(&self, o: &Self) -> bool {
        eq(self, o)
    }
    fn list_tv(xs: Rc<Vec<V>>) -> V {
        V::List(xs)
    }
    fn list_fv(xs: Rc<Vec<V>>) -> Result<Rc<Vec<V>>, Ctl> {
        Ok(xs)
    }
}
impl Val for i64 {
    fn tv(self) -> V {
        V::Int(self)
    }
    fn fv(v: V) -> Result<Self, Ctl> {
        match v {
            V::Int(n) => Ok(n),
            v => bad(&v),
        }
    }
    fn eqv(&self, o: &Self) -> bool {
        self == o
    }
}
impl Val for bool {
    fn tv(self) -> V {
        V::Bool(self)
    }
    fn fv(v: V) -> Result<Self, Ctl> {
        match v {
            V::Bool(b) => Ok(b),
            v => bad(&v),
        }
    }
    fn eqv(&self, o: &Self) -> bool {
        self == o
    }
}
impl Val for Rc<str> {
    fn tv(self) -> V {
        V::Str(self)
    }
    fn fv(v: V) -> Result<Self, Ctl> {
        match v {
            V::Str(s) => Ok(s),
            v => bad(&v),
        }
    }
    fn eqv(&self, o: &Self) -> bool {
        self == o
    }
}
impl<T: Val> Val for Rc<Vec<T>> {
    fn tv(self) -> V {
        T::list_tv(self)
    }
    fn fv(v: V) -> Result<Self, Ctl> {
        match v {
            V::List(xs) => T::list_fv(xs),
            v => bad(&v),
        }
    }
    fn eqv(&self, o: &Self) -> bool {
        Rc::ptr_eq(self, o) || self.len() == o.len() && self.iter().zip(o.iter()).all(|(a, b)| a.eqv(b))
    }
}

pub fn lconv<A: Val, B: Val>(xs: Rc<Vec<A>>) -> Result<Rc<Vec<B>>, Ctl> {
    match A::list_tv(xs) {
        V::List(l) => B::list_fv(l),
        v => bad(&v),
    }
}

// Błąd z tym samym opisem, jaki dałby conform w runtime.
pub fn fail_conform(v: V, d: &Ty, wh: &Wh) -> Ctl {
    match conform(d, v, wh) {
        Err(e) => e,
        Ok(_) => Ctl::Type(format!("{}: warunek typu", wh)),
    }
}

#[inline]
fn int_ok(n: Option<i64>) -> Result<i64, Ctl> {
    match n {
        Some(n) if (-MAX_SAFE..=MAX_SAFE).contains(&n) => Ok(n),
        Some(n) => terr(format!("liczba poza zakresem Int: {}", n)),
        None => terr("liczba poza zakresem Int"),
    }
}
#[inline]
pub fn iadd(a: i64, b: i64) -> Result<i64, Ctl> {
    int_ok(a.checked_add(b))
}
#[inline]
pub fn isub(a: i64, b: i64) -> Result<i64, Ctl> {
    int_ok(a.checked_sub(b))
}
#[inline]
pub fn imul(a: i64, b: i64) -> Result<i64, Ctl> {
    int_ok(a.checked_mul(b))
}
#[inline]
pub fn idiv(a: i64, b: i64) -> Result<i64, Ctl> {
    if b == 0 {
        return terr("dzielenie przez zero");
    }
    Ok(a / b)
}
#[inline]
pub fn irem(a: i64, b: i64) -> Result<i64, Ctl> {
    if b == 0 {
        return terr(format!("reszta z dzielenia tylko dla Int: {} % {}", a, b));
    }
    Ok(a % b)
}

pub fn scat(parts: &[&str]) -> Rc<str> {
    let mut o = String::with_capacity(parts.iter().map(|p| p.len()).sum());
    for p in parts {
        o.push_str(p);
    }
    Rc::from(o)
}
// Bufor do składania tekstu: po użyciu wraca do wątku, więc Rc<str> to jedyna alokacja.
thread_local! {
    static SCRATCH: RefCell<String> = RefCell::new(String::new());
}
pub fn sbuf() -> String {
    let mut b = SCRATCH.with(|s| std::mem::take(&mut *s.borrow_mut()));
    b.reserve(64);
    b
}
pub fn sdone(mut b: String) -> Rc<str> {
    let r = Rc::from(b.as_str());
    b.clear();
    SCRATCH.with(|s| *s.borrow_mut() = b);
    r
}
#[inline]
pub fn set_rc<T: ?Sized>(d: &mut Rc<T>, s: &Rc<T>) {
    if !Rc::ptr_eq(d, s) {
        *d = s.clone();
    }
}
pub fn lext<T: Clone>(mut a: Rc<Vec<T>>, b: Vec<T>) -> Rc<Vec<T>> {
    Rc::make_mut(&mut a).extend(b);
    a
}
pub fn push_int(b: &mut String, x: i64) {
    let mut d = [0u8; 20];
    let mut i = d.len();
    let mut u = x.unsigned_abs();
    loop {
        i -= 1;
        d[i] = b'0' + (u % 10) as u8;
        u /= 10;
        if u == 0 {
            break;
        }
    }
    if x < 0 {
        b.push('-');
    }
    // SAFETY: same cyfry ASCII.
    b.push_str(unsafe { std::str::from_utf8_unchecked(&d[i..]) });
}
pub fn push_char(b: &mut String, n: i64) -> Result<(), Ctl> {
    match u32::try_from(n).ok().and_then(char::from_u32) {
        Some(c) => Ok(b.push(c)),
        None => terr(format!("char: {} nie jest kodem znaku", n)),
    }
}
pub fn lcat<T: Clone>(mut a: Rc<Vec<T>>, b: Rc<Vec<T>>) -> Rc<Vec<T>> {
    if a.is_empty() {
        return b;
    }
    match Rc::try_unwrap(b) {
        Ok(v) => Rc::make_mut(&mut a).extend(v),
        Err(rc) => Rc::make_mut(&mut a).extend(rc.iter().cloned()),
    }
    a
}
#[inline]
pub fn slen(t: &str) -> i64 {
    t.chars().count() as i64
}
#[inline]
// Błąd poza gorącą ścieżką, żeby at_ref i at_ dało się wkleić w pętlę.
#[cold]
#[inline(never)]
fn at_err(i: i64, n: usize) -> Ctl {
    Ctl::Type(format!("at: indeks {} poza listą o długości {}", i, n))
}
pub fn at_ref<T>(xs: &Rc<Vec<T>>, i: i64) -> Result<&T, Ctl> {
    match xs.get(i as usize) {
        Some(x) if i >= 0 => Ok(x),
        _ => Err(at_err(i, xs.len())),
    }
}
pub fn at_<T: Clone>(xs: &Rc<Vec<T>>, i: i64) -> Result<T, Ctl> {
    match xs.get(i as usize) {
        Some(x) if i >= 0 => Ok(x.clone()),
        _ => Err(at_err(i, xs.len())),
    }
}
pub fn join_(xs: &Rc<Vec<Rc<str>>>, sep: &str) -> Rc<str> {
    let mut out = String::with_capacity(xs.iter().map(|x| x.len() + sep.len()).sum());
    for (k, x) in xs.iter().enumerate() {
        if k > 0 {
            out.push_str(sep);
        }
        out.push_str(x);
    }
    Rc::from(out)
}
pub fn char_(n: i64) -> Result<Rc<str>, Ctl> {
    match u32::try_from(n).ok().and_then(char::from_u32) {
        Some(c) => Ok(Rc::from(c.encode_utf8(&mut [0; 4]) as &str)),
        None => terr(format!("char: {} nie jest kodem znaku", n)),
    }
}
pub fn trim_(t: &str) -> Rc<str> {
    Rc::from(t.trim_matches(is_space))
}
pub fn str_contains(t: &str, part: &str) -> bool {
    t.contains(part)
}
pub fn split_(t: &str, p: &str) -> Rc<Vec<Rc<str>>> {
    if p.is_empty() {
        return Rc::new(t.encode_utf16().map(|u| Rc::from(String::from_utf16_lossy(&[u]))).collect());
    }
    Rc::new(t.split(p).map(Rc::from).collect())
}
pub fn chars_(t: &str) -> Rc<Vec<Rc<str>>> {
    Rc::new(t.chars().map(|c| Rc::from(c.encode_utf8(&mut [0; 4]) as &str)).collect())
}

// map, filter i reverse na liście bez innych referencji pracują w tej samej tablicy.
// map z tym samym typem elementu zmienia tablicę w miejscu. Lista z innymi referencjami jest najpierw kopiowana.
pub fn lmap_same<A: Clone, F: FnMut(A) -> Result<Option<A>, Ctl>>(mut xs: Rc<Vec<A>>, mut f: F) -> Result<Rc<Vec<A>>, Ctl> {
    // Zmiana w miejscu: element wyjmowany, przekazywany f i wkładany z powrotem, a przy None zostaje.
    // Na czas pętli długość 0, więc panika w f tylko gubi elementy, a nie zwalnia ich dwa razy.
    let v = Rc::make_mut(&mut xs);
    let n = v.len();
    let p = v.as_mut_ptr();
    unsafe {
        v.set_len(0);
        for i in 0..n {
            let x = p.add(i);
            match f(std::ptr::read(x)) {
                Ok(None) => {}
                Ok(Some(y)) => std::ptr::write(x, y),
                Err(e) => {
                    std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(p.add(i + 1), n - i - 1));
                    v.set_len(i);
                    return Err(e);
                }
            }
        }
        v.set_len(n);
    }
    Ok(xs)
}
pub fn lmap<A: Clone, B, F: FnMut(A) -> Result<B, Ctl>>(xs: Rc<Vec<A>>, mut f: F) -> Result<Rc<Vec<B>>, Ctl> {
    let same = std::mem::size_of::<A>() == std::mem::size_of::<B>() && std::mem::align_of::<A>() == std::mem::align_of::<B>();
    match Rc::try_unwrap(xs) {
        Ok(v) if same => Ok(Rc::new(v.into_iter().map(f).collect::<Result<Vec<B>, Ctl>>()?)),
        Ok(v) => {
            let mut out = Vec::with_capacity(v.len());
            for x in v {
                out.push(f(x)?);
            }
            Ok(Rc::new(out))
        }
        Err(rc) => {
            let mut out = Vec::with_capacity(rc.len());
            for x in rc.iter() {
                out.push(f(x.clone())?);
            }
            Ok(Rc::new(out))
        }
    }
}
pub fn lfilter<A: Clone, F: FnMut(&A) -> Result<bool, Ctl>>(xs: Rc<Vec<A>>, mut f: F) -> Result<Rc<Vec<A>>, Ctl> {
    let mut out = Vec::with_capacity(xs.len());
    for x in xs.iter() {
        if f(x)? {
            out.push(x.clone());
        }
    }
    Ok(Rc::new(out))
}
pub fn lrev<A: Clone>(mut xs: Rc<Vec<A>>) -> Rc<Vec<A>> {
    Rc::make_mut(&mut xs).reverse();
    xs
}

pub fn rnd_int(c: &V, lo: i64, hi: i64, line: usize) -> Result<i64, Ctl> {
    if let V::Cap(cap) = c {
        if let Cap::Random(g) = &**cap {
            if lo > hi {
                return terr(format!("random.int: pusty przedział {}..{}", lo, hi));
            }
            return Ok(lo + g.borrow_mut().below((hi - lo + 1) as u64) as i64);
        }
    }
    from_v(call(c.clone(), "int", vec![V::Int(lo), V::Int(hi)], vec![], vec![], line)?)
}
pub fn rnd_choice<T: Val>(c: &V, xs: &Rc<Vec<T>>, line: usize) -> Result<T, Ctl> {
    if let V::Cap(cap) = c {
        if let Cap::Random(g) = &**cap {
            if xs.is_empty() {
                return terr("random.choice: pusta lista");
            }
            return Ok(xs[g.borrow_mut().below(xs.len() as u64) as usize].clone());
        }
    }
    from_v(call(c.clone(), "choice", vec![xs.clone().tv()], vec![], vec![], line)?)
}
// Dopisanie krótkiego tekstu bez wołania memcpy: dwa zachodzące na siebie kawałki stałej długości.
#[inline(always)]
pub fn push_s(b: &mut String, s: &str) {
    let n = s.len();
    if n > 32 {
        b.push_str(s);
        return;
    }
    // SAFETY: zapis mieści się w zarezerwowanym miejscu, a odczyt w s; dopisane bajty to całe s.
    unsafe {
        let v = b.as_mut_vec();
        v.reserve(32);
        let d = v.as_mut_ptr().add(v.len());
        let p = s.as_ptr();
        use std::ptr::copy_nonoverlapping as cp;
        if n >= 16 {
            cp(p, d, 16);
            cp(p.add(n - 16), d.add(n - 16), 16);
        } else if n >= 8 {
            cp(p, d, 8);
            cp(p.add(n - 8), d.add(n - 8), 8);
        } else if n >= 4 {
            cp(p, d, 4);
            cp(p.add(n - 4), d.add(n - 4), 4);
        } else if n > 0 {
            *d = *p;
            *d.add(n / 2) = *p.add(n / 2);
            *d.add(n - 1) = *p.add(n - 1);
        }
        v.set_len(v.len() + n);
    }
}

// Bufor stdout do pisania wprost, bez tekstu pośredniego. Wyrażenie pisane do bufora nie widzi
// żadnego uprawnienia, więc nic innego nie pisze w tym czasie na stdout.
pub fn out_take() -> String {
    OUT.with(|o| std::mem::take(&mut *o.borrow_mut()))
}
pub fn term_put(c: &V, mut b: String, st: usize, ok: Result<(), Ctl>, line: usize) -> R {
    let real = matches!(c, V::Cap(cap) if matches!(**cap, Cap::Terminal));
    let t = if real && ok.is_ok() {
        None
    } else {
        let t = b[st..].to_string();
        b.truncate(st);
        Some(t)
    };
    let full = b.len() > 1 << 16;
    OUT.with(|o| *o.borrow_mut() = b);
    if full {
        out_flush();
    }
    ok?;
    match t {
        None => Ok(V::Unit),
        Some(t) => call(c.clone(), "write", vec![V::Str(Rc::from(t))], vec![], vec![], line),
    }
}
pub fn term_write(c: &V, t: &str, line: usize) -> R {
    if let V::Cap(cap) = c {
        if let Cap::Terminal = &**cap {
            out_write(t);
            return Ok(V::Unit);
        }
    }
    call(c.clone(), "write", vec![V::Str(Rc::from(t))], vec![], vec![], line)
}
