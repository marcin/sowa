# Kompilator Sowy (prototyp)

Najprostszy kompilator, który wystarcza, żeby sprawdzić, przetestować i uruchomić [examples/fakturownia_web](../examples/fakturownia_web/). Napisany w Ruście bez zależności. Tłumaczy cały projekt na jeden plik JavaScript, który uruchamia [Bun](https://bun.sh). Bazę daje wbudowany w Buna `bun:sqlite`, więc program też nie ma zależności.

Nie ma solvera, `sowa review` ani mutacji. Warunki w typach kompilator sprawdza w runtime, a nie dowodzi.

## Użycie

```
cargo build --manifest-path compiler/Cargo.toml
sowa check [KATALOG]               # sprawdza projekt
sowa test [KATALOG]                # przykłady, property (po 100 przypadków) i bloki sowa z docs/
sowa test --rust [KATALOG]         # to samo, ale testy są kompilowane do Rusta
sowa build [KATALOG]               # zapisuje program w KATALOG/.sowa/app.js
sowa run [--fake ZASÓB] [KATALOG]  # buduje i uruchamia
```

`KATALOG` to katalog z `sowa.toml` albo dowolny katalog pod nim. `--fake ksef` bierze opis zasobu `ksef` z `[resources.test]`, czyli atrapę. Bun jest szukany w `SOWA_BUN`, potem w `PATH`, potem w `~/.bun/bin/bun`.

`sowa test --rust` tłumaczy program i testy na jeden plik `.sowa/test_rs.rs`, kompiluje go `rustc -O` do `.sowa/test_rs` i uruchamia. Wynik i komunikaty są takie same jak z Buna, łącznie z wylosowanymi przypadkami property. Kompilacja jest pomijana, gdy kod się nie zmienił. SQLite pochodzi z systemowej biblioteki `libsqlite3`. `rustc` jest szukany w `SOWA_RUSTC`, potem w `PATH`, potem w `~/.cargo/bin/rustc`. Ten backend obsługuje tylko testy, więc nie ma w nim serwera ani prawdziwego `Http`, tylko atrapę.

## Pliki

| Plik | Co robi |
|---|---|
| `src/lexer.rs` | tokeny z wcięciami (`Indent`/`Dedent`), literały `html"..."`, linie `desc`/`doc`/`why` |
| `src/parser.rs` | drzewo składni (`src/ast.rs`) z pliku `.sowa` i z bloku `sowa` w Markdownie |
| `src/project.rs`, `src/toml.rs` | `sowa.toml`, pliki z `src/` i `impl/`, nagłówki i bloki z `docs/`, CODEOWNERS, `.gitattributes` |
| `src/env.rs` | nazwy całego programu: typy, warianty, funkcje; wbudowane typy i funkcje |
| `src/check.rs` | reguły projektu (niżej) |
| `src/codegen.rs` | JavaScript: typy jako opisy w runtime, funkcje jako `async function`, testy |
| `src/runtime.js` | runtime doklejany na początek programu: wartości, typy, JSON, formularze, baza, HTTP, serwer, testy |
| `src/codegen_rs.rs` | Rust dla `sowa test --rust`: funkcje jako `fn(V) -> R`, błędy przez `?`, lambdy jako domknięcia |
| `src/runtime.rs` | runtime dla Rusta, doklejany na początek `test_rs.rs` (nie jest modułem crate'a): te same wartości, typy, JSON, baza przez FFI do SQLite, generator property i raport |

## Co sprawdza `sowa check`

- Sygnatura w `impl/` jest taka sama jak w `src/`, a każda funkcja z `src/` ma ciało w `impl/` w pliku o tej samej nazwie. Funkcja tylko w `impl/` jest prywatna dla swojego modułu.
- Nazwy typów, wariantów i funkcji są unikalne w projekcie. Nieznane typy, nazwy i funkcje to błąd.
- Przypisanie do istniejącej nazwy bez `var` to błąd („jest niezmienne, użyj var”), a `var` o zajętej nazwie to przesłanianie. Nieużyte nazwy to uwaga.
- Uprawnienia nie są polami, elementami list ani wynikiem. Metody uprawnień są z listy (`Db`: `get`, `all`, `save`, `transaction`; `DbRead`: `get`, `all`; `Clock`: `now`, `today`; `Http`: `post`, `get`; `Server`: `serve`). Uprawnienie przekazuje się tylko przez nazwę; `Db` można dać tam, gdzie `DbRead`.
- Parametry `main` to dokładnie zasoby z `[resources]` o tych samych typach, a `[resources.test]` ma tylko te nazwy.
- Atrapa ma sygnaturę `(HttpRequest) -> HttpResponse`, a jej ciało jest w CODEOWNERS i ma `-linguist-generated`.
- `match` z jedną wartością o znanym typie obsługuje każdy wariant albo ma `_`.
- Konstruktor rekordu i wariantu dostaje dokładnie swoje pola po nazwie. `==` z wariantem z danymi to błąd (trzeba `is`).
- Błędy funkcji po `try` mieszczą się w wyniku funkcji, w której stoi `try`.
- Kolejność pod sygnaturą: `desc`, `doc`, `why`, `example`, `property`. Pliki i nagłówki z `doc`/`why` istnieją, a `{Symbol}` w `desc` i w `docs/` wskazuje typ albo funkcję.
- Uwagi: `desc` dłuższy niż 3 linie, więcej niż 3 przykłady i property, przykład dłuższy niż 5 linii.

## Reguły, których nie ma w specyfikacji

Kompilator musiał jakoś rozstrzygnąć rzeczy, których [specyfikacja](../docs/zalozenia.md) jeszcze nie opisuje. To propozycje do przejrzenia, a nie decyzje:

- **Literał `html"..."`.** W jednej linii kończy się na `"` poza `{}`. `html"` na końcu linii zaczyna literał wieloliniowy, który kończy linia zaczynająca się od `"`; wspólne wcięcie jest usuwane. Każda wartość w `{}` jest escapowana (`& < > " '`, wszystkie pięć wszędzie, bez rozróżniania miejsca), a `Html` i listy `Html` wstawiają się bez zmian.
- **`try f(...)`.** Stoi tylko przed wywołaniem funkcji z programu. Pierwszy wariant wyniku to sukces, a jeśli wartość pasuje do reszty wariantów, funkcja z `try` od razu ją zwraca. W przykładzie taki wynik to niezaliczony test.
- **`tekst as Rekord`.** Tekst zaczynający się od `{` albo `[` to JSON, a inny to dane formularza (`application/x-www-form-urlencoded`) z kluczami `lines[0].name`. Brakująca lista w formularzu to pusta lista. Kwoty w JSON-ie nie przechodzą przez liczby zmiennoprzecinkowe.
- **Baza.** `db.get<T>(kolekcja, key:)` zwraca `T | NoRow` (albo `DbError`, gdy zapisane dane nie pasują do `T`), `db.all<T>(kolekcja)` zwraca listę po kluczu, `db.save(kolekcja, key:, value:)` zapisuje albo nadpisuje. `db.transaction(tx => ...)` zwraca wynik lambdy albo `DbError`; transakcje na jednej bazie idą po kolei. W SQLite to jedna tabela `(collection, key, value)` z wartością w JSON-ie.
- **Serwer.** `web.serve(req => ...)` buduje rekord `Request` z programu (albo `HttpRequest`, gdy go nie ma): `method` to `Get`, `Post`, `Put`, `Patch` albo `Delete`, inne metody dostają 405. Warianty odpowiedzi mapują się na statusy: `Ok` 200, `Redirect` 303 z `Location: to`, `BadRequest` 400, `NotFound` 404, `BadGateway` 502, inne 500. Pole `body` to treść `text/html; charset=utf-8`. Błąd programu to 500 i wpis w logu.
- **Http.** `ksef.post(path, body)` wysyła `POST url + path` z tokenem `Bearer` ze zmiennej z `token_env` i zwraca `HttpResponse` albo `HttpError` (brak połączenia, 10 s bez odpowiedzi). Z `fake = "f"` woła czystą funkcję `f` z programu.
- **Zegar.** `clock.now()` to `DateTime` w czasie lokalnym, `clock.today()` to `Date`. W testach zegar stoi na `now` z `[resources.test]`.
- **Liczby.** `Int` to liczba całkowita (poza zakresem ±2⁵³ to błąd programu), `Money` to liczba dziesiętna bez zaokrągleń przy `+`, `-`, `*`; `/` liczy z 20 miejscami, `round(x, 2)` zaokrągla połówki od zera. `Int` pasuje tam, gdzie `Money`.
- **Wykonanie.** Każda funkcja jest asynchroniczna i każde wywołanie czeka na wynik, więc kod w Sowie nie ma `async`. Warunki w typach są synchroniczne i nie mogą wołać funkcji z programu, metod, lambd, `or` ani `try`.
- **Warunki w runtime.** Każdy parametr i wynik funkcji jest sprawdzany z typem przy wywołaniu. Niespełniony warunek to błąd programu z nazwą funkcji, a nie zwykły wariant.
- **Typy w `impl/`.** Plik w `impl/` może definiować własne typy (np. `VatInputError` w `impl/issuing.sowa`). Ich nazwy są globalne jak wszystkie inne.
- **Property.** Parametry bez uprawnień są losowane z typu: 100 przypadków, ziarno z pliku i linii, więc wynik jest powtarzalny. Generator korzysta z warunku (`α > 0`, `len(α) == 10`, `matches(α, "...")`, `only_digits`, `nip_checksum_ok`, `valid_email`, `starts_with`) i odrzuca wartości, które warunku nie spełniają. Teksty losuje też z listy trudnych przypadków (`<script>`, `&`, cudzysłowy, polskie litery).

## Ograniczenia

- `match` na kilku wartościach nie jest sprawdzany pod kątem kompletności; brak pasującej gałęzi to błąd programu w runtime.
- Typy wyrażeń nie są wyprowadzane poza prostymi przypadkami (parametr, wynik funkcji), więc część błędów typów wychodzi dopiero w runtime albo w testach.
- Brak `sowa review`, solvera, mutacji, `docs.lock` i `--ci`.
