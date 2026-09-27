// Tablica nazw całego programu: typy, warianty i funkcje (src i impl razem).

use crate::ast::*;
use std::collections::BTreeMap;

pub const PRIMS: [&str; 7] = ["Int", "Money", "String", "Bool", "Html", "Date", "DateTime"];
pub const CAPS: [&str; 7] = ["Db", "DbRead", "Clock", "Http", "Server", "Random", "Terminal"];

// Wbudowane rekordy: nazwa → pola (nazwa, typ).
pub const BUILTIN_RECORDS: [(&str, &[(&str, &str)]); 2] = [
    ("HttpRequest", &[("method", "Method"), ("path", "String"), ("body", "String")]),
    ("HttpResponse", &[("status", "Int"), ("body", "String")]),
];

// Wbudowane unie wariantów bez danych: nazwa typu → warianty.
pub const BUILTIN_UNIONS: [(&str, &[&str]); 1] = [("Method", &["Get", "Post", "Put", "Patch", "Delete"])];

// Wbudowane warianty bez typu-unii (zwracają je operacje wbudowane).
pub const BUILTIN_VARIANTS: [&str; 4] = ["HttpError", "DbError", "NoRow", "NotANumber"];

// Wbudowane funkcje: nazwa, liczba argumentów, czy asynchroniczna (bierze lambdę).
pub const BUILTIN_FNS: [(&str, usize, bool); 27] = [
    ("segments", 1, false),
    ("parse_int", 1, false),
    ("parse_money", 1, false),
    ("to_string", 1, false),
    ("to_json", 1, false),
    ("round", 2, false),
    ("sum", 1, false),
    ("distinct", 1, false),
    ("sort_by", 2, true),
    ("len", 1, false),
    ("trim", 1, false),
    ("remove", 2, false),
    ("drop_prefix", 2, false),
    ("pad_left", 3, false),
    ("starts_with", 2, false),
    ("contains", 2, false),
    ("matches", 2, false),
    ("only_digits", 1, false),
    ("nip_checksum_ok", 1, false),
    ("valid_email", 1, false),
    ("lower", 1, false),
    ("upper", 1, false),
    ("at", 2, false),
    ("join", 2, false),
    ("chars", 1, false),
    ("char", 1, false),
    ("split", 2, false),
];

// Metody: typ odbiorcy → dozwolone metody.
pub const CAP_METHODS: [(&str, &[&str]); 7] = [
    ("Db", &["get", "all", "save", "transaction"]),
    ("DbRead", &["get", "all"]),
    ("Clock", &["now", "today"]),
    ("Http", &["post", "get"]),
    ("Server", &["serve"]),
    ("Random", &["int", "choice"]),
    ("Terminal", &["read", "write", "exit"]),
];
pub const LIST_METHODS: [&str; 3] = ["map", "filter", "reverse"];

pub fn builtin_fn(name: &str) -> Option<(usize, bool)> {
    BUILTIN_FNS.iter().find(|(n, _, _)| *n == name).map(|(_, a, s)| (*a, *s))
}

pub struct VariantInfo<'a> {
    pub fields: Option<&'a Vec<Field>>,
    pub file: String,
    pub line: usize,
}

pub struct FnInfo<'a> {
    pub spec: Option<(&'a FnDecl, &'a SourceFile)>,
    pub imp: Option<(&'a FnDecl, &'a SourceFile)>,
}

impl<'a> FnInfo<'a> {
    pub fn decl(&self) -> &'a FnDecl {
        self.spec.or(self.imp).unwrap().0
    }
    pub fn module(&self) -> &'a str {
        &self.spec.or(self.imp).unwrap().1.module
    }
    pub fn body(&self) -> Option<&'a Vec<Stmt>> {
        self.imp.and_then(|(d, _)| d.body.as_ref())
    }
    pub fn is_private(&self) -> bool {
        self.spec.is_none()
    }
}

pub struct Env<'a> {
    pub types: BTreeMap<String, (&'a TypeDecl, &'a SourceFile)>,
    pub variants: BTreeMap<String, VariantInfo<'a>>,
    pub fns: BTreeMap<String, FnInfo<'a>>,
    pub diags: Vec<Diag>,
}

impl<'a> Env<'a> {
    pub fn build(files: &'a [SourceFile]) -> Env<'a> {
        let mut env = Env {
            types: BTreeMap::new(),
            variants: BTreeMap::new(),
            fns: BTreeMap::new(),
            diags: vec![],
        };
        for (_, vs) in BUILTIN_UNIONS {
            for v in vs.iter() {
                env.variants.insert(
                    v.to_string(),
                    VariantInfo {
                        fields: None,
                        file: "(wbudowane)".into(),
                        line: 0,
                    },
                );
            }
        }
        for v in BUILTIN_VARIANTS {
            env.variants.insert(
                v.to_string(),
                VariantInfo {
                    fields: None,
                    file: "(wbudowane)".into(),
                    line: 0,
                },
            );
        }
        // Najpierw nazwy typów, bo od nich zależy, czy człon unii to nowy wariant.
        for f in files {
            for it in &f.items {
                if let Item::Type(t) = it {
                    if env.is_builtin_type(&t.name) {
                        env.diags.push(Diag::new(&f.path, t.line, format!("typ {} jest wbudowany", t.name)));
                    } else if let Some((_, pf)) = env.types.get(&t.name) {
                        env.diags
                            .push(Diag::new(&f.path, t.line, format!("typ {} jest już zdefiniowany w {}", t.name, pf.path)));
                    } else {
                        env.types.insert(t.name.clone(), (t, f));
                    }
                }
            }
        }
        for f in files {
            for it in &f.items {
                match it {
                    Item::Type(t) => {
                        if let TypeBody::Rhs(terms) = &t.body {
                            for term in terms {
                                if let TypeExpr::Name {
                                    name,
                                    fields,
                                    line,
                                    args,
                                    cond,
                                } = term
                                {
                                    if env.is_type(name) {
                                        continue;
                                    }
                                    if !args.is_empty() || cond.is_some() {
                                        env.diags.push(Diag::new(&f.path, *line, format!("nieznany typ {}", name)));
                                        continue;
                                    }
                                    if let Some(prev) = env.variants.get(name) {
                                        env.diags.push(Diag::new(
                                            &f.path,
                                            *line,
                                            format!(
                                                "wariant {} jest już zdefiniowany ({}:{}); nazwy wariantów są unikalne w projekcie",
                                                name, prev.file, prev.line
                                            ),
                                        ));
                                        continue;
                                    }
                                    env.variants.insert(
                                        name.clone(),
                                        VariantInfo {
                                            fields: fields.as_ref(),
                                            file: f.path.clone(),
                                            line: *line,
                                        },
                                    );
                                }
                            }
                        }
                    }
                    Item::Fn(d) => {
                        let e = env.fns.entry(d.name.clone()).or_insert(FnInfo { spec: None, imp: None });
                        let slot = if f.is_impl { &mut e.imp } else { &mut e.spec };
                        if let Some((_, pf)) = slot {
                            let msg = format!("funkcja {} jest już zdefiniowana w {}; nazwy funkcji są unikalne w projekcie", d.name, pf.path);
                            env.diags.push(Diag::new(&f.path, d.line, msg));
                        } else {
                            *slot = Some((d, f));
                        }
                    }
                }
            }
        }
        env
    }

    pub fn is_builtin_type(&self, n: &str) -> bool {
        PRIMS.contains(&n) || CAPS.contains(&n) || n == "List" || BUILTIN_RECORDS.iter().any(|(r, _)| *r == n) || BUILTIN_UNIONS.iter().any(|(r, _)| *r == n)
    }

    pub fn is_type(&self, n: &str) -> bool {
        self.is_builtin_type(n) || self.types.contains_key(n)
    }

    // Pola rekordu (użytkownika albo wbudowanego) jako (nazwa, tekst typu) do komunikatów i sprawdzeń.
    pub fn record_fields(&self, n: &str) -> Option<Vec<String>> {
        if let Some((_, fs)) = BUILTIN_RECORDS.iter().find(|(r, _)| *r == n) {
            return Some(fs.iter().map(|(f, _)| f.to_string()).collect());
        }
        match self.types.get(n) {
            Some((
                TypeDecl {
                    body: TypeBody::Record(fs), ..
                },
                _,
            )) => Some(fs.iter().map(|f| f.name.clone()).collect()),
            _ => None,
        }
    }

    pub fn variant_fields(&self, n: &str) -> Option<Vec<String>> {
        self.variants
            .get(n)
            .and_then(|v| v.fields.map(|fs| fs.iter().map(|f| f.name.clone()).collect()))
    }

    // Liście typu: nazwy wariantów i typów, po rozwinięciu unii nazwanych.
    pub fn leaves(&self, t: &TypeExpr) -> Vec<String> {
        let mut out = vec![];
        self.leaves_into(t, &mut out, 0);
        out
    }

    fn leaves_into(&self, t: &TypeExpr, out: &mut Vec<String>, depth: usize) {
        if depth > 20 {
            return;
        }
        for a in t.alts() {
            if let TypeExpr::Name { name, .. } = a {
                if let Some((_, vs)) = BUILTIN_UNIONS.iter().find(|(u, _)| *u == name) {
                    out.extend(vs.iter().map(|v| v.to_string()));
                    continue;
                }
                match self.types.get(name) {
                    Some((
                        TypeDecl {
                            body: TypeBody::Rhs(terms), ..
                        },
                        _,
                    )) if terms.len() > 1 || !self.is_single_refine(terms) => {
                        for term in terms {
                            self.leaves_into(term, out, depth + 1);
                        }
                    }
                    _ => out.push(name.clone()),
                }
            }
        }
    }

    // `type Quantity = Int(α > 0)` to zawężenie, a nie unia: jego liść to sama nazwa.
    fn is_single_refine(&self, terms: &[TypeExpr]) -> bool {
        terms.len() == 1 && matches!(&terms[0], TypeExpr::Name { name, .. } if self.is_type(name))
    }
}
