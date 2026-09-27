// Lekser z wcięciami. Zasady:
// - w nawiasach nowa linia nic nie znaczy, chyba że linia kończy się `=>`: wtedy pod spodem
//   jest blok lambdy z wcięciem, który kończy się linią o mniejszym wcięciu (np. `)`),
// - linia zaczynająca się od `|`, `->`, `==`, `!=` albo `&&` jest ciągiem poprzedniej,
// - `desc`, `doc` i `why` na początku linii to tekst dosłowny do końca linii
//   (samo `desc` bierze linie z większym wcięciem),
// - html"..." w jednej linii kończy się na `"` poza `{}`, a html" na końcu linii
//   kończy się linią, która po wcięciu zaczyna się od `"`.

use crate::ast::Diag;

#[derive(Debug, Clone, PartialEq)]
pub enum HtmlPart {
    Lit(String),
    Expr(String, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Dec(String),
    Str(String),
    Html(Vec<HtmlPart>),
    Directive(String, String, usize),
    Sym(&'static str),
    Newline,
    Indent,
    Dedent,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

enum Ctx {
    Block { indents: Vec<usize>, lambda: bool },
    Bracket(char),
}

const SYMS2: [&str; 8] = ["==", "!=", "<=", ">=", "=>", "->", "&&", "||"];
const SYMS1: [&str; 18] = ["(", ")", "[", "]", "{", "}", ",", ".", ":", "=", "+", "-", "*", "/", "%", "<", ">", "|"];

pub struct Lexer {
    c: Vec<char>,
    pos: usize,
    line: usize,
    toks: Vec<Token>,
    ctx: Vec<Ctx>,
    pending_nl: bool,
    file: String,
}

pub fn lex(src: &str, file: &str, first_line: usize) -> Result<Vec<Token>, Diag> {
    let mut lx = Lexer {
        c: src.chars().collect(),
        pos: 0,
        line: first_line,
        toks: vec![],
        ctx: vec![Ctx::Block {
            indents: vec![0],
            lambda: false,
        }],
        pending_nl: false,
        file: file.to_string(),
    };
    lx.run()?;
    Ok(lx.toks)
}

impl Lexer {
    fn err(&self, msg: impl Into<String>) -> Diag {
        Diag::new(&self.file, self.line, msg)
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.pos).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.c.get(self.pos + 1).copied()
    }
    fn push(&mut self, tok: Tok) {
        self.toks.push(Token { tok, line: self.line });
    }
    fn last_is(&self, s: &str) -> bool {
        matches!(self.toks.last(), Some(Token { tok: Tok::Sym(x), .. }) if *x == s)
    }
    fn rest_of_line(&self) -> String {
        let mut i = self.pos;
        let mut s = String::new();
        while i < self.c.len() && self.c[i] != '\n' {
            s.push(self.c[i]);
            i += 1;
        }
        s
    }
    fn in_block(&self) -> bool {
        matches!(self.ctx.last(), Some(Ctx::Block { .. }))
    }

    fn run(&mut self) -> Result<(), Diag> {
        let mut line_start = true;
        loop {
            if line_start {
                line_start = false;
                if !self.line_start()? {
                    if self.pos >= self.c.len() {
                        break;
                    }
                    line_start = true;
                    continue;
                }
            }
            while matches!(self.peek(), Some(' ') | Some('\r')) {
                self.pos += 1;
            }
            let Some(ch) = self.peek() else { break };
            if ch == '\t' {
                return Err(self.err("tabulator: Sowa używa spacji"));
            }
            if ch == '\n' {
                self.pos += 1;
                if self.in_block() && !self.toks.is_empty() {
                    self.pending_nl = true;
                }
                self.line += 1;
                line_start = true;
                continue;
            }
            if ch == '/' && self.peek2() == Some('/') {
                while !matches!(self.peek(), None | Some('\n')) {
                    self.pos += 1;
                }
                continue;
            }
            self.token()?;
        }
        // Koniec pliku: zamknij linię i wszystkie bloki.
        if self.ctx.len() > 1 {
            if let Some(Ctx::Bracket(c)) = self.ctx.last() {
                return Err(self.err(format!("niezamknięty nawias `{}`", c)));
            }
        }
        if !matches!(self.toks.last(), None | Some(Token { tok: Tok::Newline, .. })) {
            self.push(Tok::Newline);
        }
        while let Some(ctx) = self.ctx.pop() {
            match ctx {
                Ctx::Block { indents, .. } => {
                    for _ in 1..indents.len() {
                        self.push(Tok::Dedent);
                    }
                }
                Ctx::Bracket(c) => return Err(self.err(format!("niezamknięty nawias `{}`", c))),
            }
        }
        self.push(Tok::Eof);
        Ok(())
    }

    fn emit_newline(&mut self) {
        if !matches!(
            self.toks.last(),
            None | Some(Token { tok: Tok::Newline, .. }) | Some(Token { tok: Tok::Indent, .. })
        ) {
            self.push(Tok::Newline);
        }
    }

    // Zwraca false, gdy linia jest pusta albo to sam komentarz (została pominięta).
    fn line_start(&mut self) -> Result<bool, Diag> {
        let mut indent = 0;
        let mut i = self.pos;
        while i < self.c.len() && self.c[i] == ' ' {
            indent += 1;
            i += 1;
        }
        if i < self.c.len() && self.c[i] == '\t' {
            self.line_err_tab()?;
        }
        let blank = i >= self.c.len() || self.c[i] == '\n' || self.c[i] == '\r' || (self.c[i] == '/' && self.c.get(i + 1) == Some(&'/'));
        if blank {
            while self.pos < self.c.len() && self.c[self.pos] != '\n' {
                self.pos += 1;
            }
            if self.pos < self.c.len() {
                self.pos += 1;
                self.line += 1;
            }
            return Ok(false);
        }
        self.pos = i;
        match self.ctx.last() {
            Some(Ctx::Bracket(_)) => {
                if self.last_is("=>") {
                    self.ctx.push(Ctx::Block {
                        indents: vec![indent],
                        lambda: true,
                    });
                    self.push(Tok::Indent);
                    self.directive(indent)?;
                }
                Ok(true)
            }
            _ => {
                let rest = self.rest_of_line();
                let cont = ["|", "->", "==", "!=", "&&"].iter().any(|p| rest.starts_with(p));
                if self.pending_nl && cont {
                    self.pending_nl = false;
                    return Ok(true);
                }
                if self.pending_nl {
                    self.emit_newline();
                    self.pending_nl = false;
                }
                self.indent_to(indent)?;
                if self.in_block() {
                    self.directive(indent)?;
                }
                Ok(true)
            }
        }
    }

    fn line_err_tab(&self) -> Result<(), Diag> {
        Err(self.err("tabulator: Sowa używa spacji"))
    }

    fn indent_to(&mut self, indent: usize) -> Result<(), Diag> {
        let top = match self.ctx.last() {
            Some(Ctx::Block { indents, .. }) => *indents.last().unwrap(),
            _ => return Ok(()),
        };
        if indent > top {
            if let Some(Ctx::Block { indents, .. }) = self.ctx.last_mut() {
                indents.push(indent);
            }
            self.push(Tok::Indent);
            return Ok(());
        }
        loop {
            let (cur, len, lambda) = match self.ctx.last() {
                Some(Ctx::Block { indents, lambda }) => (*indents.last().unwrap(), indents.len(), *lambda),
                _ => return Ok(()),
            };
            if indent >= cur {
                if indent != cur {
                    return Err(self.err("niespójne wcięcie"));
                }
                return Ok(());
            }
            if len == 1 && !lambda {
                return Err(self.err("niespójne wcięcie"));
            }
            if let Some(Ctx::Block { indents, .. }) = self.ctx.last_mut() {
                indents.pop();
            }
            self.push(Tok::Dedent);
            if len == 1 {
                // Koniec bloku lambdy: reszta linii należy do nawiasu, np. `)`.
                self.ctx.pop();
                return Ok(());
            }
        }
    }

    fn directive(&mut self, indent: usize) -> Result<(), Diag> {
        let rest = self.rest_of_line();
        for kind in ["desc", "doc", "why"] {
            let is = rest == kind || rest.starts_with(&format!("{} ", kind));
            if !is {
                continue;
            }
            let start_line = self.line;
            let text = rest[kind.len()..].trim().to_string();
            // Przejdź na koniec linii.
            while !matches!(self.peek(), None | Some('\n')) {
                self.pos += 1;
            }
            if !text.is_empty() || kind != "desc" {
                self.toks.push(Token {
                    tok: Tok::Directive(kind.to_string(), text, 1),
                    line: start_line,
                });
                return Ok(());
            }
            // desc w bloku: linie z większym wcięciem.
            let mut lines: Vec<String> = vec![];
            loop {
                // self.pos stoi na '\n' (albo końcu pliku).
                if self.pos >= self.c.len() {
                    break;
                }
                let mut j = self.pos + 1;
                let mut ind = 0;
                while j < self.c.len() && self.c[j] == ' ' {
                    ind += 1;
                    j += 1;
                }
                let mut k = j;
                let mut l = String::new();
                while k < self.c.len() && self.c[k] != '\n' {
                    l.push(self.c[k]);
                    k += 1;
                }
                if l.trim().is_empty() {
                    // Pusta linia: należy do bloku tylko, jeśli dalej jest jeszcze tekst z wcięciem.
                    let mut m = k;
                    let mut more = false;
                    while m < self.c.len() {
                        let mut ind2 = 0;
                        let mut n = m + 1;
                        while n < self.c.len() && self.c[n] == ' ' {
                            ind2 += 1;
                            n += 1;
                        }
                        if n >= self.c.len() {
                            break;
                        }
                        if self.c[n] == '\n' {
                            m = n;
                            continue;
                        }
                        more = ind2 > indent;
                        break;
                    }
                    if !more {
                        break;
                    }
                    self.pos = k;
                    self.line += 1;
                    lines.push(String::new());
                    continue;
                }
                if ind <= indent {
                    break;
                }
                lines.push(l.trim().to_string());
                self.pos = k;
                self.line += 1;
            }
            let n = lines.iter().filter(|l| !l.is_empty()).count();
            self.toks.push(Token {
                tok: Tok::Directive("desc".into(), lines.join("\n").trim().to_string(), n.max(1)),
                line: start_line,
            });
            return Ok(());
        }
        Ok(())
    }

    fn token(&mut self) -> Result<(), Diag> {
        let ch = self.peek().unwrap();
        if ch.is_alphabetic() || ch == '_' {
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_alphanumeric() || c == '_' {
                    s.push(c);
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if s == "html" && self.peek() == Some('"') {
                return self.html();
            }
            self.push(Tok::Ident(s));
            return Ok(());
        }
        if ch.is_ascii_digit() {
            let mut s = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if self.peek() == Some('.') && self.peek2().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                s.push('.');
                self.pos += 1;
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                self.push(Tok::Dec(s));
            } else {
                let n: i64 = s.parse().map_err(|_| self.err("za duża liczba"))?;
                self.push(Tok::Int(n));
            }
            return Ok(());
        }
        if ch == '"' {
            self.pos += 1;
            let s = self.string_body()?;
            self.push(Tok::Str(s));
            return Ok(());
        }
        let two: String = self.c[self.pos..(self.pos + 2).min(self.c.len())].iter().collect();
        for s in SYMS2 {
            if two == s {
                self.pos += 2;
                self.push(Tok::Sym(s));
                return Ok(());
            }
        }
        for s in SYMS1 {
            if s.starts_with(ch) {
                self.pos += 1;
                match s {
                    "(" | "[" | "{" => self.ctx.push(Ctx::Bracket(ch)),
                    ")" | "]" | "}" => self.close(ch)?,
                    _ => {}
                }
                self.push(Tok::Sym(s));
                return Ok(());
            }
        }
        Err(self.err(format!("nieznany znak `{}`", ch)))
    }

    fn close(&mut self, ch: char) -> Result<(), Diag> {
        // Nawias zamknięty w linii bloku lambdy zamyka też ten blok.
        while let Some(Ctx::Block { lambda: true, indents }) = self.ctx.last() {
            let n = indents.len();
            self.emit_newline();
            for _ in 0..n {
                self.push(Tok::Dedent);
            }
            self.ctx.pop();
        }
        let open = match ch {
            ')' => '(',
            ']' => '[',
            _ => '{',
        };
        match self.ctx.last() {
            Some(Ctx::Bracket(c)) if *c == open => {
                self.ctx.pop();
                Ok(())
            }
            _ => Err(self.err(format!("niepasujący nawias `{}`", ch))),
        }
    }

    fn string_body(&mut self) -> Result<String, Diag> {
        let mut s = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => return Err(self.err("niezamknięty tekst")),
                Some('"') => {
                    self.pos += 1;
                    return Ok(s);
                }
                Some('\\') => {
                    self.pos += 1;
                    let c = self.peek().ok_or_else(|| self.err("niezamknięty tekst"))?;
                    self.pos += 1;
                    s.push(match c {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        c => c,
                    });
                }
                Some(c) => {
                    s.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn html(&mut self) -> Result<(), Diag> {
        let start_line = self.line;
        self.pos += 1; // "
        let rest = self.rest_of_line();
        let content;
        let content_line;
        if rest.trim().is_empty() {
            // Wieloliniowy: do linii, która po wcięciu zaczyna się od `"`.
            while !matches!(self.peek(), None | Some('\n')) {
                self.pos += 1;
            }
            let mut lines: Vec<String> = vec![];
            loop {
                if self.pos >= self.c.len() {
                    return Err(Diag::new(&self.file, start_line, "niezamknięty literał html\""));
                }
                self.pos += 1; // '\n'
                self.line += 1;
                let l = self.rest_of_line();
                if l.trim_start().starts_with('"') {
                    let off = l.find('"').unwrap();
                    self.pos += l[..off].chars().count() + 1;
                    break;
                }
                self.pos += l.chars().count();
                lines.push(l);
            }
            let min = lines
                .iter()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            let body: Vec<String> = lines
                .iter()
                .map(|l| if l.len() >= min { l[min..].trim_end().to_string() } else { String::new() })
                .collect();
            content = body.join("\n") + "\n";
            content_line = start_line + 1;
        } else {
            let mut s = String::new();
            let mut depth = 0;
            loop {
                match self.peek() {
                    None | Some('\n') => return Err(self.err("niezamknięty literał html\"")),
                    Some('"') if depth == 0 => {
                        self.pos += 1;
                        break;
                    }
                    Some(c) => {
                        if c == '{' {
                            depth += 1;
                        } else if c == '}' && depth > 0 {
                            depth -= 1;
                        } else if c == '"' && depth > 0 {
                            // Tekst w wyrażeniu: przepisz do końcowego cudzysłowu.
                            s.push(c);
                            self.pos += 1;
                            while let Some(d) = self.peek() {
                                s.push(d);
                                self.pos += 1;
                                if d == '"' {
                                    break;
                                }
                                if d == '\n' {
                                    return Err(self.err("niezamknięty tekst"));
                                }
                            }
                            continue;
                        }
                        s.push(c);
                        self.pos += 1;
                    }
                }
            }
            content = s;
            content_line = start_line;
        }
        let parts = html_parts(&content, content_line, &self.file)?;
        self.toks.push(Token {
            tok: Tok::Html(parts),
            line: start_line,
        });
        Ok(())
    }
}

fn html_parts(s: &str, first_line: usize, file: &str) -> Result<Vec<HtmlPart>, Diag> {
    let cs: Vec<char> = s.chars().collect();
    let mut parts = vec![];
    let mut lit = String::new();
    let mut i = 0;
    let mut line = first_line;
    while i < cs.len() {
        let c = cs[i];
        if c == '{' {
            let mut depth = 1;
            let mut j = i + 1;
            let mut e = String::new();
            while j < cs.len() {
                let d = cs[j];
                if d == '"' {
                    e.push(d);
                    j += 1;
                    while j < cs.len() && cs[j] != '"' {
                        e.push(cs[j]);
                        j += 1;
                    }
                }
                if j >= cs.len() {
                    break;
                }
                let d = cs[j];
                if d == '{' {
                    depth += 1;
                } else if d == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                e.push(d);
                j += 1;
            }
            if depth != 0 {
                return Err(Diag::new(file, line, "niezamknięte `{` w literale html\""));
            }
            if !lit.is_empty() {
                parts.push(HtmlPart::Lit(std::mem::take(&mut lit)));
            }
            parts.push(HtmlPart::Expr(e.trim().to_string(), line));
            i = j + 1;
            continue;
        }
        if c == '\n' {
            line += 1;
        }
        lit.push(c);
        i += 1;
    }
    if !lit.is_empty() {
        parts.push(HtmlPart::Lit(lit));
    }
    Ok(parts)
}
