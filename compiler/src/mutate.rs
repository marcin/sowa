// Testy mutacyjne: sowa test --mutate i sowa review --base REF --mutate.
//
// Generator Rusta wstawia w ciała funkcji z impl/ drobne zmiany (mutanty): + na -, < na <=,
// == na !=, && na ||, liczbę n na n+1, true na false, !x na x. Program testów kompiluje się raz,
// a mutant K włącza się przy uruchomieniu przez SOWA_MUTANT=K. Mutant jest wykryty, gdy któryś
// test nie przechodzi albo program przekracza czas. Mutant, który przeżył, wskazuje kod, którego
// zmiany żaden przykład ani property nie zauważa.

use crate::codegen_rs::{Gen, Mutant};
use crate::env::Env;
use crate::project::Project;
use std::collections::HashSet;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

pub struct Outcome {
    pub total: usize,
    pub survived: Vec<Mutant>,
    // Wyjście testów bez mutacji.
    pub baseline: String,
}

// Mutanty w funkcjach z `only` (pusty zbiór = wszystkie poza main i atrapami).
pub fn run(p: &Project, env: &Env, only: HashSet<String>) -> Result<Outcome, String> {
    let test_res = crate::check::resource_names(p, "resources.test");
    let mut g = Gen::new(env);
    g.mutate = Some(only);
    let program = g.tests_program(p, &test_res);
    let mutants = std::mem::take(&mut g.mutants);
    let out_dir = p.root.join(".sowa");
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {}", out_dir.display(), e))?;
    let src = format!("{}\n// ---- program ----\n{}", include_str!("runtime.rs"), program);
    let exe = crate::compile_rust(&out_dir, "mutants_rs", &src, false)?;
    // Bez SOWA_MUTANT to zwykłe testy: muszą przechodzić, inaczej wynik mutacji nic nie znaczy.
    let t0 = Instant::now();
    let base = Command::new(&exe)
        .current_dir(&p.root)
        .env_remove("SOWA_MUTANT")
        .output()
        .map_err(|e| format!("{}: {}", exe.display(), e))?;
    let baseline = String::from_utf8_lossy(&base.stdout).into_owned();
    if !base.status.success() {
        return Err(format!("{}testy mutacyjne: testy nie przechodzą bez mutacji", baseline));
    }
    let limit = (t0.elapsed() * 10).max(Duration::from_secs(2));
    let n = mutants.len();
    let next = AtomicUsize::new(0);
    let killed: Mutex<Vec<Option<bool>>> = Mutex::new(vec![None; n]);
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(4).min(n);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= n {
                        break;
                    }
                    let r = run_one(&exe, &p.root, k, limit);
                    killed.lock().unwrap()[k] = r;
                }
            });
        }
    });
    let killed = killed.into_inner().unwrap();
    if killed.iter().any(|k| k.is_none()) {
        return Err(format!("testy mutacyjne: nie da się uruchomić {}", exe.display()));
    }
    let mut survived: Vec<Mutant> = mutants.into_iter().zip(killed).filter(|(_, k)| *k == Some(false)).map(|(m, _)| m).collect();
    survived.sort_by(|a, b| (&a.file, a.line).cmp(&(&b.file, b.line)));
    Ok(Outcome { total: n, survived, baseline })
}

// Czy mutant k jest wykryty: błąd testu, panika albo przekroczony czas.
fn run_one(exe: &Path, root: &Path, k: usize, limit: Duration) -> Option<bool> {
    let mut ch = Command::new(exe)
        .current_dir(root)
        .env("SOWA_MUTANT", k.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let end = Instant::now() + limit;
    loop {
        match ch.try_wait() {
            Ok(Some(st)) => return Some(!st.success()),
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = ch.kill();
                let _ = ch.wait();
                return Some(true);
            }
        }
    }
}

// `mutacje wykryte przez testy: X z Y` i mutanty, które przeżyły, z miejscem w kodzie.
pub fn text(o: &Outcome, indent: &str) -> String {
    if o.total == 0 {
        return "mutacje: w zmienionym kodzie nie ma czego mutować\n".into();
    }
    let mut s = format!("mutacje wykryte przez testy: {} z {}\n", o.total - o.survived.len(), o.total);
    if o.survived.is_empty() {
        return s;
    }
    s.push_str(&format!("{}przeżyły, bo żaden test ich nie wykrywa:\n", indent));
    let locs: Vec<String> = o.survived.iter().map(|m| format!("{}:{}", m.file, m.line)).collect();
    let w = locs.iter().map(|l| l.chars().count()).max().unwrap_or(0) + 2;
    for (m, l) in o.survived.iter().zip(&locs) {
        s.push_str(&format!("{}  {:w$}{}: {}\n", indent, l, m.fname, m.desc, w = w));
    }
    s
}
