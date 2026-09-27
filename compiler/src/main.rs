// sowa check [--ci] | build | run [--fake NAZWA] | test | review --base REF  [--bun] [--panic-abort] [KATALOG]
//
// Kompilator czyta projekt (sowa.toml, src/, impl/, docs/), sprawdza go, tłumaczy program albo
// testy na Rusta i kompiluje je rustc do pliku wykonywalnego w <projekt>/.sowa/. Z --bun zamiast
// tego zapisuje jeden plik JavaScript, który uruchamia Bun. Backend Bun jest tylko do testów
// i porównań: działa, ale nowe funkcje trafiają najpierw do Rusta.

mod ast;
mod check;
mod ci;
mod codegen;
mod codegen_rs;
mod docslock;
mod env;
mod lexer;
mod moves;
mod parser;
mod project;
mod review;
mod solver;
mod toml;

use ast::Diag;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "użycie: sowa <polecenie> [KATALOG]

polecenia:
  check                 sprawdza projekt: typy, sygnatury, uprawnienia, dokumentację
  build                 sprawdza i kompiluje program do .sowa/app_rs (z --bun: .sowa/app.js)
  run [--fake ZASÓB]    buduje i uruchamia program; --fake podmienia zasób na ten z [resources.test]
  test                  uruchamia przykłady, property i bloki sowa z docs/
  review --base REF     zmiany w specyfikacji względem REF (gałęzi albo commitu), od najbardziej
                        ryzykownej: uprawnienie, osłabienie, usunięty test, rozszerzenie, zwykłe
  review --confirm-docs[=OPIS]
                        zapisuje w docs.lock, że opisy (wszystkie oczekujące albo OPIS, np.
                        uzytkownik/faktury.md#rabat) są przejrzane przy bieżących sygnaturach

check --ci sprawdza też, czy specyfikacja nie zmieniła się od zatwierdzenia PR: zatwierdzenie
  czyta z API GitHuba przez gh (GITHUB_REPOSITORY, GITHUB_REF, GH_TOKEN). --approved-at COMMIT
  podaje zatwierdzony commit ręcznie i porównuje go z katalogiem roboczym.

Program i testy kompilują się do Rusta (rustc).
--bun kompiluje do JS dla Buna: testy ruszają od razu, bez rustc, ale program jest wolniejszy.
  To backend testowy, na razie bez nowych funkcji.
--panic-abort przy panice kończy program od razu, bez sprzątania. Program jest ok. 2% szybszy
  i ok. 10% mniejszy. Przy serwerze panika w jednym żądaniu zatrzymałaby cały serwer.

KATALOG to katalog projektu albo dowolny katalog pod nim (domyślnie bieżący).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        eprintln!("{}", USAGE);
        return ExitCode::from(2);
    };
    let mut fakes = vec![];
    let mut rust = true;
    let mut panic_abort = false;
    let mut dir = None;
    let mut o = Opts::default();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--base" | "--approved-at" if i + 1 < args.len() => {
                let v = Some(args[i + 1].clone());
                if args[i] == "--base" {
                    o.base = v;
                } else {
                    o.approved_at = v;
                }
                i += 2;
                continue;
            }
            a if a.starts_with("--base=") => o.base = Some(a["--base=".len()..].to_string()),
            a if a.starts_with("--approved-at=") => o.approved_at = Some(a["--approved-at=".len()..].to_string()),
            "--confirm-docs" => o.confirm = Some(o.confirm.take().unwrap_or_default()),
            a if a.starts_with("--confirm-docs=") => {
                o.confirm.get_or_insert_with(Vec::new).push(a["--confirm-docs=".len()..].to_string())
            }
            "--ci" => o.ci = true,
            "--fake" if i + 1 < args.len() => {
                fakes.push(args[i + 1].clone());
                i += 2;
                continue;
            }
            a if a.starts_with("--fake=") => fakes.push(a["--fake=".len()..].to_string()),
            // --rust to dawna nazwa domyślnego backendu.
            "--rust" => rust = true,
            "--bun" => rust = false,
            "--panic-abort" => panic_abort = true,
            a if a.starts_with('-') => {
                eprintln!("nieznana opcja {}\n\n{}", a, USAGE);
                return ExitCode::from(2);
            }
            a => dir = Some(PathBuf::from(a)),
        }
        i += 1;
    }
    let dir = dir.unwrap_or_else(|| PathBuf::from("."));
    match cmd.as_str() {
        "check" | "build" | "run" | "test" | "review" => {}
        "help" | "--help" | "-h" => {
            println!("{}", USAGE);
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("nieznane polecenie {}\n\n{}", cmd, USAGE);
            return ExitCode::from(2);
        }
    }
    if !fakes.is_empty() && cmd != "run" {
        eprintln!("--fake działa tylko z sowa run");
        return ExitCode::from(2);
    }
    if (o.ci || o.approved_at.is_some()) && cmd != "check" {
        eprintln!("--ci i --approved-at działają tylko z sowa check");
        return ExitCode::from(2);
    }
    if o.approved_at.is_some() {
        o.ci = true;
    }
    if (o.base.is_some() || o.confirm.is_some()) && cmd != "review" {
        eprintln!("--base i --confirm-docs działają tylko z sowa review");
        return ExitCode::from(2);
    }
    if cmd == "review" && o.base.is_none() == o.confirm.is_none() {
        eprintln!("sowa review potrzebuje --base REF albo --confirm-docs\n\n{}", USAGE);
        return ExitCode::from(2);
    }
    if panic_abort && !rust {
        eprintln!("--panic-abort nie działa z --bun");
        return ExitCode::from(2);
    }
    match run(&cmd, &dir, &fakes, rust, panic_abort, &o) {
        Ok(code) => code,
        Err(msg) => {
            eprintln!("{}", msg);
            ExitCode::FAILURE
        }
    }
}

fn print_diags(kind: &str, ds: &[Diag]) {
    for d in ds {
        eprintln!("{}: {}", kind, d);
    }
}

#[derive(Default)]
struct Opts {
    base: Option<String>,
    // Some(puste) = wszystkie oczekujące opisy.
    confirm: Option<Vec<String>>,
    ci: bool,
    approved_at: Option<String>,
}

fn run(cmd: &str, dir: &Path, fakes: &[String], rust: bool, panic_abort: bool, o: &Opts) -> Result<ExitCode, String> {
    let root = project::find_root(dir).ok_or_else(|| format!("nie ma sowa.toml w {} ani wyżej", dir.display()))?;
    let proj = match project::load(&root) {
        Ok(p) => p,
        Err(ds) => {
            print_diags("błąd", &ds);
            return Ok(ExitCode::FAILURE);
        }
    };
    let env = env::Env::build(&proj.files);
    if cmd == "review" {
        return review(&proj, &env, o);
    }
    let mut errors: Vec<Diag> = env.diags.iter().map(|d| Diag::new(&d.file, d.line, d.msg.clone())).collect();
    let mut g = codegen::Gen::new(&env);
    let program = g.program();
    let test_res = check::resource_names(&proj, "resources.test");
    let tests = if cmd == "test" || cmd == "check" {
        g.tests(&proj, &test_res)
    } else {
        String::new()
    };
    errors.append(&mut g.diags);
    let (mut cerr, mut warns) = check::check(&proj, &env);
    warns.extend(docslock::warnings(&proj, &env));
    errors.append(&mut cerr);
    errors.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    errors.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.msg == b.msg);
    print_diags("uwaga", &warns);
    print_diags("błąd", &errors);
    if !errors.is_empty() {
        eprintln!("{}: {}", proj.name, plural(errors.len(), "błąd", "błędy", "błędów"));
        return Ok(ExitCode::FAILURE);
    }
    if cmd == "check" && o.ci {
        let msg = ci::verify(&proj, o.approved_at.as_deref())?;
        println!("{}", msg);
    }
    if cmd == "check" {
        let nfiles = proj.files.len();
        println!(
            "{}: w porządku ({} .sowa, {} w docs/)",
            proj.name,
            plural(nfiles, "plik", "pliki", "plików"),
            plural(proj.docs.len(), "plik", "pliki", "plików")
        );
        return Ok(ExitCode::SUCCESS);
    }

    let out_dir = root.join(".sowa");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {}", out_dir.display(), e))?;
    if rust {
        let (name, program) = if cmd == "test" {
            ("test_rs", codegen_rs::Gen::new(&env).tests_program(&proj, &test_res))
        } else {
            check_fakes(&proj, fakes)?;
            ("app_rs", codegen_rs::Gen::new(&env).main_program(&proj, fakes))
        };
        let src = format!("{}\n// ---- program ----\n{}", include_str!("runtime.rs"), program);
        let exe = compile_rust(&out_dir, name, &src, panic_abort)?;
        if cmd == "build" {
            println!("{}: zapisano {}", proj.name, exe.display());
            return Ok(ExitCode::SUCCESS);
        }
        let status = Command::new(&exe)
            .current_dir(&root)
            .status()
            .map_err(|e| format!("{}: {}", exe.display(), e))?;
        return Ok(ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8));
    }
    let root_js = codegen::js_str(&proj.root.to_string_lossy());
    let (file, js) = if cmd == "test" {
        let spec = resources_json(&proj, "resources.test", &[])?;
        (
            out_dir.join("test.js"),
            format!("{}\n{}\nawait $run_tests($TESTS, {}, false);\n", program, tests, spec),
        )
    } else {
        let names = check::resource_names(&proj, "resources");
        check_fakes(&proj, fakes)?;
        let spec = resources_json(&proj, "resources", fakes)?;
        let names_js: Vec<String> = names.iter().map(|n| codegen::js_str(n)).collect();
        let entry = format!("await $run_main(f_main, [{}], {}, {});\n", names_js.join(", "), spec, root_js);
        (out_dir.join("app.js"), format!("{}\n{}", program, entry))
    };
    std::fs::write(&file, js).map_err(|e| format!("{}: {}", file.display(), e))?;
    if cmd == "build" {
        println!("{}: zapisano {}", proj.name, file.display());
        return Ok(ExitCode::SUCCESS);
    }
    let bun = find_bun().ok_or("nie znaleziono Buna: zainstaluj go (https://bun.sh) albo podaj ścieżkę w SOWA_BUN")?;
    let status = Command::new(&bun)
        .arg(&file)
        .current_dir(&root)
        .status()
        .map_err(|e| format!("{}: {}", bun.display(), e))?;
    Ok(if status.success() { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}

fn review(proj: &project::Project, env: &env::Env, o: &Opts) -> Result<ExitCode, String> {
    if let Some(base) = &o.base {
        print!("{}", review::review(proj, env, base)?);
        return Ok(ExitCode::SUCCESS);
    }
    let only = o.confirm.clone().unwrap_or_default();
    let who = Command::new("git")
        .current_dir(&proj.root)
        .args(["config", "user.email"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or("--confirm-docs: ustaw git config user.email, docs.lock zapisuje, kto przejrzał opis")?;
    let done = docslock::confirm(proj, env, &only, &who, &today())?;
    if done.is_empty() {
        println!("{}: nic do potwierdzenia", docslock::FILE);
    } else {
        println!("{}: przejrzane przez {}:", docslock::FILE, who);
        for t in done {
            println!("  {}/{}", proj.docs_dir, t);
        }
    }
    Ok(ExitCode::SUCCESS)
}

// Dzisiejsza data UTC jako RRRR-MM-DD (algorytm dni cywilnych Howarda Hinnanta).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn check_fakes(proj: &project::Project, fakes: &[String]) -> Result<(), String> {
    let names = check::resource_names(proj, "resources");
    match fakes.iter().find(|f| !names.contains(f)) {
        Some(f) => Err(format!("--fake {}: nie ma takiego zasobu w [resources]", f)),
        None => Ok(()),
    }
}

// Kompiluje źródło Rusta do .sowa/<name>, tylko gdy kod albo flagi się zmieniły. Jedna jednostka
// kodu (codegen-units=1) daje LLVM cały program naraz. panic=abort usuwa kod rozwijania stosu,
// bo błędy Sowy idą przez Result, ale panika kończy wtedy cały proces, a nie tylko jeden wątek.
fn compile_rust(out_dir: &Path, name: &str, src: &str, panic_abort: bool) -> Result<PathBuf, String> {
    let file = out_dir.join(format!("{}.rs", name));
    let exe = out_dir.join(name);
    let mut flags = vec!["--edition=2021", "-O", "-C", "codegen-units=1"];
    if panic_abort {
        flags.extend(["-C", "panic=abort"]);
    }
    let src = format!("// rustc {}\n{}", flags.join(" "), src);
    let fresh = exe.is_file() && std::fs::read_to_string(&file).is_ok_and(|old| old == src);
    if !fresh {
        std::fs::write(&file, &src).map_err(|e| format!("{}: {}", file.display(), e))?;
        let rustc = find_tool("SOWA_RUSTC", "rustc", ".cargo/bin/rustc")
            .ok_or("nie znaleziono rustc: zainstaluj Rusta (https://rustup.rs) albo podaj ścieżkę w SOWA_RUSTC")?;
        let status = Command::new(&rustc)
            .args(&flags)
            .args(["--crate-name", &format!("sowa_{}", name), "-o"])
            .arg(&exe)
            .arg(&file)
            .status()
            .map_err(|e| format!("{}: {}", rustc.display(), e))?;
        if !status.success() {
            let _ = std::fs::remove_file(&exe);
            return Err(format!("rustc nie skompilował {}", file.display()));
        }
    }
    Ok(exe)
}

pub fn plural(n: usize, one: &str, few: &str, many: &str) -> String {
    let w = if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    };
    format!("{} {}", n, w)
}

fn find_bun() -> Option<PathBuf> {
    find_tool("SOWA_BUN", "bun", ".bun/bin/bun")
}

// Program ze zmiennej środowiska, potem z PATH, potem z katalogu domowego.
fn find_tool(var: &str, name: &str, in_home: &str) -> Option<PathBuf> {
    if let Ok(p) = std::env::var(var) {
        return Some(PathBuf::from(p));
    }
    if let Ok(path) = std::env::var("PATH") {
        for d in std::env::split_paths(&path) {
            let p = d.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let home = std::env::var("HOME").ok()?;
    let p = Path::new(&home).join(in_home);
    p.is_file().then_some(p)
}

fn value_json(v: &toml::Value) -> String {
    match v {
        toml::Value::Str(s) => codegen::js_str(s),
        toml::Value::Int(n) => n.to_string(),
        toml::Value::Bool(b) => b.to_string(),
        toml::Value::Table(kv) => {
            let items: Vec<String> = kv.iter().map(|(k, v)| format!("{}: {}", codegen::js_str(k), value_json(v))).collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

// Zasoby z sekcji jako obiekt JS; zasoby z `fakes` biorą opis z [resources.test].
fn resources_json(proj: &project::Project, section: &str, fakes: &[String]) -> Result<String, String> {
    let entries = proj.toml.get(section).cloned().unwrap_or_default();
    let test = proj.toml.get("resources.test").cloned().unwrap_or_default();
    let mut items = vec![];
    for e in &entries {
        let v = if fakes.contains(&e.key) {
            &test
                .iter()
                .find(|t| t.key == e.key)
                .ok_or_else(|| format!("--fake {}: nie ma go w [resources.test]", e.key))?
                .value
        } else {
            &e.value
        };
        items.push(format!("{}: {}", codegen::js_str(&e.key), value_json(v)));
    }
    Ok(format!("{{{}}}", items.join(", ")))
}
