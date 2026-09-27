// Drzewo składni Sowy. Parser buduje je z tokenów, a check i codegen tylko czytają.

use std::fmt;

#[derive(Debug, Clone)]
pub struct Diag {
    pub file: String,
    pub line: usize,
    pub msg: String,
}

impl Diag {
    pub fn new(file: &str, line: usize, msg: impl Into<String>) -> Diag {
        Diag {
            file: file.to_string(),
            line,
            msg: msg.into(),
        }
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.line > 0 {
            write!(f, "{}:{}: {}", self.file, self.line, self.msg)
        } else {
            write!(f, "{}: {}", self.file, self.msg)
        }
    }
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    // Name<args>(cond) albo Name(pole: Typ, ...) w definicji wariantu.
    Name {
        name: String,
        args: Vec<TypeExpr>,
        cond: Option<Box<Cond>>,
        fields: Option<Vec<Field>>,
        line: usize,
    },
    Union(Vec<TypeExpr>),
}

impl TypeExpr {
    pub fn alts(&self) -> Vec<&TypeExpr> {
        match self {
            TypeExpr::Union(v) => v.iter().collect(),
            t => vec![t],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cond {
    pub param: String,
    pub named: bool,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: TypeExpr,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Directive {
    pub kind: String,
    pub text: String,
    pub line: usize,
    pub lines: usize,
}

#[derive(Debug, Clone)]
pub enum TypeBody {
    Rhs(Vec<TypeExpr>),
    Record(Vec<Field>),
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub line: usize,
    pub body: TypeBody,
    pub directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Example {
    pub line: usize,
    pub stmts: Vec<Stmt>,
    pub multiline: bool,
    pub nlines: usize,
}

#[derive(Debug, Clone)]
pub struct Property {
    pub line: usize,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub line: usize,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub directives: Vec<Directive>,
    pub examples: Vec<Example>,
    pub properties: Vec<Property>,
    pub body: Option<Vec<Stmt>>,
    // Kolejność linii pod sygnaturą (desc, doc, why, example, property) do sprawdzenia.
    pub order: Vec<(String, usize)>,
}

impl FnDecl {
    pub fn signature(&self) -> String {
        let params: Vec<String> = self.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
        match &self.ret {
            Some(r) => format!("fn {}({}) -> {}", self.name, params.join(", "), r),
            None => format!("fn {}({})", self.name, params.join(", ")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Item {
    Type(TypeDecl),
    Fn(FnDecl),
}

#[derive(Debug, Clone)]
pub struct SourceFile {
    pub path: String,
    pub module: String,
    pub is_impl: bool,
    pub header: Vec<Directive>,
    pub items: Vec<Item>,
    pub stray: Vec<Directive>,
}

// Linie zostają w drzewie dla przyszłych komunikatów, nawet jeśli dziś nikt ich nie czyta.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Stmt {
    Set {
        name: String,
        is_var: bool,
        expr: Expr,
        line: usize,
    },
    Return {
        expr: Option<Expr>,
        line: usize,
    },
    Expr {
        expr: Expr,
        line: usize,
    },
    If {
        cond: Expr,
        then: Vec<Stmt>,
        els: Option<Vec<Stmt>>,
        line: usize,
    },
    For {
        var: String,
        iter: Expr,
        body: Vec<Stmt>,
        line: usize,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
        line: usize,
    },
    Match {
        subjects: Vec<Expr>,
        arms: Vec<Arm>,
        line: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Arm {
    pub pats: Vec<Pat>,
    pub body: Vec<Stmt>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub enum Pat {
    Wild,
    Str(String),
    List(Vec<Pat>),
    Bind(String),
    Name { name: String, bind: Option<String> },
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct Arg {
    pub name: Option<String>,
    pub value: Expr,
}

#[derive(Debug, Clone)]
pub enum HtmlSeg {
    Lit(String),
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub enum Alt {
    Return(Option<Expr>),
    Value(Expr),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Int(i64),
    Dec(String),
    Str(String),
    Bool(bool),
    Html(Vec<HtmlSeg>),
    Ident(String),
    List(Vec<Expr>),
    Call {
        name: String,
        args: Vec<Arg>,
    },
    Method {
        obj: Box<Expr>,
        name: String,
        targs: Vec<TypeExpr>,
        args: Vec<Arg>,
    },
    Field {
        obj: Box<Expr>,
        name: String,
    },
    Bin {
        op: &'static str,
        l: Box<Expr>,
        r: Box<Expr>,
    },
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Is {
        e: Box<Expr>,
        ty: TypeExpr,
        neg: bool,
    },
    As {
        e: Box<Expr>,
        ty: TypeExpr,
        alt: Option<Box<Alt>>,
    },
    With {
        e: Box<Expr>,
        fields: Vec<(String, Expr)>,
    },
    Lambda {
        param: String,
        body: LambdaBody,
    },
    Try(Box<Expr>),
}

// Wypisywanie: sygnatury do porównania src z impl, warunki w komunikatach i testach.

impl fmt::Display for TypeExpr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TypeExpr::Union(v) => {
                let parts: Vec<String> = v.iter().map(|t| t.to_string()).collect();
                write!(f, "{}", parts.join(" | "))
            }
            TypeExpr::Name { name, args, cond, fields, .. } => {
                write!(f, "{}", name)?;
                if !args.is_empty() {
                    let a: Vec<String> = args.iter().map(|t| t.to_string()).collect();
                    write!(f, "<{}>", a.join(", "))?;
                }
                if let Some(fs) = fields {
                    let a: Vec<String> = fs.iter().map(|x| format!("{}: {}", x.name, x.ty)).collect();
                    write!(f, "({})", a.join(", "))?;
                }
                if let Some(c) = cond {
                    if c.named {
                        write!(f, "({} => {})", c.param, c.expr)?;
                    } else {
                        write!(f, "({})", c.expr)?;
                    }
                }
                Ok(())
            }
        }
    }
}

fn prec(op: &str) -> u8 {
    match op {
        "||" => 1,
        "&&" => 2,
        "==" | "!=" | "<" | "<=" | ">" | ">=" => 4,
        "+" | "-" => 6,
        "*" | "/" | "%" => 7,
        _ => 9,
    }
}

fn eprec(e: &Expr) -> u8 {
    match &e.kind {
        ExprKind::Bin { op, .. } => prec(op),
        ExprKind::Not(_) => 3,
        ExprKind::Is { .. } => 4,
        ExprKind::As { .. } | ExprKind::With { .. } => 5,
        ExprKind::Lambda { .. } => 0,
        _ => 10,
    }
}

fn args_str(args: &[Arg]) -> String {
    let a: Vec<String> = args
        .iter()
        .map(|a| match &a.name {
            Some(n) => format!("{}: {}", n, a.value),
            None => a.value.to_string(),
        })
        .collect();
    a.join(", ")
}

pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// Warunek z parametrem `from` jako warunek z α.
pub fn rename(e: &Expr, from: &str) -> Expr {
    if from == "α" {
        return e.clone();
    }
    subst(e, &|n| {
        (n == from).then(|| Expr {
            kind: ExprKind::Ident("α".into()),
            line: e.line,
        })
    })
}

// Wyrażenie z identyfikatorami zamienionymi przez f. Lambda zasłania swój parametr, a lambdy
// z blokiem zostają bez zmian.
pub fn subst(e: &Expr, f: &dyn Fn(&str) -> Option<Expr>) -> Expr {
    let r = |x: &Expr| Box::new(subst(x, f));
    let args = |a: &[Arg]| -> Vec<Arg> {
        a.iter()
            .map(|a| Arg {
                name: a.name.clone(),
                value: subst(&a.value, f),
            })
            .collect()
    };
    let kind = match &e.kind {
        ExprKind::Ident(n) => match f(n) {
            Some(x) => return x,
            None => ExprKind::Ident(n.clone()),
        },
        ExprKind::Field { obj, name } => ExprKind::Field {
            obj: r(obj),
            name: name.clone(),
        },
        ExprKind::Call { name, args: a } => ExprKind::Call {
            name: name.clone(),
            args: args(a),
        },
        ExprKind::Method { obj, name, targs, args: a } => ExprKind::Method {
            obj: r(obj),
            name: name.clone(),
            targs: targs.clone(),
            args: args(a),
        },
        ExprKind::Bin { op, l, r: rr } => ExprKind::Bin { op, l: r(l), r: r(rr) },
        ExprKind::Not(x) => ExprKind::Not(r(x)),
        ExprKind::Neg(x) => ExprKind::Neg(r(x)),
        ExprKind::Try(x) => ExprKind::Try(r(x)),
        ExprKind::List(v) => ExprKind::List(v.iter().map(|x| subst(x, f)).collect()),
        ExprKind::Is { e: x, ty, neg } => ExprKind::Is {
            e: r(x),
            ty: ty.clone(),
            neg: *neg,
        },
        ExprKind::With { e: x, fields } => ExprKind::With {
            e: r(x),
            fields: fields.iter().map(|(n, v)| (n.clone(), subst(v, f))).collect(),
        },
        ExprKind::Lambda {
            param,
            body: LambdaBody::Expr(b),
        } => {
            let g = |n: &str| if n == param { None } else { f(n) };
            ExprKind::Lambda {
                param: param.clone(),
                body: LambdaBody::Expr(Box::new(subst(b, &g))),
            }
        }
        k => k.clone(),
    };
    Expr { kind, line: e.line }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ExprKind::Int(n) => write!(f, "{}", n),
            ExprKind::Dec(s) => write!(f, "{}", s),
            ExprKind::Str(s) => write!(f, "{}", quote(s)),
            ExprKind::Bool(b) => write!(f, "{}", b),
            ExprKind::Html(segs) => {
                write!(f, "html\"")?;
                for s in segs {
                    match s {
                        HtmlSeg::Lit(t) => write!(f, "{}", t)?,
                        HtmlSeg::Expr(e) => write!(f, "{{{}}}", e)?,
                    }
                }
                write!(f, "\"")
            }
            ExprKind::Ident(n) => write!(f, "{}", n),
            ExprKind::List(v) => {
                let a: Vec<String> = v.iter().map(|e| e.to_string()).collect();
                write!(f, "[{}]", a.join(", "))
            }
            ExprKind::Call { name, args } => write!(f, "{}({})", name, args_str(args)),
            ExprKind::Method { obj, name, targs, args } => {
                write!(f, "{}.{}", obj, name)?;
                if !targs.is_empty() {
                    let a: Vec<String> = targs.iter().map(|t| t.to_string()).collect();
                    write!(f, "<{}>", a.join(", "))?;
                }
                write!(f, "({})", args_str(args))
            }
            ExprKind::Field { obj, name } => write!(f, "{}.{}", obj, name),
            ExprKind::Bin { op, l, r } => {
                let p = prec(op);
                if eprec(l) < p {
                    write!(f, "({})", l)?;
                } else {
                    write!(f, "{}", l)?;
                }
                write!(f, " {} ", op)?;
                if eprec(r) <= p { write!(f, "({})", r) } else { write!(f, "{}", r) }
            }
            ExprKind::Not(e) => write!(f, "not {}", e),
            ExprKind::Neg(e) => write!(f, "-{}", e),
            ExprKind::Is { e, ty, neg } => write!(f, "{} is {}{}", e, if *neg { "not " } else { "" }, ty),
            ExprKind::As { e, ty, alt } => {
                write!(f, "{} as {}", e, ty)?;
                match alt.as_deref() {
                    None => Ok(()),
                    Some(Alt::Return(Some(x))) => write!(f, " or return {}", x),
                    Some(Alt::Return(None)) => write!(f, " or return"),
                    Some(Alt::Value(x)) => write!(f, " or {}", x),
                    Some(Alt::Block(_)) => write!(f, " or ..."),
                }
            }
            ExprKind::With { e, fields } => {
                let a: Vec<String> = fields.iter().map(|(n, v)| format!("{}: {}", n, v)).collect();
                write!(f, "{} with {}", e, a.join(", "))
            }
            ExprKind::Lambda { param, body } => match body {
                LambdaBody::Expr(e) => write!(f, "{} => {}", param, e),
                LambdaBody::Block(_) => write!(f, "{} => ...", param),
            },
            ExprKind::Try(e) => write!(f, "try {}", e),
        }
    }
}
