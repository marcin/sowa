// Parser: tokeny → drzewo z ast.rs.

use crate::ast::*;
use crate::lexer::{HtmlPart, Tok, Token, lex};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
    file: String,
}

type R<T> = Result<T, Diag>;

const KEYWORDS: [&str; 18] = [
    "fn", "type", "return", "match", "if", "else", "for", "while", "in", "var", "try", "as", "or", "is", "not", "with", "true", "false",
];

pub fn parse_file(src: &str, path: &str, module: &str, is_impl: bool) -> R<SourceFile> {
    let toks = lex(src, path, 1)?;
    let mut p = Parser {
        toks,
        pos: 0,
        file: path.to_string(),
    };
    p.file_items(path, module, is_impl)
}

// Blok kodu (przykład w .md): ciąg instrukcji na poziomie 0.
pub fn parse_block_src(src: &str, path: &str, first_line: usize) -> R<Vec<Stmt>> {
    let toks = lex(src, path, first_line)?;
    let mut p = Parser {
        toks,
        pos: 0,
        file: path.to_string(),
    };
    let mut out = vec![];
    p.skip_nl();
    while !p.at_eof() {
        out.push(p.stmt()?);
        p.skip_nl();
    }
    Ok(out)
}

pub fn parse_expr_src(src: &str, path: &str, line: usize) -> R<Expr> {
    let toks = lex(src, path, line)?;
    let mut p = Parser {
        toks,
        pos: 0,
        file: path.to_string(),
    };
    let e = p.expr()?;
    p.skip_nl();
    if !p.at_eof() {
        return Err(p.err("nadmiarowy tekst w wyrażeniu"));
    }
    Ok(e)
}

impl Parser {
    fn err(&self, msg: impl Into<String>) -> Diag {
        Diag::new(&self.file, self.line(), msg)
    }
    fn line(&self) -> usize {
        self.toks.get(self.pos).map(|t| t.line).unwrap_or(0)
    }
    fn peek(&self) -> &Tok {
        &self.toks[self.pos.min(self.toks.len() - 1)].tok
    }
    fn peek_at(&self, n: usize) -> &Tok {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].tok
    }
    fn next(&mut self) -> Tok {
        let t = self.peek().clone();
        if self.pos < self.toks.len() {
            self.pos += 1;
        }
        t
    }
    fn at_eof(&self) -> bool {
        matches!(self.peek(), Tok::Eof)
    }
    fn is_sym(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Sym(x) if *x == s)
    }
    fn is_kw(&self, s: &str) -> bool {
        matches!(self.peek(), Tok::Ident(x) if x == s)
    }
    fn eat_sym(&mut self, s: &str) -> bool {
        if self.is_sym(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, s: &str) -> bool {
        if self.is_kw(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_sym(&mut self, s: &str) -> R<()> {
        if self.eat_sym(s) {
            Ok(())
        } else {
            Err(self.err(format!("oczekiwano `{}`, jest {}", s, show(self.peek()))))
        }
    }
    fn ident(&mut self) -> R<String> {
        match self.peek().clone() {
            Tok::Ident(s) if !KEYWORDS.contains(&s.as_str()) => {
                self.pos += 1;
                Ok(s)
            }
            t => Err(self.err(format!("oczekiwano nazwy, jest {}", show(&t)))),
        }
    }
    fn skip_nl(&mut self) {
        while matches!(self.peek(), Tok::Newline) {
            self.pos += 1;
        }
    }
    fn end_line(&mut self) -> R<()> {
        match self.peek() {
            Tok::Newline => {
                self.pos += 1;
                Ok(())
            }
            Tok::Dedent | Tok::Eof => Ok(()),
            t => Err(self.err(format!("oczekiwano końca linii, jest {}", show(t)))),
        }
    }

    fn file_items(&mut self, path: &str, module: &str, is_impl: bool) -> R<SourceFile> {
        let mut f = SourceFile {
            path: path.into(),
            module: module.into(),
            is_impl,
            header: vec![],
            items: vec![],
            stray: vec![],
        };
        self.skip_nl();
        while !self.at_eof() {
            match self.peek().clone() {
                Tok::Directive(kind, text, n) => {
                    let d = Directive {
                        kind,
                        text,
                        line: self.line(),
                        lines: n,
                    };
                    self.pos += 1;
                    if f.items.is_empty() {
                        f.header.push(d);
                    } else {
                        f.stray.push(d);
                    }
                    self.end_line()?;
                }
                Tok::Ident(k) if k == "type" => {
                    let t = self.type_decl()?;
                    f.items.push(Item::Type(t));
                }
                Tok::Ident(k) if k == "fn" => {
                    let d = self.fn_decl()?;
                    f.items.push(Item::Fn(d));
                }
                t => return Err(self.err(format!("oczekiwano `fn` albo `type`, jest {}", show(&t)))),
            }
            self.skip_nl();
        }
        Ok(f)
    }

    fn directives_block(&mut self, out: &mut Vec<Directive>) -> R<()> {
        while let Tok::Directive(kind, text, n) = self.peek().clone() {
            out.push(Directive {
                kind,
                text,
                line: self.line(),
                lines: n,
            });
            self.pos += 1;
            self.end_line()?;
            self.skip_nl();
        }
        Ok(())
    }

    fn type_decl(&mut self) -> R<TypeDecl> {
        let line = self.line();
        self.pos += 1; // type
        let name = self.ident()?;
        let mut directives = vec![];
        if self.eat_sym("=") {
            let rhs = self.type_expr()?;
            let terms = match rhs {
                TypeExpr::Union(v) => v,
                t => vec![t],
            };
            self.end_line()?;
            if matches!(self.peek(), Tok::Indent) {
                self.pos += 1;
                self.skip_nl();
                self.directives_block(&mut directives)?;
                if !matches!(self.peek(), Tok::Dedent) {
                    return Err(self.err("pod typem mogą stać tylko desc, doc i why"));
                }
                self.pos += 1;
            }
            return Ok(TypeDecl {
                name,
                line,
                body: TypeBody::Rhs(terms),
                directives,
            });
        }
        self.end_line()?;
        let mut fields = vec![];
        if matches!(self.peek(), Tok::Indent) {
            self.pos += 1;
            self.skip_nl();
            self.directives_block(&mut directives)?;
            while !matches!(self.peek(), Tok::Dedent | Tok::Eof) {
                let fl = self.line();
                let fname = self.ident()?;
                self.expect_sym(":")?;
                let ty = self.type_expr()?;
                fields.push(Field { name: fname, ty, line: fl });
                self.end_line()?;
                self.skip_nl();
            }
            self.pos += 1;
        }
        Ok(TypeDecl {
            name,
            line,
            body: TypeBody::Record(fields),
            directives,
        })
    }

    pub fn type_expr(&mut self) -> R<TypeExpr> {
        let mut alts = vec![self.type_term()?];
        while self.is_sym("|") {
            self.pos += 1;
            alts.push(self.type_term()?);
        }
        if alts.len() == 1 {
            Ok(alts.pop().unwrap())
        } else {
            Ok(TypeExpr::Union(alts))
        }
    }

    fn type_term(&mut self) -> R<TypeExpr> {
        let line = self.line();
        let name = self.ident()?;
        let mut args = vec![];
        if self.is_sym("<") {
            self.pos += 1;
            loop {
                args.push(self.type_expr()?);
                if !self.eat_sym(",") {
                    break;
                }
            }
            self.expect_sym(">")?;
        }
        let mut cond = None;
        let mut fields = None;
        if self.is_sym("(") {
            self.pos += 1;
            let is_fields = matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym(":"));
            let is_named = matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym("=>"));
            if is_fields {
                let mut fs = vec![];
                while !self.is_sym(")") {
                    let fl = self.line();
                    let fname = self.ident()?;
                    self.expect_sym(":")?;
                    let ty = self.type_expr()?;
                    fs.push(Field { name: fname, ty, line: fl });
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                fields = Some(fs);
            } else if is_named {
                let param = self.ident()?;
                self.expect_sym("=>")?;
                let e = self.expr()?;
                cond = Some(Box::new(Cond { param, named: true, expr: e }));
            } else {
                let e = self.expr()?;
                cond = Some(Box::new(Cond {
                    param: "α".into(),
                    named: false,
                    expr: e,
                }));
            }
            self.expect_sym(")")?;
        }
        Ok(TypeExpr::Name {
            name,
            args,
            cond,
            fields,
            line,
        })
    }

    fn fn_decl(&mut self) -> R<FnDecl> {
        let line = self.line();
        self.pos += 1; // fn
        let name = self.ident()?;
        self.expect_sym("(")?;
        let mut params = vec![];
        while !self.is_sym(")") {
            let pl = self.line();
            let pname = self.ident()?;
            self.expect_sym(":")?;
            let ty = self.type_expr()?;
            params.push(Param { name: pname, ty, line: pl });
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        let ret = if self.eat_sym("->") { Some(self.type_expr()?) } else { None };
        self.end_line()?;
        let mut d = FnDecl {
            name,
            line,
            params,
            ret,
            directives: vec![],
            examples: vec![],
            properties: vec![],
            body: None,
            order: vec![],
        };
        if !matches!(self.peek(), Tok::Indent) {
            return Ok(d);
        }
        self.pos += 1;
        self.skip_nl();
        let mut body = vec![];
        loop {
            self.skip_nl();
            match self.peek().clone() {
                Tok::Dedent => {
                    self.pos += 1;
                    break;
                }
                Tok::Eof => break,
                Tok::Directive(kind, text, n) => {
                    if !body.is_empty() {
                        return Err(self.err(format!("`{}` musi stać przed kodem funkcji", kind)));
                    }
                    d.order.push((kind.clone(), self.line()));
                    d.directives.push(Directive {
                        kind,
                        text,
                        line: self.line(),
                        lines: n,
                    });
                    self.pos += 1;
                    self.end_line()?;
                }
                Tok::Ident(k) if k == "example" => {
                    let el = self.line();
                    self.pos += 1;
                    d.order.push(("example".into(), el));
                    if matches!(self.peek(), Tok::Newline) && matches!(self.peek_at(1), Tok::Indent) {
                        let start = self.pos;
                        let stmts = self.block()?;
                        let end_line = self.toks[self.pos.saturating_sub(1)].line;
                        let _ = start;
                        d.examples.push(Example {
                            line: el,
                            stmts,
                            multiline: true,
                            nlines: end_line.saturating_sub(el).max(1),
                        });
                    } else {
                        let e = self.expr()?;
                        let last = self.toks[self.pos.saturating_sub(1)].line;
                        self.end_line()?;
                        d.examples.push(Example {
                            line: el,
                            stmts: vec![Stmt::Expr { expr: e, line: el }],
                            multiline: false,
                            nlines: last - el + 1,
                        });
                    }
                }
                Tok::Ident(k) if k == "property" => {
                    let pl = self.line();
                    self.pos += 1;
                    d.order.push(("property".into(), pl));
                    let e = self.expr()?;
                    self.end_line()?;
                    d.properties.push(Property { line: pl, expr: e });
                }
                _ => {
                    body.push(self.stmt()?);
                }
            }
        }
        if !body.is_empty() {
            d.body = Some(body);
        }
        Ok(d)
    }

    fn block(&mut self) -> R<Vec<Stmt>> {
        if matches!(self.peek(), Tok::Newline) {
            self.pos += 1;
        }
        if !matches!(self.peek(), Tok::Indent) {
            return Err(self.err("oczekiwano bloku z wcięciem"));
        }
        self.pos += 1;
        let mut out = vec![];
        loop {
            self.skip_nl();
            match self.peek() {
                Tok::Dedent => {
                    self.pos += 1;
                    break;
                }
                Tok::Eof => break,
                _ => out.push(self.stmt()?),
            }
        }
        Ok(out)
    }

    fn stmt(&mut self) -> R<Stmt> {
        let line = self.line();
        if self.eat_kw("return") {
            if matches!(self.peek(), Tok::Newline | Tok::Dedent | Tok::Eof) {
                self.end_line()?;
                return Ok(Stmt::Return { expr: None, line });
            }
            let e = self.expr()?;
            self.end_stmt()?;
            return Ok(Stmt::Return { expr: Some(e), line });
        }
        if self.eat_kw("var") {
            let name = self.ident()?;
            self.expect_sym("=")?;
            let e = self.expr()?;
            self.end_stmt()?;
            return Ok(Stmt::Set {
                name,
                is_var: true,
                expr: e,
                line,
            });
        }
        if self.eat_kw("if") {
            let cond = self.expr()?;
            let then = self.block()?;
            let mut els = None;
            if self.is_kw("else") {
                self.pos += 1;
                if self.is_kw("if") {
                    els = Some(vec![self.stmt()?]);
                } else {
                    els = Some(self.block()?);
                }
            }
            return Ok(Stmt::If { cond, then, els, line });
        }
        if self.eat_kw("for") {
            let var = self.ident()?;
            if !self.eat_kw("in") {
                return Err(self.err("oczekiwano `in`"));
            }
            let iter = self.expr()?;
            let body = self.block()?;
            return Ok(Stmt::For { var, iter, body, line });
        }
        if self.eat_kw("while") {
            let cond = self.expr()?;
            let body = self.block()?;
            return Ok(Stmt::While { cond, body, line });
        }
        if self.eat_kw("match") {
            let mut subjects = vec![self.expr()?];
            while self.eat_sym(",") {
                subjects.push(self.expr()?);
            }
            self.end_line()?;
            if !matches!(self.peek(), Tok::Indent) {
                return Err(self.err("oczekiwano gałęzi `match` z wcięciem"));
            }
            self.pos += 1;
            let mut arms = vec![];
            loop {
                self.skip_nl();
                if matches!(self.peek(), Tok::Dedent) {
                    self.pos += 1;
                    break;
                }
                if self.at_eof() {
                    break;
                }
                arms.push(self.arm()?);
            }
            return Ok(Stmt::Match { subjects, arms, line });
        }
        if let (Tok::Ident(n), Tok::Sym("=")) = (self.peek().clone(), self.peek_at(1).clone()) {
            if !KEYWORDS.contains(&n.as_str()) {
                self.pos += 2;
                let e = self.expr()?;
                self.end_stmt()?;
                return Ok(Stmt::Set {
                    name: n,
                    is_var: false,
                    expr: e,
                    line,
                });
            }
        }
        let e = self.expr()?;
        self.end_stmt()?;
        Ok(Stmt::Expr { expr: e, line })
    }

    // Po wyrażeniu kończącym się blokiem (`or` z blokiem, lambda z blokiem) nie ma już końca linii.
    fn end_stmt(&mut self) -> R<()> {
        if matches!(self.toks.get(self.pos.wrapping_sub(1)).map(|t| &t.tok), Some(Tok::Dedent)) {
            return Ok(());
        }
        self.end_line()
    }

    fn arm(&mut self) -> R<Arm> {
        let line = self.line();
        let mut pats = vec![self.pattern()?];
        while self.eat_sym(",") {
            pats.push(self.pattern()?);
        }
        self.expect_sym("=>")?;
        let body = if matches!(self.peek(), Tok::Newline) {
            self.block()?
        } else {
            vec![self.stmt()?]
        };
        Ok(Arm { pats, body, line })
    }

    fn pattern(&mut self) -> R<Pat> {
        match self.peek().clone() {
            Tok::Str(s) => {
                self.pos += 1;
                Ok(Pat::Str(s))
            }
            Tok::Sym("[") => {
                self.pos += 1;
                let mut v = vec![];
                while !self.is_sym("]") {
                    v.push(self.pattern()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                Ok(Pat::List(v))
            }
            Tok::Ident(n) if n == "_" => {
                self.pos += 1;
                Ok(Pat::Wild)
            }
            Tok::Ident(n) => {
                self.pos += 1;
                if n.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                    let bind = if let Tok::Ident(b) = self.peek().clone() {
                        self.pos += 1;
                        Some(b)
                    } else {
                        None
                    };
                    Ok(Pat::Name { name: n, bind })
                } else {
                    Ok(Pat::Bind(n))
                }
            }
            t => Err(self.err(format!("oczekiwano wzorca, jest {}", show(&t)))),
        }
    }

    // Wyrażenia, od najniższego priorytetu.
    pub fn expr(&mut self) -> R<Expr> {
        if let (Tok::Ident(n), Tok::Sym("=>")) = (self.peek().clone(), self.peek_at(1).clone()) {
            if !KEYWORDS.contains(&n.as_str()) {
                let line = self.line();
                self.pos += 2;
                let body = if matches!(self.peek(), Tok::Indent) || (matches!(self.peek(), Tok::Newline) && matches!(self.peek_at(1), Tok::Indent)) {
                    LambdaBody::Block(self.block()?)
                } else {
                    LambdaBody::Expr(Box::new(self.expr()?))
                };
                return Ok(Expr {
                    kind: ExprKind::Lambda { param: n, body },
                    line,
                });
            }
        }
        self.or_expr()
    }

    fn bin(l: Expr, op: &'static str, r: Expr) -> Expr {
        let line = l.line;
        Expr {
            kind: ExprKind::Bin {
                op,
                l: Box::new(l),
                r: Box::new(r),
            },
            line,
        }
    }

    fn or_expr(&mut self) -> R<Expr> {
        let mut l = self.and_expr()?;
        while self.eat_sym("||") {
            let r = self.and_expr()?;
            l = Self::bin(l, "||", r);
        }
        // `or` bez `as` przed nim: to nie jest lub logiczne.
        if self.is_kw("or") {
            return Err(self.err("`or` stoi tylko po `as Typ`; lub logiczne to `||`"));
        }
        Ok(l)
    }

    fn and_expr(&mut self) -> R<Expr> {
        let mut l = self.not_expr()?;
        while self.eat_sym("&&") {
            let r = self.not_expr()?;
            l = Self::bin(l, "&&", r);
        }
        Ok(l)
    }

    fn not_expr(&mut self) -> R<Expr> {
        let line = self.line();
        if self.eat_kw("not") {
            let e = self.not_expr()?;
            return Ok(Expr {
                kind: ExprKind::Not(Box::new(e)),
                line,
            });
        }
        self.cmp_expr()
    }

    fn cmp_expr(&mut self) -> R<Expr> {
        let l = self.as_expr()?;
        for op in ["==", "!=", "<=", ">=", "<", ">"] {
            if self.is_sym(op) {
                self.pos += 1;
                let r = self.as_expr()?;
                let op: &'static str = match op {
                    "==" => "==",
                    "!=" => "!=",
                    "<=" => "<=",
                    ">=" => ">=",
                    "<" => "<",
                    _ => ">",
                };
                return Ok(Self::bin(l, op, r));
            }
        }
        if self.is_kw("is") {
            self.pos += 1;
            let neg = self.eat_kw("not");
            let ty = self.type_expr()?;
            let line = l.line;
            return Ok(Expr {
                kind: ExprKind::Is { e: Box::new(l), ty, neg },
                line,
            });
        }
        Ok(l)
    }

    fn as_expr(&mut self) -> R<Expr> {
        let mut e = self.add_expr()?;
        loop {
            if self.is_kw("as") {
                self.pos += 1;
                let ty = self.type_expr()?;
                let mut alt = None;
                if self.eat_kw("or") {
                    if self.eat_kw("return") {
                        if matches!(self.peek(), Tok::Newline | Tok::Dedent | Tok::Eof | Tok::Sym(")")) {
                            alt = Some(Box::new(Alt::Return(None)));
                        } else {
                            alt = Some(Box::new(Alt::Return(Some(self.expr()?))));
                        }
                    } else if matches!(self.peek(), Tok::Newline) && matches!(self.peek_at(1), Tok::Indent) {
                        alt = Some(Box::new(Alt::Block(self.block()?)));
                    } else {
                        // Wartość domyślna to całe wyrażenie, jak po `return`:
                        // `x as Flag or b > 0` to `x as Flag or (b > 0)`.
                        alt = Some(Box::new(Alt::Value(self.expr()?)));
                    }
                }
                let line = e.line;
                e = Expr {
                    kind: ExprKind::As { e: Box::new(e), ty, alt },
                    line,
                };
                continue;
            }
            if self.is_kw("with") {
                self.pos += 1;
                let mut fields = vec![];
                loop {
                    let n = self.ident()?;
                    self.expect_sym(":")?;
                    // Wartość pola to całe wyrażenie do przecinka, jak argument w P(pole: wartość):
                    // `p with ok: p.n >= 0` to `p with ok: (p.n >= 0)`.
                    let v = self.expr()?;
                    fields.push((n, v));
                    // Kolejne pole tylko wtedy, gdy po przecinku stoi `nazwa:`.
                    if self.is_sym(",") && matches!(self.peek_at(1), Tok::Ident(_)) && matches!(self.peek_at(2), Tok::Sym(":")) {
                        self.pos += 1;
                        continue;
                    }
                    break;
                }
                let line = e.line;
                e = Expr {
                    kind: ExprKind::With { e: Box::new(e), fields },
                    line,
                };
                continue;
            }
            return Ok(e);
        }
    }

    fn add_expr(&mut self) -> R<Expr> {
        let mut l = self.mul_expr()?;
        loop {
            let op = if self.is_sym("+") {
                "+"
            } else if self.is_sym("-") {
                "-"
            } else {
                return Ok(l);
            };
            self.pos += 1;
            let r = self.mul_expr()?;
            l = Self::bin(l, op, r);
        }
    }

    fn mul_expr(&mut self) -> R<Expr> {
        let mut l = self.unary()?;
        loop {
            let op = if self.is_sym("*") {
                "*"
            } else if self.is_sym("/") {
                "/"
            } else if self.is_sym("%") {
                "%"
            } else {
                return Ok(l);
            };
            self.pos += 1;
            let r = self.unary()?;
            l = Self::bin(l, op, r);
        }
    }

    fn unary(&mut self) -> R<Expr> {
        let line = self.line();
        if self.eat_sym("-") {
            let e = self.unary()?;
            return Ok(Expr {
                kind: ExprKind::Neg(Box::new(e)),
                line,
            });
        }
        if self.eat_kw("try") {
            let e = self.unary()?;
            return Ok(Expr {
                kind: ExprKind::Try(Box::new(e)),
                line,
            });
        }
        self.postfix()
    }

    fn args(&mut self) -> R<Vec<Arg>> {
        // `(` już zjedzony.
        let mut args = vec![];
        while !self.is_sym(")") {
            let named = matches!(self.peek(), Tok::Ident(_)) && matches!(self.peek_at(1), Tok::Sym(":"));
            if named {
                let n = self.ident()?;
                self.pos += 1;
                let v = self.expr()?;
                args.push(Arg { name: Some(n), value: v });
            } else {
                let v = self.expr()?;
                args.push(Arg { name: None, value: v });
            }
            if !self.eat_sym(",") {
                break;
            }
        }
        self.expect_sym(")")?;
        Ok(args)
    }

    fn try_type_args(&mut self) -> Option<Vec<TypeExpr>> {
        let save = self.pos;
        if !self.eat_sym("<") {
            return None;
        }
        let mut v = vec![];
        loop {
            match self.type_expr() {
                Ok(t) => v.push(t),
                Err(_) => {
                    self.pos = save;
                    return None;
                }
            }
            if !self.eat_sym(",") {
                break;
            }
        }
        if self.eat_sym(">") && self.is_sym("(") {
            return Some(v);
        }
        self.pos = save;
        None
    }

    fn postfix(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.is_sym("(") {
                if let ExprKind::Ident(name) = &e.kind {
                    let name = name.clone();
                    self.pos += 1;
                    let args = self.args()?;
                    e = Expr {
                        kind: ExprKind::Call { name, args },
                        line: e.line,
                    };
                    continue;
                }
                return Err(self.err("wywołać można tylko funkcję po nazwie"));
            }
            if self.is_sym(".") {
                self.pos += 1;
                let name = self.ident()?;
                let targs = self.try_type_args().unwrap_or_default();
                if self.is_sym("(") {
                    self.pos += 1;
                    let args = self.args()?;
                    e = Expr {
                        kind: ExprKind::Method {
                            obj: Box::new(e),
                            name,
                            targs,
                            args,
                        },
                        line: self.line(),
                    };
                } else {
                    let line = e.line;
                    e = Expr {
                        kind: ExprKind::Field { obj: Box::new(e), name },
                        line,
                    };
                }
                continue;
            }
            return Ok(e);
        }
    }

    fn primary(&mut self) -> R<Expr> {
        let line = self.line();
        let t = self.next();
        let kind = match t {
            Tok::Int(n) => ExprKind::Int(n),
            Tok::Dec(s) => ExprKind::Dec(s),
            Tok::Str(s) => ExprKind::Str(s),
            Tok::Html(parts) => {
                let mut segs = vec![];
                for p in parts {
                    match p {
                        HtmlPart::Lit(s) => segs.push(HtmlSeg::Lit(s)),
                        HtmlPart::Expr(src, l) => segs.push(HtmlSeg::Expr(parse_expr_src(&src, &self.file, l)?)),
                    }
                }
                ExprKind::Html(segs)
            }
            Tok::Ident(s) if s == "true" => ExprKind::Bool(true),
            Tok::Ident(s) if s == "false" => ExprKind::Bool(false),
            Tok::Ident(s) if !KEYWORDS.contains(&s.as_str()) => ExprKind::Ident(s),
            Tok::Sym("[") => {
                let mut v = vec![];
                while !self.is_sym("]") {
                    v.push(self.expr()?);
                    if !self.eat_sym(",") {
                        break;
                    }
                }
                self.expect_sym("]")?;
                ExprKind::List(v)
            }
            Tok::Sym("(") => {
                let e = self.expr()?;
                self.expect_sym(")")?;
                return Ok(e);
            }
            t => {
                self.pos -= 1;
                return Err(self.err(format!("oczekiwano wyrażenia, jest {}", show(&t))));
            }
        };
        Ok(Expr { kind, line })
    }
}

fn show(t: &Tok) -> String {
    match t {
        Tok::Ident(s) => format!("`{}`", s),
        Tok::Int(n) => format!("`{}`", n),
        Tok::Dec(s) => format!("`{}`", s),
        Tok::Str(s) => format!("tekst {}", quote(s)),
        Tok::Html(_) => "html\"...\"".into(),
        Tok::Directive(k, _, _) => format!("`{}`", k),
        Tok::Sym(s) => format!("`{}`", s),
        Tok::Newline => "koniec linii".into(),
        Tok::Indent => "wcięcie".into(),
        Tok::Dedent => "koniec bloku".into(),
        Tok::Eof => "koniec pliku".into(),
    }
}
