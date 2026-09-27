// sowa check | build [--rust] | run [--fake NAZWA] [--rust] | test [--rust]  [--panic-abort] [KATALOG]
//
// Kompilator czyta projekt (sowa.toml, src/, impl/, docs/), sprawdza go i tłumaczy na jeden
// plik JavaScript w <projekt>/.sowa/, który uruchamia Bun. Z --rust zamiast tego tłumaczy
// program albo testy na Rusta i kompiluje je rustc do pliku wykonywalnego.

mod ast;
mod check;
mod codegen;
mod codegen_rs;
mod env;
mod lexer;
mod moves;
mod parser;
mod project;
mod toml;

use ast::Diag;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const USAGE: &str = "użycie: sowa <polecenie> [KATALOG]

polecenia:
  check                 sprawdza projekt: typy, sygnatury, uprawnienia, dokumentację
  build [--rust]        sprawdza i zapisuje program w .sowa/app.js (z --rust: .sowa/app_rs)
  run [--fake ZASÓB]    buduje i uruchamia program; --fake podmienia zasób na ten z [resources.test]
  test [--rust]         uruchamia przykłady, property i bloki sowa z docs/

--rust kompiluje do Rusta (rustc) zamiast do JS dla Buna.
--panic-abort (z --rust) przy panice kończy program od razu, bez sprzątania. Program jest ok. 2%
  szybszy i ok. 10% mniejszy. Przy serwerze panika w jednym żądaniu zatrzymałaby cały serwer.

KATALOG to katalog projektu albo dowolny katalog pod nim (domyślnie bieżący).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        eprintln!("{}", USAGE);
        return ExitCode::from(2);
    };
    let mut fakes = vec![];
    let mut rust = false;
    let mut panic_abort = false;
    let mut dir = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--fake" if i + 1 < args.len() => {
                fakes.push(args[i + 1].clone());
                i += 2;
                continue;
            }
            a if a.starts_with("--fake=") => fakes.push(a["--fake=".len()..].to_string()),
            "--rust" => rust = true,
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
        "check" | "build" | "run" | "test" => {}
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
    if panic_abort && !rust {
        eprintln!("--panic-abort działa tylko z --rust");
        return ExitCode::from(2);
    }
    match run(&cmd, &dir, &fakes, rust, panic_abort) {
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

fn run(cmd: &str, dir: &Path, fakes: &[String], rust: bool, panic_abort: bool) -> Result<ExitCode, String> {
    let root = project::find_root(dir).ok_or_else(|| format!("nie ma sowa.toml w {} ani wyżej", dir.display()))?;
    let proj = match project::load(&root) {
        Ok(p) => p,
        Err(ds) => {
            print_diags("błąd", &ds);
            return Ok(ExitCode::FAILURE);
        }
    };
    let env = env::Env::build(&proj.files);
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
    let (mut cerr, warns) = check::check(&proj, &env);
    errors.append(&mut cerr);
    errors.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    errors.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.msg == b.msg);
    print_diags("uwaga", &warns);
    print_diags("błąd", &errors);
    if !errors.is_empty() {
        eprintln!("{}: {}", proj.name, plural(errors.len(), "błąd", "błędy", "błędów"));
        return Ok(ExitCode::FAILURE);
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

fn plural(n: usize, one: &str, few: &str, many: &str) -> String {
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
