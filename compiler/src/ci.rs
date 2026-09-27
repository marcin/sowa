// sowa check --ci: czy specyfikacja nie zmieniła się po zatwierdzeniu PR.
//
// Zatwierdzenie to ostatnia recenzja APPROVED od właściciela z CODEOWNERS, czytana z API GitHuba
// przez `gh`. Od zatwierdzonego commitu do bieżącej głowy PR mogą się zmieniać tylko pliki bez
// właściciela, czyli zwykle impl/. Zmiana w src/, docs/, sowa.toml, docs.lock albo w pliku
// z właścicielem wymaga ponownego zatwierdzenia. Z --approved-at COMMIT zatwierdzony commit
// podaje się ręcznie, a porównanie idzie z katalogiem roboczym, bez GitHuba.

use crate::check::glob_match;
use crate::project::Project;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn run(root: &Path, prog: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(prog)
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {}", prog, e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("{} {}: {}", prog, args.join(" "), err));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

// Pliki projektu, których zmiana wymaga zgody właściciela.
fn is_spec(p: &Project, path: &str) -> bool {
    let under = |d: &str| path.starts_with(&format!("{}/", d.trim_end_matches('/')));
    path == "sowa.toml"
        || path == crate::docslock::FILE
        || under(&p.src_dir)
        || under(&p.docs_dir)
        || ["CODEOWNERS", ".github/CODEOWNERS", "docs/CODEOWNERS"].contains(&path)
        || p.codeowners.iter().any(|(pat, _)| glob_match(pat, path))
}

// Zmienione pliki projektu (ścieżki od katalogu projektu) między `from` a `to`,
// a bez `to` między `from` a katalogiem roboczym, razem z nowymi plikami spoza gita.
fn changed(p: &Project, from: &str, to: Option<&str>) -> Result<Vec<String>, String> {
    let root = &p.root;
    for c in [Some(from), to].into_iter().flatten() {
        if run(root, "git", &["cat-file", "-e", &format!("{}^{{commit}}", c)]).is_err() {
            return Err(format!(
                "błąd: nie ma commitu {} w repozytorium (za płytki checkout albo force-push)\n      \
                 potrzebne fetch-depth: 0 albo ponowne zatwierdzenie PR",
                short(c)
            ));
        }
    }
    let mut args = vec!["diff", "--name-only", "--relative", from];
    if let Some(t) = to {
        args.push(t);
    }
    let mut out: Vec<String> = run(root, "git", &args)?.lines().map(|l| l.to_string()).collect();
    if to.is_none() {
        let extra = run(root, "git", &["ls-files", "--others", "--exclude-standard"])?;
        out.extend(extra.lines().map(|l| l.to_string()));
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn short(c: &str) -> &str {
    if c.len() > 7 && c.chars().all(|x| x.is_ascii_hexdigit()) {
        &c[..7]
    } else {
        c
    }
}

fn verdict(p: &Project, approved: &str, files: Vec<String>) -> Result<String, String> {
    let spec: Vec<String> = files.into_iter().filter(|f| is_spec(p, f)).collect();
    if spec.is_empty() {
        return Ok(format!(
            "{}: specyfikacja bez zmian od zatwierdzenia ({})",
            p.name,
            short(approved)
        ));
    }
    let list: Vec<String> = spec.iter().map(|f| format!("        {}", f)).collect();
    Err(format!(
        "błąd: specyfikacja zmieniła się po zatwierdzeniu (zatwierdzone na {})\n{}\n      potrzebne ponowne zatwierdzenie PR",
        short(approved),
        list.join("\n")
    ))
}

pub fn verify(p: &Project, approved_at: Option<&str>) -> Result<String, String> {
    if let Some(c) = approved_at {
        let files = changed(p, c, None)?;
        let sha = run(&p.root, "git", &["rev-parse", "--short", c]).unwrap_or_else(|_| c.to_string());
        return verdict(p, &sha, files);
    }
    let repo = std::env::var("GITHUB_REPOSITORY")
        .map_err(|_| "--ci działa w GitHub Actions (GITHUB_REPOSITORY); poza nim podaj --approved-at COMMIT".to_string())?;
    let Some(pr) = pr_number() else {
        return Ok(format!("{}: to nie jest PR, --ci nie ma czego sprawdzać", p.name));
    };
    let root = &p.root;
    let reviews = run(
        root,
        "gh",
        &[
            "api",
            "--paginate",
            &format!("repos/{}/pulls/{}/reviews", repo, pr),
            "--jq",
            ".[] | [.user.login, .state, .commit_id] | @tsv",
        ],
    )?;
    let owners = owners(p, &repo)?;
    let approved = latest_approval(&reviews, &owners);
    let Some(approved) = approved else {
        return Ok(format!(
            "{}: PR #{} nie ma jeszcze zatwierdzenia właściciela z CODEOWNERS, sprawdzenie ruszy po nim",
            p.name, pr
        ));
    };
    let head = run(root, "gh", &["api", &format!("repos/{}/pulls/{}", repo, pr), "--jq", ".head.sha"])?;
    let files = changed(p, &approved, Some(head.trim()))?;
    verdict(p, &approved, files)
}

// Commit z ostatniego zatwierdzenia właściciela. Wiersze: login, stan, commit (chronologicznie).
// Liczy się ostatni rozstrzygający stan każdej osoby; komentarz bez decyzji go nie zmienia,
// a późniejsze CHANGES_REQUESTED albo DISMISSED unieważnia jej zatwierdzenie.
fn latest_approval(reviews: &str, owners: &[String]) -> Option<String> {
    let mut last: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    for (i, l) in reviews.lines().enumerate() {
        let c: Vec<&str> = l.split('\t').collect();
        if c.len() < 3 || !matches!(c[1], "APPROVED" | "CHANGES_REQUESTED" | "DISMISSED") {
            continue;
        }
        last.insert(c[0].to_lowercase(), (i, c[1].to_string(), c[2].to_string()));
    }
    last.iter()
        .filter(|(who, (_, st, _))| st == "APPROVED" && owners.contains(*who))
        .max_by_key(|(_, (i, _, _))| *i)
        .map(|(_, (_, _, c))| c.clone())
}

// Numer PR z GITHUB_REF (refs/pull/N/merge), a w razie potrzeby z pliku zdarzenia.
fn pr_number() -> Option<String> {
    if let Ok(r) = std::env::var("GITHUB_REF") {
        if let Some(n) = r.strip_prefix("refs/pull/").and_then(|x| x.split('/').next()) {
            return Some(n.to_string());
        }
    }
    let ev = std::fs::read_to_string(std::env::var("GITHUB_EVENT_PATH").ok()?).ok()?;
    let at = ev.find("\"pull_request\"")?;
    let rest = &ev[at..];
    let n = rest.find("\"number\"")?;
    let digits: String = rest[n + 8..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    (!digits.is_empty()).then_some(digits)
}

// Loginy właścicieli z CODEOWNERS, małymi literami; zespoły @org/zespół rozwija API.
fn owners(p: &Project, repo: &str) -> Result<Vec<String>, String> {
    let mut out = vec![];
    let mut all: Vec<&String> = p.codeowners.iter().flat_map(|(_, os)| os.iter()).collect();
    all.sort();
    all.dedup();
    for o in all {
        let Some(o) = o.strip_prefix('@') else { continue };
        match o.split_once('/') {
            Some((org, team)) => {
                let members = run(
                    &p.root,
                    "gh",
                    &["api", "--paginate", &format!("orgs/{}/teams/{}/members", org, team), "--jq", ".[].login"],
                )
                .map_err(|e| {
                    format!(
                        "--ci: nie da się odczytać członków zespołu @{} ({}): token potrzebuje read:org\n  {}",
                        o, repo, e
                    )
                })?;
                out.extend(members.lines().map(|l| l.trim().to_lowercase()));
            }
            None => out.push(o.to_lowercase()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval() {
        let owners = vec!["anna".to_string(), "piotr".to_string()];
        let r = "agent\tAPPROVED\taaa\nAnna\tAPPROVED\tbbb\npiotr\tCOMMENTED\tccc\n";
        assert_eq!(latest_approval(r, &owners), Some("bbb".into()));
        let r = "anna\tAPPROVED\tbbb\nanna\tCHANGES_REQUESTED\tccc\n";
        assert_eq!(latest_approval(r, &owners), None);
        let r = "anna\tAPPROVED\tbbb\npiotr\tAPPROVED\tddd\n";
        assert_eq!(latest_approval(r, &owners), Some("ddd".into()));
        assert_eq!(latest_approval("agent\tAPPROVED\taaa\n", &owners), None);
    }
}
