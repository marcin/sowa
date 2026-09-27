// Wczytanie projektu: sowa.toml, pliki .sowa z src/ i impl/, dokumentacja z docs/.

use crate::ast::*;
use crate::parser::parse_file;
use crate::toml;
use std::fs;
use std::path::{Path, PathBuf};

pub struct MdFile {
    pub rel: String,
    pub anchors: Vec<String>,
    pub refs: Vec<(String, usize)>,
    // (linia pierwszej linii kodu, kod)
    pub blocks: Vec<(usize, String)>,
}

pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub toml: toml::Doc,
    pub src_dir: String,
    pub impl_dir: String,
    pub docs_dir: String,
    pub files: Vec<SourceFile>,
    pub docs: Vec<MdFile>,
    pub codeowners: Vec<String>,
    pub gitattributes: Vec<(String, Vec<String>)>,
}

pub fn find_root(start: &Path) -> Option<PathBuf> {
    let mut p = fs::canonicalize(start).ok()?;
    loop {
        if p.join("sowa.toml").is_file() {
            return Some(p);
        }
        if !p.pop() {
            return None;
        }
    }
}

fn walk(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            walk(&p, ext, out);
        } else if p.extension().map(|e| e == ext).unwrap_or(false) {
            out.push(p);
        }
    }
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

pub fn load(root: &Path) -> Result<Project, Vec<Diag>> {
    let mut diags = vec![];
    let toml_src = fs::read_to_string(root.join("sowa.toml")).map_err(|e| vec![Diag::new("sowa.toml", 0, e.to_string())])?;
    let doc = toml::parse(&toml_src).map_err(|(l, m)| vec![Diag::new("sowa.toml", l, m)])?;
    let get = |k: &str, def: &str| -> String {
        doc.get("project")
            .and_then(|es| es.iter().find(|e| e.key == k))
            .and_then(|e| if let toml::Value::Str(s) = &e.value { Some(s.clone()) } else { None })
            .unwrap_or_else(|| def.to_string())
    };
    let name = get("name", "app");
    let src_dir = get("src", "src");
    let impl_dir = get("impl", "impl");
    let docs_dir = get("docs", "docs");

    let mut files = vec![];
    for (dir, is_impl) in [(&src_dir, false), (&impl_dir, true)] {
        let mut paths = vec![];
        walk(&root.join(dir), "sowa", &mut paths);
        for p in paths {
            let r = rel(root, &p);
            let module = rel(&root.join(dir), &p).trim_end_matches(".sowa").to_string();
            match fs::read_to_string(&p) {
                Ok(src) => match parse_file(&src, &r, &module, is_impl) {
                    Ok(f) => files.push(f),
                    Err(d) => diags.push(d),
                },
                Err(e) => diags.push(Diag::new(&r, 0, e.to_string())),
            }
        }
    }

    let mut docs = vec![];
    let mut mds = vec![];
    let docs_root = root.join(&docs_dir);
    walk(&docs_root, "md", &mut mds);
    for p in mds {
        let src = fs::read_to_string(&p).unwrap_or_default();
        docs.push(read_md(&rel(&docs_root, &p), &src));
    }

    let codeowners = [".github/CODEOWNERS", "CODEOWNERS", "docs/CODEOWNERS"]
        .iter()
        .find_map(|p| fs::read_to_string(root.join(p)).ok())
        .map(|s| {
            s.lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .filter_map(|l| l.split_whitespace().next().map(|x| x.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let gitattributes = fs::read_to_string(root.join(".gitattributes"))
        .map(|s| {
            s.lines()
                .map(|l| l.trim())
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .map(|l| {
                    let mut it = l.split_whitespace();
                    let pat = it.next().unwrap_or("").to_string();
                    (pat, it.map(|x| x.to_string()).collect())
                })
                .collect()
        })
        .unwrap_or_default();

    if !diags.is_empty() {
        return Err(diags);
    }
    Ok(Project {
        root: root.to_path_buf(),
        name,
        toml: doc,
        src_dir,
        impl_dir,
        docs_dir,
        files,
        docs,
        codeowners,
        gitattributes,
    })
}

// Kotwica jak na GitHubie: małe litery, bez interpunkcji, spacje na `-`. Polskie litery zostają.
pub fn slug(title: &str) -> String {
    let mut s = String::new();
    for c in title.trim().chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            s.extend(c.to_lowercase());
        } else if c == ' ' {
            s.push('-');
        }
    }
    s
}

fn read_md(rel: &str, src: &str) -> MdFile {
    let mut md = MdFile {
        rel: rel.to_string(),
        anchors: vec![],
        refs: vec![],
        blocks: vec![],
    };
    let mut in_code: Option<(bool, usize, String)> = None;
    for (i, line) in src.lines().enumerate() {
        let n = i + 1;
        let t = line.trim_start();
        if t.starts_with("```") {
            match in_code.take() {
                Some((is_sowa, start, code)) => {
                    if is_sowa {
                        md.blocks.push((start, code));
                    }
                }
                None => in_code = Some((t[3..].trim() == "sowa", n + 1, String::new())),
            }
            continue;
        }
        if let Some((_, _, code)) = in_code.as_mut() {
            code.push_str(line);
            code.push('\n');
            continue;
        }
        if t.starts_with('#') {
            let title = t.trim_start_matches('#');
            md.anchors.push(slug(title));
        }
        // Odnośniki {Symbol} poza `kodem`.
        let mut in_tick = false;
        let cs: Vec<char> = line.chars().collect();
        let mut j = 0;
        while j < cs.len() {
            if cs[j] == '`' {
                in_tick = !in_tick;
            } else if cs[j] == '{' && !in_tick {
                let mut k = j + 1;
                let mut name = String::new();
                while k < cs.len() && (cs[k].is_alphanumeric() || cs[k] == '_') {
                    name.push(cs[k]);
                    k += 1;
                }
                if k < cs.len() && cs[k] == '}' && !name.is_empty() {
                    md.refs.push((name, n));
                }
            }
            j += 1;
        }
    }
    md
}
