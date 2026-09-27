// Zatwierdzanie podpisanym commitem, przy pracy bez PR albo na jednym koncie ([review] sign = true).
//
// `sowa review --approve` robi pusty commit na bieżącej gałęzi, podpisany kluczem SSH z git config,
// z wierszem `Sowa-Approved: <katalog projektu>`. Commit zatwierdza drzewo plików, na którym
// stoi, a podpis obejmuje to drzewo i rodzica, więc przeniesiony albo przepisany commit traci
// ważność. `sowa check --ci` szuka najnowszego takiego commitu z ważnym podpisem osoby
// z [review] approvers i sprawdza, czy od niego zmieniła się specyfikacja.
//
// Klucze do sprawdzenia podpisu nie mogą pochodzić z repozytorium, bo agent mógłby dopisać
// swój. `@login` bierze klucze do podpisu z konta na GitHubie (zmiana wymaga uprawnienia
// admin:ssh_signing_key, którego zwykły token nie ma). Adres e-mail bierze klucz z pliku
// allowed_signers spoza repozytorium: SOWA_ALLOWED_SIGNERS albo git config gpg.ssh.allowedSignersFile.

use crate::ast::Diag;
use crate::ci::{changed, run, short, verdict};
use crate::project::Project;
use crate::toml::Value;
use std::path::Path;
use std::process::{Command, Stdio};

pub const TRAILER: &str = "Sowa-Approved";

pub struct Review {
    pub sign: bool,
    pub approvers: Vec<String>,
}

pub fn config(p: &Project) -> Review {
    let es = p.toml.get("review").cloned().unwrap_or_default();
    let get = |k: &str| es.iter().find(|e| e.key == k).map(|e| e.value.clone());
    let approvers = match get("approvers") {
        Some(Value::List(xs)) => xs
            .iter()
            .filter_map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None })
            .collect(),
        Some(Value::Str(s)) => vec![s],
        _ => vec![],
    };
    Review {
        sign: matches!(get("sign"), Some(Value::Bool(true))),
        approvers,
    }
}

// Błędy i uwagi do [review] dla `sowa check`.
pub fn diags(p: &Project) -> (Vec<Diag>, Vec<Diag>) {
    let (mut errs, mut warns) = (vec![], vec![]);
    let Some(es) = p.toml.get("review") else {
        return (errs, warns);
    };
    for e in es {
        let ok = match (e.key.as_str(), &e.value) {
            ("approvers", Value::List(xs)) => {
                if xs.iter().any(|x| !matches!(x, Value::Str(s) if valid_approver(s))) {
                    errs.push(Diag::new(
                        "sowa.toml",
                        e.line,
                        "[review] approvers: każdy wpis to \"@login\" z GitHuba albo adres e-mail",
                    ));
                }
                true
            }
            ("sign", Value::Bool(_)) => true,
            ("agent", Value::Str(_)) => {
                warns.push(Diag::new("sowa.toml", e.line, "[review] agent: drugi agent-recenzent jeszcze nie działa"));
                true
            }
            ("approvers" | "sign" | "agent", _) => {
                let want = match e.key.as_str() {
                    "approvers" => "lista tekstów",
                    "sign" => "true albo false",
                    _ => "tekst",
                };
                errs.push(Diag::new("sowa.toml", e.line, format!("[review] {}: oczekiwano: {}", e.key, want)));
                true
            }
            _ => false,
        };
        if !ok {
            errs.push(Diag::new("sowa.toml", e.line, format!("[review] {}: nieznany klucz (approvers, sign, agent)", e.key)));
        }
    }
    let cfg = config(p);
    if cfg.sign && cfg.approvers.is_empty() {
        let line = es.iter().find(|e| e.key == "sign").map(|e| e.line).unwrap_or(0);
        errs.push(Diag::new("sowa.toml", line, "[review] sign = true potrzebuje approvers: kto może zatwierdzać"));
    }
    (errs, warns)
}

fn valid_approver(s: &str) -> bool {
    let login = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    match s.strip_prefix('@') {
        Some(l) => login(l),
        None => s.contains('@') && !s.contains(char::is_whitespace),
    }
}

// Katalog projektu względem korzenia repozytorium: "examples/invoices" albo ".".
fn project_path(p: &Project) -> Result<String, String> {
    let pre = run(&p.root, "git", &["rev-parse", "--show-prefix"])?;
    let pre = pre.trim().trim_end_matches('/');
    Ok(if pre.is_empty() { ".".into() } else { pre.into() })
}

fn git_config(root: &Path, key: &str) -> Option<String> {
    run(root, "git", &["config", key]).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

pub fn approve(p: &Project) -> Result<String, String> {
    let cfg = config(p);
    if !cfg.sign {
        return Err("--approve: zatwierdzanie podpisem jest wyłączone; w sowa.toml potrzebne:\n  [review]\n  approvers = [\"@login\"]\n  sign      = true".into());
    }
    let dirty: Vec<String> = changed(p, "HEAD", None)?.into_iter().filter(|f| crate::ci::is_spec(p, f)).collect();
    if !dirty.is_empty() {
        return Err(format!(
            "--approve: zatwierdza się specyfikację z commitu, a te pliki mają zmiany poza nim:\n{}\n  najpierw commit albo cofnięcie zmian",
            dirty.iter().map(|f| format!("    {}", f)).collect::<Vec<_>>().join("\n")
        ));
    }
    if git_config(&p.root, "gpg.format").as_deref() != Some("ssh") || git_config(&p.root, "user.signingkey").is_none() {
        return Err("--approve: zatwierdzenie to commit podpisany kluczem SSH, a git nie ma go ustawionego:\n  \
             git config gpg.format ssh\n  \
             git config user.signingkey ~/.ssh/id_ed25519_sk.pub\n  \
             klucz musi wymagać potwierdzenia przy każdym użyciu (klucz sprzętowy albo ssh-add -c)"
            .into());
    }
    let path = project_path(p)?;
    let head = run(&p.root, "git", &["rev-parse", "HEAD"])?;
    let msg = format!("sowa: zatwierdzenie specyfikacji {}\n\n{}: {}\n", p.name, TRAILER, path);
    // commit-tree nie rusza indeksu: do commitu nie trafi nic, co ktoś dodał przez git add.
    // Podpis może pytać o potwierdzenie w terminalu, więc stdin i stderr idą do użytkownika.
    let out = Command::new("git")
        .current_dir(&p.root)
        .args(["commit-tree", "-S", "HEAD^{tree}", "-p", "HEAD", "-m", &msg])
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("git: {}", e))?;
    if !out.status.success() {
        return Err("--approve: git nie złożył podpisu, zatwierdzenia nie ma".into());
    }
    let new = String::from_utf8_lossy(&out.stdout).trim().to_string();
    run(&p.root, "git", &["update-ref", "-m", "sowa review --approve", "HEAD", &new, head.trim()])?;
    Ok(format!(
        "{}: zatwierdzono specyfikację w commicie {} (podpis z {})\n  po git push sowa check --ci przyjmie ten stan",
        p.name,
        short(&new),
        git_config(&p.root, "user.signingkey").unwrap_or_default()
    ))
}

// Plik allowed_signers dla git: wiersz „approver klucz” dla każdego znanego klucza.
fn signers(p: &Project, approvers: &[String]) -> Result<(String, Vec<String>), String> {
    let mut lines = vec![];
    let mut missing = vec![];
    let file = std::env::var("SOWA_ALLOWED_SIGNERS").ok().or_else(|| git_config(&p.root, "gpg.ssh.allowedSignersFile"));
    let local = file.as_ref().and_then(|f| {
        let f = f.strip_prefix("~/").map(|r| format!("{}/{}", std::env::var("HOME").unwrap_or_default(), r)).unwrap_or(f.clone());
        std::fs::read_to_string(f).ok()
    });
    for a in approvers {
        let before = lines.len();
        if let Some(login) = a.strip_prefix('@') {
            let keys = run(&p.root, "gh", &["api", &format!("users/{}/ssh_signing_keys", login), "--jq", ".[].key"])
                .map_err(|e| format!("--ci: nie da się pobrać kluczy do podpisu @{} z GitHuba\n  {}", login, e))?;
            for k in keys.lines().map(|k| k.trim()).filter(|k| !k.is_empty()) {
                lines.push(format!("{} namespaces=\"git\" {}", a, k));
            }
        } else if let Some(src) = &local {
            for l in src.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')) {
                let Some((who, rest)) = l.split_once(char::is_whitespace) else { continue };
                if who.split(',').any(|w| w == a) {
                    lines.push(format!("{} {}", a, rest.trim()));
                }
            }
        }
        if lines.len() == before {
            missing.push(a.clone());
        }
    }
    Ok((lines.join("\n") + "\n", missing))
}

// Wersja --ci dla [review] sign = true.
pub fn verify(p: &Project, approvers: &[String]) -> Result<String, String> {
    let path = project_path(p)?;
    let (keys, missing) = signers(p, approvers)?;
    if keys.trim().is_empty() {
        return Err(format!(
            "błąd: nie ma kluczy do sprawdzenia podpisu ({})\n      \
             @login: klucze do podpisu (Signing keys) na koncie GitHuba; e-mail: plik z SOWA_ALLOWED_SIGNERS",
            missing.join(", ")
        ));
    }
    let tmp = std::env::temp_dir().join(format!("sowa-signers-{}", std::process::id()));
    std::fs::write(&tmp, keys).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    let cfg = format!("gpg.ssh.allowedSignersFile={}", tmp.display());
    let res = find(p, &path, approvers, &cfg);
    let _ = std::fs::remove_file(&tmp);
    let (commit, who) = res?.ok_or_else(|| {
        format!(
            "błąd: specyfikacja {} nie ma ważnego zatwierdzenia (commit {} z podpisem osoby z [review] approvers)\n      \
             potrzebne: sowa review --approve i git push",
            p.name, TRAILER
        )
    })?;
    let files = changed(p, &commit, None)?;
    verdict(p, &format!("{}, podpis {}", short(&commit), who), files)
        .map_err(|e| e.replace("ponowne zatwierdzenie PR", "ponowne zatwierdzenie: sowa review --approve"))
}

// Najnowszy commit z Sowa-Approved dla tego projektu i ważnym podpisem osoby z listy.
fn find(p: &Project, path: &str, approvers: &[String], cfg: &str) -> Result<Option<(String, String)>, String> {
    let grep = format!("--grep={}: {}", TRAILER, path);
    let list = run(&p.root, "git", &["log", "--format=%H", "-F", &grep, "HEAD"])?;
    let trailer = format!("--format=%(trailers:key={},valueonly,separator=%x0A)", TRAILER);
    for c in list.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
        let t = run(&p.root, "git", &["log", "-1", &trailer, c])?;
        if !t.lines().any(|l| l.trim() == path) {
            continue;
        }
        // %G? = G: dobry podpis znanego klucza; %GS: osoba z allowed_signers.
        let Ok(sig) = run(&p.root, "git", &["-c", cfg, "log", "-1", "--format=%G?%x09%GS", c]) else { continue };
        let (st, who) = sig.trim().split_once('\t').unwrap_or((sig.trim(), ""));
        if st == "G" && approvers.iter().any(|a| a == who) {
            return Ok(Some((c.to_string(), who.to_string())));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approvers() {
        assert!(valid_approver("@anna"));
        assert!(valid_approver("@anna-kowalska"));
        assert!(valid_approver("reviewer@intum.com"));
        assert!(!valid_approver("@"));
        assert!(!valid_approver("@org/zespol"));
        assert!(!valid_approver("anna"));
        assert!(!valid_approver("x y@z"));
    }
}
