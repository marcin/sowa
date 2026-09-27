# Kompilator Sowy (prototyp)

Najprostszy kompilator, który wystarcza, żeby sprawdzić, przetestować i uruchomić [examples/fakturownia_web](../examples/fakturownia_web/). Napisany w Ruście bez zależności. Tłumaczy cały projekt na Rusta i kompiluje go rustc do jednego pliku wykonywalnego. Program też nie ma zależności z crates.io: bazę daje systemowy `libsqlite3`, a zapytania HTTP systemowy libcurl.

Drugi backend tłumaczy projekt na jeden plik JavaScript, który uruchamia [Bun](https://bun.sh) (`--bun`, baza z `bun:sqlite`). Jest tylko do testów i porównań: działa i przechodzi te same testy, ale nowe funkcje trafiają najpierw do Rusta.

`sowa review` porównuje specyfikację z inną wersją w gicie i sortuje zmiany od najbardziej ryzykownej, a mały solver rozstrzyga, czy zmieniony warunek jest luźniejszy, i podaje kontrprzykład. `sowa check --ci` pilnuje, żeby po zatwierdzeniu PR albo podpisanym zatwierdzeniu (`sowa review --approve`) specyfikacja się nie zmieniła. Warunków w programie kompilator nie dowodzi, tylko sprawdza je w runtime. Testów mutacyjnych nie ma.

## Użycie

```
cargo build --manifest-path compiler/Cargo.toml
sowa check [KATALOG]               # sprawdza projekt
sowa test [KATALOG]                # przykłady, property (po 100 przypadków) i bloki sowa z docs/
sowa build [KATALOG]               # kompiluje program do KATALOG/.sowa/app_rs
sowa run [--fake ZASÓB] [KATALOG]  # buduje i uruchamia
sowa review --base REF [KATALOG]   # zmiany w znaczeniu specyfikacji względem REF (gałąź, tag, commit)
sowa review --confirm-docs[=OPIS]  # zapisuje w docs.lock przejrzane opisy (wszystkie albo jeden)
sowa review --approve [KATALOG]    # zatwierdza specyfikację z HEAD commitem podpisanym kluczem SSH
sowa check --ci [KATALOG]          # czy specyfikacja nie zmieniła się po zatwierdzeniu (PR albo podpis)
sowa check --approved-at REF       # to samo bez GitHuba: REF to zatwierdzony commit
sowa test --bun / build --bun / run --bun   # to samo przez Buna: KATALOG/.sowa/app.js
```

`KATALOG` to katalog z `sowa.toml` albo dowolny katalog pod nim. `--fake ksef` bierze opis zasobu `ksef` z `[resources.test]`, czyli atrapę. `rustc` jest szukany w `SOWA_RUSTC`, potem w `PATH`, potem w `~/.cargo/bin/rustc`, a Bun w `SOWA_BUN`, potem w `PATH`, potem w `~/.bun/bin/bun`. `--rust` to dawna nazwa domyślnego backendu i nadal działa.

`sowa test` tłumaczy program i testy na jeden plik `.sowa/test_rs.rs`, kompiluje go `rustc -O -C codegen-units=1` do `.sowa/test_rs` i uruchamia. Kompilacja trwa 3–6 s i jest pomijana, gdy kod się nie zmienił. `sowa test --bun` daje wynik w ok. 0,1 s. Wynik i komunikaty obu backendów są takie same, łącznie z wylosowanymi przypadkami property. `sowa build` i `sowa run` kompilują tak samo `main` do `.sowa/app_rs`. Działają w nim wszystkie zasoby: `Db`, `Clock`, `Random`, `Terminal`, `Server`, `Http` i `Mailer`.

**Server** to HTTP/1.1 na `std::net`, bez crate'ów. Jedno żądanie idzie na połączenie (`Connection: close`). Nie ma TLS, HTTP/2 ani `Transfer-Encoding: chunked` w żądaniu (to ostatnie dostaje 411). Nagłówki mają limit 16 KB, a treść 1 MB (413). Odczyt i zapis mają po 5 s. Każde połączenie czyta i pisze własny wątek, więc wolny klient nie wstrzymuje innych. Program woła tylko wątek główny, po kolei, jak w Bunie. HTTPS daje reverse proxy przed aplikacją (Caddy, nginx). **Http** wysyła zapytania przez systemowy libcurl (`libcurl.4.dylib` na macOS, `libcurl.so.4` na Linuksie). Runtime ładuje go przez `dlopen` przy pierwszym zapytaniu, więc program bez `Http` go nie potrzebuje. Bez libcurl zapytanie kończy się `HttpError` i wpisem w logu. **Mailer** wysyła e-maile przez ten sam libcurl (SMTP). W Bunie działa tylko `server = "memory"`.

`--panic-abort` (nie działa z `--bun`) dodaje `-C panic=abort`. Program jest wtedy ok. 2% szybszy, a plik wykonywalny ok. 10% mniejszy, bo znika kod rozwijania stosu. Błędy Sowy idą przez `Result` i działają tak samo. Inaczej zachowuje się tylko panika, czyli błąd w kompilatorze albo w runtime: proces kończy się od razu z kodem 134 zamiast 101 i bez sprzątania. Opcja nie jest domyślna, bo z nią panika w obsłudze jednego żądania zatrzymałaby cały serwer. Bez niej serwer łapie panikę i odpowiada 500 tylko temu żądaniu.

## Przegląd i zatwierdzanie

`sowa review --base main` wypakowuje projekt z `main` (`git archive`) do katalogu tymczasowego, ładuje obie wersje i porównuje je symbol po symbolu: zasoby i `[review]` w `sowa.toml`, typy, sygnatury, `example`, `property`, `desc`/`doc`/`why`, nagłówki plików i bloki `sowa` w `docs/`. Każda zmiana dostaje kategorię (uprawnienie, osłabienie, usunięty test, rozszerzenie, zwykłe), a lista jest posortowana od najbardziej ryzykownej. Pod listą są: funkcje, które mają każde uprawnienie („Mailer (mail) mają tylko: …”), liczba zmienionych linii w `impl/` z plikami z CODEOWNERS do przeczytania i opisy z `docs.lock` do przejrzenia. Gdy w `REF` nie ma projektu, wszystko jest nowe, a zwykłe zmiany są zliczone w jednej linii.

**Solver** (`src/solver.rs`) sprawdza, czy stary warunek wynika z nowego. Warunek zamienia na alternatywę koniunkcji porównań liniowych na liczbach całkowitych (`α.x + 2 <= α.y`, `len(α) == 10`; `Money` w groszach), testów wariantu (`α.status == Paid`) i atomów, których nie rozumie (wywołania funkcji, `matches`). Koniunkcję `nowy && !stary` rozwiązuje eliminacją Fouriera–Motzkina na liczbach całkowitych. Rozwiązanie to kontrprzykład („dopuszcza np. 101”, „dopuszcza np. α.status = Cancelled”). Atom, którego nie rozumie, jest porównywany tylko tekstowo; gdy od niego zależy wynik, zmiana dostaje „nie udało się porównać” i trafia do osłabień.

**`docs.lock`** (`src/docslock.rs`) ma wiersz na każde powiązanie opisu z symbolem: sekcję `.md` albo cały plik, symbol, sposób powiązania (`doc`, `why` albo `{}`), 6 znaków hasha, kto i kiedy przejrzał. `sowa check` zgłasza uwagę przy każdym opisie, którego hash się zmienił albo którego jeszcze nie ma w pliku. `sowa review --confirm-docs` zapisuje bieżące hashe z adresem z `git config user.email`. Plik jest pod CODEOWNERS, więc potwierdzenie agenta i tak czeka na zgodę właściciela.

**`sowa check --ci`** (`src/ci.rs`) czyta przez `gh api` recenzje PR, bierze ostatnie zatwierdzenie osoby z CODEOWNERS (zespół `@org/nazwa` rozwija przez API, co wymaga `read:org`) i porównuje zatwierdzony commit z głową PR. Zmiana w `src/`, `docs/`, `sowa.toml`, `docs.lock`, CODEOWNERS albo w pliku z właścicielem to błąd. Zmiana w innych plikach `impl/` przechodzi. Bez zatwierdzenia check przechodzi z uwagą, bo wymóg zatwierdzenia egzekwuje ochrona gałęzi.

**Zatwierdzenie podpisem** (`src/sign.rs`) jest dla pracy bez PR albo na jednym koncie, gdzie recenzji PR nie ma albo agent mógłby ją złożyć sam. Włącza je `[review]` w `sowa.toml`:

```toml
[review]
approvers = ["@marcin", "reviewer@intum.com"]
sign      = true
```

`sowa review --approve` robi na bieżącej gałęzi pusty commit podpisany kluczem SSH z `git config user.signingkey` (`gpg.format = ssh`), z wierszem `Sowa-Approved: <katalog projektu>`. Commit powstaje przez `git commit-tree`, więc nie zabiera niczego z indeksu. Gdy pliki specyfikacji mają zmiany poza commitem, `--approve` odmawia. `sowa check --ci` szuka najnowszego commitu z `Sowa-Approved` dla tego projektu, z ważnym podpisem osoby z `approvers`, i porównuje go z katalogiem roboczym, jak `--approved-at`. Commit podpisany innym kluczem albo dla innego projektu się nie liczy. Klucze nie pochodzą z repozytorium, bo agent mógłby dopisać swój: `@login` bierze klucze do podpisu (Signing keys) z konta na GitHubie, a adres e-mail wiersze z pliku `allowed_signers` z `SOWA_ALLOWED_SIGNERS` albo `git config gpg.ssh.allowedSignersFile`. Klucz powinien wymagać potwierdzenia przy każdym użyciu (klucz sprzętowy albo `ssh-add -c`), bo agent na tym samym komputerze może go użyć.

Workflow tego repozytorium to [.github/workflows/sowa.yml](../.github/workflows/sowa.yml). Buduje kompilator, uruchamia `cargo test`, `sowa check` i `sowa test` na każdym `examples/*/sowa.toml`, w PR `sowa check --ci`, a raport `sowa review` względem gałęzi docelowej wstawia jako jeden komentarz w PR, poprawiany przy każdym pushu. Checkout bierze głowę PR z `fetch-depth: 0`: zatwierdzony commit musi być w repozytorium, a przy podpisie porównanie idzie z katalogiem roboczym, więc nie może to być commit scalenia z `main`. Na Linuksie programy z `Db` potrzebują `libsqlite3-dev`.

Czasy kompilacji i działania w różnych ustawieniach, w tym z Cranelift, są w [docs/kompilacja.md](../docs/kompilacja.md).

## Pliki

| Plik | Co robi |
|---|---|
| `src/lexer.rs` | tokeny z wcięciami (`Indent`/`Dedent`), literały `html"..."`, linie `desc`/`doc`/`why` |
| `src/parser.rs` | drzewo składni (`src/ast.rs`) z pliku `.sowa` i z bloku `sowa` w Markdownie |
| `src/project.rs`, `src/toml.rs` | `sowa.toml`, pliki z `src/` i `impl/`, nagłówki i bloki z `docs/`, CODEOWNERS, `.gitattributes` |
| `src/env.rs` | nazwy całego programu: typy, warianty, funkcje; wbudowane typy i funkcje |
| `src/check.rs` | reguły projektu (niżej) |
| `src/codegen.rs` | JavaScript dla `--bun`: typy jako opisy w runtime, funkcje jako `async function`, testy |
| `src/runtime.js` | runtime doklejany na początek programu: wartości, typy, JSON, formularze, baza, HTTP, serwer, testy |
| `src/codegen_rs.rs` | Rust dla `sowa test`, `build` i `run`: typy znane w kompilacji jako typy Rusta (`i64`, `Rc<str>`, `Rc<Vec<T>>`, `struct`), reszta jako dynamiczne `V`; błędy przez `?`, lambdy jako domknięcia |
| `src/moves.rs` | analiza dla backendu Rust: który odczyt zmiennej jest ostatni i może przenieść wartość zamiast ją klonować |
| `src/review.rs` | `sowa review`: porównanie dwóch wersji projektu, kategorie, raport |
| `src/solver.rs` | czy jeden warunek wynika z drugiego, z kontrprzykładem |
| `src/docslock.rs` | powiązania opisów z symbolami, hashe, uwagi i zapis `docs.lock` |
| `src/ci.rs` | `sowa check --ci`: zatwierdzenie z API GitHuba i zmienione pliki od niego |
| `src/sign.rs` | `[review]`, `sowa review --approve` i sprawdzenie podpisanego zatwierdzenia |
| `src/runtime.rs` | runtime dla Rusta, doklejany na początek `test_rs.rs` i `app_rs.rs` (nie jest modułem crate'a): te same wartości, typy, JSON, baza przez FFI do SQLite, serwer na `std::net`, Http przez libcurl, generator property i raport |

## Co sprawdza `sowa check`

- Sygnatura w `impl/` jest taka sama jak w `src/`, a każda funkcja z `src/` ma ciało w `impl/` w pliku o tej samej nazwie. Funkcja tylko w `impl/` jest prywatna dla swojego modułu.
- Nazwy typów, wariantów i funkcji są unikalne w projekcie. Nieznane typy, nazwy i funkcje to błąd.
- Przypisanie do istniejącej nazwy bez `var` to błąd („jest niezmienne, użyj var”), a `var` o zajętej nazwie to przesłanianie. Nieużyte nazwy to uwaga.
- Uprawnienia nie są polami, elementami list ani wynikiem. Metody uprawnień są z listy (`Db`: `get`, `all`, `save`, `transaction`; `DbRead`: `get`, `all`; `Clock`: `now`, `today`; `Http`: `post`, `get`; `Mailer`: `send`; `Server`: `serve`; `Random`: `int`, `choice`; `Terminal`: `read`, `write`, `exit`). `mail.send` dostaje tekst, tekst i `Html`. Uprawnienie przekazuje się tylko przez nazwę; `Db` można dać tam, gdzie `DbRead`.
- Parametry `main` to dokładnie zasoby z `[resources]` o tych samych typach, a `[resources.test]` ma tylko te nazwy.
- Atrapa ma sygnaturę `(HttpRequest) -> HttpResponse`, a jej ciało jest w CODEOWNERS i ma `-linguist-generated`.
- `match` z jedną wartością o znanym typie obsługuje każdy wariant albo ma `_`.
- Zmienne lokalne mają typ wartości: z przypisania, z elementu listy w `for` i we wzorcu `[a, b]`, z parametru lambdy w `map`, `filter` i `sort_by`, z wariantu we wzorcu `Wariant v`. Wyrażenia dostają typ ze stałych, pól, wyników funkcji, konstruktorów, operatorów, `as`, `with`, `try` i metod list. Gdy typ jest znany, błędem jest: argument, który nie pasuje do parametru funkcji, `return` z wartością spoza wyniku, warunek `if` i `while` oraz argumenty `&&`, `||` i `!` inne niż `Bool`, porównanie wartości różnych typów, arytmetyka na czymś innym niż liczby (`+` łączy też teksty i listy), `var` z wartością innego typu i pole rekordu z wartością innego typu. Nieznany typ niczego nie blokuje. `Int` pasuje tam, gdzie `Money`.
- Konstruktor rekordu i wariantu dostaje dokładnie swoje pola po nazwie. `==` z wariantem z danymi to błąd (trzeba `is`).
- Błędy funkcji po `try` mieszczą się w wyniku funkcji, w której stoi `try`.
- Kolejność pod sygnaturą: `desc`, `doc`, `why`, `example`, `property`. Pliki i nagłówki z `doc`/`why` istnieją, a `{Symbol}` w `desc` i w `docs/` wskazuje typ albo funkcję.
- Uwagi: `desc` dłuższy niż 3 linie, więcej niż 3 przykłady i property, przykład dłuższy niż 5 linii, opis z `docs.lock` do przejrzenia.

## Reguły, których nie ma w specyfikacji

Kompilator musiał jakoś rozstrzygnąć rzeczy, których [specyfikacja](../docs/zalozenia.md) jeszcze nie opisuje. To propozycje do przejrzenia, a nie decyzje:

- **Literał `html"..."`.** W jednej linii kończy się na `"` poza `{}`. `html"` na końcu linii zaczyna literał wieloliniowy, który kończy linia zaczynająca się od `"`; wspólne wcięcie jest usuwane. Każda wartość w `{}` jest escapowana (`& < > " '`, wszystkie pięć wszędzie, bez rozróżniania miejsca), a `Html` i listy `Html` wstawiają się bez zmian.
- **`try f(...)`.** Stoi tylko przed wywołaniem funkcji z programu albo przed `mail.send(...)`. Pierwszy wariant wyniku to sukces, a jeśli wartość pasuje do reszty wariantów, funkcja z `try` od razu ją zwraca. W przykładzie taki wynik to niezaliczony test.
- **`tekst as Rekord`.** Tekst zaczynający się od `{` albo `[` to JSON, a inny to dane formularza (`application/x-www-form-urlencoded`) z kluczami `lines[0].name`. Brakująca lista w formularzu to pusta lista. Kwoty w JSON-ie nie przechodzą przez liczby zmiennoprzecinkowe.
- **Baza.** `db.get<T>(kolekcja, key:)` zwraca `T | NoRow` (albo `DbError`, gdy zapisane dane nie pasują do `T`), `db.all<T>(kolekcja)` zwraca listę po kluczu, `db.save(kolekcja, key:, value:)` zapisuje albo nadpisuje. `db.transaction(tx => ...)` zwraca wynik lambdy albo `DbError`; transakcje na jednej bazie idą po kolei. W SQLite to jedna tabela `(collection, key, value)` z wartością w JSON-ie.
- **Serwer.** `web.serve(req => ...)` buduje rekord `Request` z programu (albo `HttpRequest`, gdy go nie ma): `method` to `Get`, `Post`, `Put`, `Patch` albo `Delete`, inne metody dostają 405. Warianty odpowiedzi mapują się na statusy: `Ok` 200, `Redirect` 303 z `Location: to`, `BadRequest` 400, `NotFound` 404, `BadGateway` 502, inne 500. Pole `body` to treść `text/html; charset=utf-8`. Błąd programu to 500 i wpis w logu.
- **Http.** `ksef.post(path, body)` wysyła `POST url + path` z tokenem `Bearer` ze zmiennej z `token_env` i zwraca `HttpResponse` albo `HttpError` (brak połączenia, 10 s bez odpowiedzi). Z `fake = "f"` woła czystą funkcję `f` z programu.
- **Mailer.** PROPOZYCJA: `mail.send(to: String, subject: String, body: Html)` zwraca `Sent | MailError`, gdzie wbudowany `MailError` to `MailRejected | MailTimeout`. Brak połączenia albo 20 s bez odpowiedzi to `MailTimeout`, każda odmowa serwera to `MailRejected`. Zasób ma `server` (`host:port`), `from` (nadawca) i `login_env` (zmienna z `użytkownik:hasło`). Port 465 to SMTPS, na innych portach STARTTLS jest obowiązkowy, poza serwerem na `localhost`. Treść idzie jako `text/html` w base64. Nowa linia albo `<>` w adresie lub temacie to `MailRejected`. `server = "memory"` niczego nie wysyła i zwraca `Sent`. Załączników (np. PDF) nie ma, to otwarte pytanie.
- **Alias unii.** PROPOZYCJA: `type SendError = MailError` bez warunku to inna nazwa tej samej unii, więc `match` na `SendError` obsługuje `MailRejected` i `MailTimeout`, a `try` przepuszcza je do wyniku z `SendError`. Z warunkiem (`type Small = Int(α < 10)`) to zawężenie, jak dotąd.
- **Zegar.** `clock.now()` to `DateTime` w czasie lokalnym, `clock.today()` to `Date`. W testach zegar stoi na `now` z `[resources.test]`.
- **Liczby.** `Int` to liczba całkowita (poza zakresem ±2⁵³ to błąd programu), `Money` to liczba dziesiętna bez zaokrągleń przy `+`, `-`, `*`; `/` liczy z 20 miejscami, `round(x, 2)` zaokrągla połówki od zera. `Int` pasuje tam, gdzie `Money`.
- **Wykonanie.** Każda funkcja jest asynchroniczna i każde wywołanie czeka na wynik, więc kod w Sowie nie ma `async`. Warunki w typach są synchroniczne i nie mogą wołać funkcji z programu, metod, lambd, `or` ani `try`.
- **Warunki w runtime.** Każdy parametr i wynik funkcji jest sprawdzany z typem przy wywołaniu. Niespełniony warunek to błąd programu z nazwą funkcji, a nie zwykły wariant.
- **Typ zmiennej `var`.** PROPOZYCJA: `var` ma stały typ, a nadpisanie wartością innego typu to błąd w `sowa check`. Wyjątki: zmienna z `Int` po przypisaniu `Money` ma odtąd typ `Money` (`var total = 0`, potem kwoty), a `[]` dostaje typ pierwszej listy ze znanym elementem. Runtime na to pozwala: w Bunie zmienna nie ma typu, a w Ruście staje się dynamiczna.
- **`==` na różnych typach.** PROPOZYCJA: w runtime `"1" == 1` to `false`, a `sowa check` zgłasza to jako błąd, bo taki warunek nigdy nie jest prawdziwy.
- **Typy w `impl/`.** Plik w `impl/` może definiować własne typy (np. `VatInputError` w `impl/issuing.sowa`). Ich nazwy są globalne jak wszystkie inne.
- **Potwierdzanie opisów.** PROPOZYCJA: specyfikacja mówi, że `sowa review` „pyta” o opisy. Tu `sowa review --base` tylko je wypisuje, a potwierdza się osobno: `sowa review --confirm-docs` (wszystkie) albo `--confirm-docs=plik.md#sekcja` (jeden). Dzięki temu review działa bez terminala, np. w CI. Powiązanie to para (opis, symbol); nagłówek pliku z `doc`/`why` wiąże cały plik, a `{Symbol}` najbliższą sekcję nad nim.
- **Zatwierdzenie bez GitHuba.** PROPOZYCJA: `sowa check --approved-at REF` porównuje `REF` z katalogiem roboczym. Służy do sprawdzenia lokalnego i na platformach bez recenzji PR w API GitHuba.
- **Zatwierdzenie podpisem.** PROPOZYCJA: `[review] sign = true` zamienia recenzję PR na podpisany commit z `sowa review --approve`. `approvers` to lista `"@login"` (klucze do podpisu z GitHuba) albo adresów e-mail (klucze z `SOWA_ALLOWED_SIGNERS`). Wiersz `Sowa-Approved` ma katalog projektu względem korzenia repozytorium, więc jedno repozytorium może mieć kilka projektów z osobnymi zatwierdzeniami. Zatwierdzenie dotyczy drzewa plików commitu, a nie gałęzi: liczy się najnowszy ważny commit w historii `HEAD`, a zmiany po nim ocenia się jak po zatwierdzeniu PR. `[review] agent` jest przyjmowany, ale daje tylko uwagę, bo drugi agent-recenzent jeszcze nie działa.
- **Kategorie w review.** PROPOZYCJA, poza tabelą z [zalozenia.md](../docs/zalozenia.md#sowa-review): zmiana w `[review]` w `sowa.toml` to uprawnienie (zmienia, kto zatwierdza); nowy wariant w typie, który stoi w wyniku funkcji ze specyfikacji, to osłabienie, a w innym typie zwykła zmiana; zmienione ciało atrapy w `impl/` to usunięty test; zawężenie `type X = T(warunek)` bez zmiany znaczenia to zwykła zmiana z dopiskiem „to samo znaczenie”. Nowa funkcja z uprawnieniem to uprawnienie, a nowa czysta funkcja zwykła zmiana.
- **Warianty w solverze.** PROPOZYCJA: `α.status != Issued` przy typie `Issued | Paid | Cancelled` solver rozumie jako „jeden z pozostałych wariantów”, więc `== Paid` jest ostrzejsze od `!= Issued`, a kontrprzykładem jest konkretny wariant.
- **Property.** Parametry bez uprawnień są losowane z typu: 100 przypadków, ziarno z pliku i linii, więc wynik jest powtarzalny. Generator korzysta z warunku (`α > 0`, `len(α) == 10`, `matches(α, "...")`, `only_digits`, `nip_checksum_ok`, `valid_email`, `starts_with`) i odrzuca wartości, które warunku nie spełniają. Teksty losuje też z listy trudnych przypadków (`<script>`, `&`, cudzysłowy, polskie litery).

## Ograniczenia

- `match` na kilku wartościach nie jest sprawdzany pod kątem kompletności; brak pasującej gałęzi to błąd programu w runtime.
- Wnioskowanie typów jest lokalne i ostrożne: nie zawęża typu po `is`, nie wyprowadza wyniku lambdy z blokiem, nie porównuje elementów list (`List<Int>` i `List<String>` to dla niego ta sama `List`) ani typów wbudowanych funkcji poza `len`, `at`, `join` i podobnymi. Warunków w typach (`Int(α > 0)`) nie sprawdza, tylko ich podstawę. Część błędów typów wychodzi więc dopiero w runtime albo w testach.
- Solver zna tylko arytmetykę liniową na liczbach całkowitych, `len`, pola i warianty. Mnożenie dwóch zmiennych, `matches` i wywołania funkcji porównuje tekstowo.
- `sowa review` nie pokazuje, które warunki wyniku są udowodnione, i nie ma testów mutacyjnych. Nie ma też `[review] agent` ani grupowania dużych zmian po module.
- `sowa check --ci` w trybie PR działa tylko z GitHubem i potrzebuje `gh` z tokenem; w trybie podpisu `gh` jest potrzebny tylko do kluczy `@login`. `sowa review --approve` nie podpisuje kluczem GPG ani X.509, tylko SSH. Po force-pushu, który usunął zatwierdzony commit, check nie przechodzi i trzeba zatwierdzić PR ponownie.
