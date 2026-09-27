# Porównanie z innymi językami

## Sowa a popularne języki

| Język | Warunki na typach (0–100) | Efekty w sygnaturze | Błędy | Zmienne | Null | „Magia” | GC |
|---|---|---|---|---|---|---|---|
| [**Rust**](https://www.rust-lang.org) | nie ma w języku; `nutype` w runtime, [Flux](https://github.com/flux-rs/flux) (badawczy) | nie ma (tylko `unsafe`, `async`) | `Result<T, E>`, `?`, wyczerpujący `match` | `let` stała, `let mut` zmienna | brak, jest `Option` | makra | nie |
| [**Go**](https://go.dev) | nie ma | nie ma | `(T, error)`, ręczne `if err != nil`, brak sum typów | `:=` / `var`, wszystko zmienne | `nil` | mało | tak |
| [**TypeScript**](https://www.typescriptlang.org) | nie ma; Zod / ArkType w runtime, typy oznaczone (brand) | nie ma | `throw`, unie typów, wyczerpanie przez trik z `never` | `const` / `let` | `strictNullChecks` | średnio (pod spodem JS) | tak |
| [**Python**](https://www.python.org) | nie ma; Pydantic w runtime | nie ma | wyjątki | `x = 1`, bez deklaracji, wszystko zmienne | `None` | dużo (dekoratory, metaklasy) | tak |
| [**Ruby**](https://www.ruby-lang.org) | nie ma; `validates`, dry-types w runtime | nie ma | wyjątki | `x = 1`, bez deklaracji, wszystko zmienne | `nil` | bardzo dużo (monkey-patching, `method_missing`) | tak |
| [**Elixir**](https://elixir-lang.org) | strażniki `when` w runtime | nie ma, ale procesy izolują stan | `{:ok, x}` / `{:error, e}`, `with ... else` | dane niezmienne, nazwy można przepinać | `nil` | makra | tak (BEAM) |
| [**Kotlin**](https://kotlinlang.org) | nie ma | nie ma (`suspend` częściowo) | wyjątki, klasy `sealed` | `val` stała, `var` zmienna | `?` w typie | mało | tak |
| [**Swift**](https://www.swift.org) | nie ma | częściowo: `throws`, `async` | `throws` (typowane), `Result`, wyczerpujący `switch` | `let` / `var` | `?` w typie (`Optional`) | mało | ARC |
| **Sowa** | **`Int(α >= 0 && α <= 100)`, sprawdzane w kompilacji** | **uprawnienia jako parametry (`db: Db`, `mail: Mailer`), sprawdzane w kompilacji** | typ wyniku `A \| B`, `try`, wyczerpujący `match` | **`x = 1` stała, `var` zmienna** | brak (typ bez `nil`) | zero | otwarte |

Ciekawe podobieństwa:

- **Zmienne:** zapis `pct = ...` wygląda jak w Rubym i Pythonie, ale działa jak `val` w Kotlinie. W żadnym z tych języków nie ma dokładnie takiej kombinacji.
- **`as ... or`:** to ten sam problem, który Swift rozwiązuje przez `guard let x = ... else { return }`, a Elixir przez `with ... else`. Sowa wybiera krótszą, jednoliniową formę.
- **Warunki i uprawnienia w sygnaturze:** żaden popularny język nie ma obu naraz. Najbliżej są Swift (`throws`, `async`) i Rust (`Result`).

## Sowa a języki projektowane w erze AI

Dane pochodzą z katalogu [agentlanguages.dev](https://agentlanguages.dev) (stan na 21.09.2026, 42 projekty). Tabela obejmuje projekty najbliższe Sowie. Linki prowadzą do repozytoriów (dla Prove i MoonBit do stron projektu).

| Język | Nurt | Kluczowa idea | Weryfikacja | ★ | Dojrzałość | Wspólne z Sową |
|---|---|---|---|---|---|---|
| [**NanoLang**](https://github.com/jordanhubbard/nanolang) | weryfikacyjny | testy „cienie” przy kodzie, 193 twierdzenia w Coq | Coq | 629 | działa | nacisk na sprawdzalność |
| [**Vera**](https://github.com/aallan/vera) | weryfikacyjny | obowiązkowe kontrakty, typowane odwołania zamiast nazw, wywołania LLM jako efekt | Z3 | 414 | działa | kontrakty i efekty |
| [**Aver**](https://github.com/jasisz/aver) | weryfikacyjny | „AI pisze, człowiek przegląda kontrakty i intencję”: efekty, `verify`, bloki `decision`, widok kontraktów `aver context`, dozwolone hosty w `aver.toml` | eksport do Lean 4 i Dafny | 60 | działa | **ta sama teza**, zob. niżej |
| [**Thermite**](https://github.com/dollspace-gay/Thermite) | weryfikacyjny | kontrakty najpierw (`req` / `ens` / `fx`), generuje Rusta | Verus, Lean | 54 | działa | niemal ta sama sygnatura |
| [**AILANG**](https://github.com/sunholo-data/ailang) | weryfikacyjny | efekty jako uprawnienia, inferencja typów HM, kod pisany przez AI | typy | 34 | działa | efekty |
| [**Vow**](https://github.com/vow-lang/vow) | weryfikacyjny | sprawdzane maszynowo „przysięgi” | ESBMC (bounded model checking) | 8 | działa | kontrakty |
| [**Intent**](https://github.com/lhaig/intent) | weryfikacyjny | warunki wejścia i wyjścia, wiele języków docelowych | Z3 | 7 | działa | kontrakty |
| [**Prove**](https://prove.botwork.se) | weryfikacyjny | intencja najpierw, **typy z warunkami** | typy z warunkami | – | działa | **najbliżej `Int(α ...)`** |
| [**Zero**](https://github.com/vercel-labs/zerolang) (Vercel Labs) | weryfikacyjny | diagnostyka w JSON, bardzo małe pliki wykonywalne | – | 5,4 tys.+ | wczesna | czytelne błędy dla agenta |
| [**MoonBit**](https://www.moonbitlang.com) | weryfikacyjny | próbkowanie tokenów świadome semantyki (ICSE 2024) | typy | – | najdojrzalszy | znajoma składnia |
| [**Mog**](https://github.com/voltropy/mog) | składniowy | operatory bez priorytetów, uprawnienia, specyfikacja na 3,2 tys. tokenów | – | 142 | działa | mała specyfikacja |
| [**Lume**](https://github.com/mavboas/lume) | składniowy | domyślnie niezmienne | – | 1 | wczesna | stałe domyślnie |
| [**Boruna**](https://github.com/escapeboy/boruna) | orkiestracyjny | deterministyczne wykonanie, łańcuchy dowodowe z hashami | polityki | 5 | działa | pochodzenie i recenzja kodu |
| [**Lumen**](https://github.com/alliecatowo/lumen) | orkiestracyjny | kod w Markdownie, efekty algebraiczne | `@deterministic` | 1 | działa | efekty |

Sowa jest na razie szkicem na papierze.

## Sowa a przegląd kodu z agenta

Tu Sowa ma najwięcej własnego. Tabela zestawia każdą z tych rzeczy z tym, co jest najbliżej.

| Co | Sowa | Najbliżej | Różnica |
|---|---|---|---|
| człowiek czyta specyfikację, a nie kod | `src/` ze specyfikacją pod CODEOWNERS, `impl/` zwinięty w PR ([specyfikacja](specyfikacja.md)) | Aver: `aver context` pokazuje same kontrakty; Ada/SPARK (`.ads`), OCaml (`.mli`) | w Averze to widok, a agent może po cichu zmienić kontrakt; w Adzie i OCamlu podział służy kompilacji; w Sowie to granica zatwierdzania, a kod nie wyjdzie poza specyfikację |
| lista decyzji zamiast diffu | `sowa review`: nowe uprawnienie, osłabiony warunek z kontrprzykładem z solvera, usunięty test, od najbardziej ryzykownych | CODEOWNERS (ścieżki), [cargo-vet](https://github.com/mozilla/cargo-vet) (zależności), [buf breaking](https://buf.build/docs/breaking/) i [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks) (zmiany API) | tamte widzą pliki, biblioteki albo kształt API, a Sowa znaczenie warunków i uprawnień |
| kiedy człowiek zatwierdza | raz, gdy PR działa; po zatwierdzeniu `sowa check --ci` nie przepuszcza zmian specyfikacji, a poprawki w `impl/` tak | GitHub: „Dismiss stale approvals” | GitHub kasuje zatwierdzenie po każdym commicie albo po żadnym; Sowa odróżnia zmianę specyfikacji od zmiany kodu |
| dokąd wolno wysłać dane | `mail: Mailer` w parametrze, serwer w `[resources]` w `sowa.toml`; uprawnienia nie da się utworzyć w kodzie; runtime pozwala tylko na te adresy | Aver: `[effects.Http] hosts` w `aver.toml`; Deno: `--allow-net`; object capabilities (E, Pony, Austral, WASI) | w Averze i Deno lista hostów dotyczy całego programu i działa dopiero w runtime; języki z capabilities mają ten sam mechanizm, ale bez zatwierdzania: w Sowie nowe uprawnienie zawsze jest na górze `sowa review` |
| które moduły mogą łączyć się z siecią albo bazą | widać z sygnatur w `src/`: moduł bez uprawnień w parametrach jest czysty, bez osobnej konfiguracji | lintery importów ([dependency-cruiser](https://github.com/sverweij/dependency-cruiser), ArchUnit) | lintery sprawdzają importy, a w Sowie bez parametru nie ma czym wysłać, więc wywołania pośrednie też są objęte |
| testy, których agent nie osłabi | `example`, `property` i warunki wyniku w `src/` są specyfikacją, więc ich zmiana wymaga zgody; wynik testów mutacyjnych w `sowa review` | Aver i Vera: przykłady przy funkcji; [Stryker](https://stryker-mutator.io), [PIT](https://pitest.org): mutacje | tam agent może przepisać test, żeby przeszedł; w Sowie to zmiana specyfikacji |
| opis, który się nie zdezaktualizuje | `docs.lock` wskazuje akapity do przejrzenia po zmianie sygnatury | doctesty w Ruście, Elixirze, Pythonie; Vera uruchamia przykłady z dokumentacji w CI | doctesty sprawdzają kod w opisie, a nie tekst; wykrywania nieaktualnego tekstu nie ma w żadnym z przejrzanych projektów |
| proces, którego nie da się obejść po cichu | `sowa check` sprawdza, czy CODEOWNERS obejmuje `src/`, `docs/`, `sowa.toml` i `docs.lock` | brak | |

## Wnioski

- **Gwarancje typów opierają się na sprawdzonych pomysłach.** Kontrakty, efekty, uprawnienia jako wartości i typy z warunkami działają już w Verze, Thermite, Averze, Prove, Ponym i Australu, z kompilatorami i solverami. Sowa nie musi więc udowadniać, że to w ogóle działa, i może z ich doświadczeń korzystać. Swoją nowość wnosi w tym, jak człowiek zatwierdza kod.
- **Czytelność to słaba przewaga.** Większość tych projektów optymalizuje pod model: typowane odwołania zamiast nazw w Verze, JSON zamiast tekstu w [X07](https://github.com/x07lang/x07), jednoznakowe instrukcje w [Severze](https://github.com/AvitalTamir/sever). Sowa projektuje składnię pod recenzenta: `α`, stałe bez słowa kluczowego z `var`, `as ... or`, zero skrótów. Składnię łatwo jednak skopiować.
- **Wyróżnia ją to, że zatwierdzanie jest egzekwowane.** Aver ma tę samą tezę („AI pisze, człowiek przegląda kontrakty”), ale przegląd jest tam zaleceniem: agent może zmienić kontrakt albo test i nic tego nie oznaczy. W Sowie specyfikacja jest granicą: kod nie wyjdzie poza nią, a jej zmiana nie wejdzie bez zgody człowieka. Tego połączenia nie ma w żadnym z przejrzanych projektów.
- **Aver warto śledzić.** To najbliższy projekt: bloki `decision` odpowiadają `why` i plikom w `decyzje/`, a `[effects.Http] hosts` odpowiada `[resources]`. Część pomysłów da się przenieść w obie strony.
- **Popularność:** najwięcej gwiazdek w nurcie weryfikacyjnym ma Zero (5,4 tys.+), a dalej NanoLang (629) i Vera (414).
- **Dojrzałość:** realne zainteresowanie mają tylko Zero, [Fabro](https://github.com/fabro-sh/fabro) (orkiestracja, 1,6 tys.+) i MoonBit. Reszta to projekty z kilkudziesięcioma gwiazdkami lub mniej.
