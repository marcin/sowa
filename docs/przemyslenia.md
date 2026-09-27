# Przemyślenia

Notatki z rozmowy, z której wziął się ten projekt: dlaczego tak, co odrzuciliśmy i co zostaje otwarte.

## Punkt wyjścia

Punktem wyjścia był wpis José Valima (twórcy Elixira) [„Evolving programming languages in the AI era”](https://dashbit.co/blog/evolving-ai-era) z 24.09.2026. Najważniejsze tezy:

- Nowe języki „dla agentów”, skupione na składni, rozwiązują dzisiejsze ograniczenia modeli. Dla modelu różnice w składni mają małe znaczenie.
- Agentom nie przeszkadza żmudność, więc jawne typy i sygnatury dają kompilatorowi więcej informacji za darmo.
- Warto łączyć poprawność z konstrukcji, analizę statyczną, gwarancje runtime'u i testy (w tym fuzzing).
- Zamiast LSP (pliki, linie, kolumny) lepsza jest baza danych programu z językiem zapytań.
- Zamiast debuggera lepszy jest obserwowalny runtime, który agent może odpytywać programowo.

Wniosek dla Sowy: prawdziwym problemem ery AI nie jest szybkość pisania, tylko **zaufanie do kodu, którego nie napisałeś**.

## Główne napięcia w projekcie

- **Brak danych treningowych.** Modele gorzej piszą w niszowych językach. Stąd znajoma składnia i mała specyfikacja, która w całości mieści się w kontekście agenta. Uwaga: język Vera pokazuje w benchmarkach przewagę nad Pythonem mimo zerowych danych treningowych, więc ten argument może być słabszy, niż zakładamy.
- **Rozwlekłość a czytelność.** Jawność pomaga w weryfikacji, ale zbyt dużo szumu utrudnia recenzję. Rozwiązanie: rygor na granicach, swoboda w środku.
- **Siła gwarancji a koszt.** Pełna weryfikacja formalna jest za droga dla typowego SaaS-a. Gwarancje są więc stopniowane: domyślnie typy i efekty, opcjonalnie kontrakty, a dowody tylko w krytycznych modułach.

## Jak doszliśmy do `α`

Po kolei rozważaliśmy te zapisy warunku „od 0 do 100”:

| Zapis | Dlaczego nie |
|---|---|
| `pct.between?(0, 100)` | trzeba wiedzieć, czy granice wchodzą w zakres |
| `0..100` | Ruby: 100 wchodzi, Rust: 100 nie wchodzi |
| `Int where 0 <= self <= 100` | `where` i `self` to dwa słowa do nauczenia |
| `Int from 0 to 100`, `above 0`, `at least 0` | trzeba pamiętać, że `above` nie obejmuje granicy, a `at least` obejmuje |
| `Int(0 <= x <= 100)` | w C, Javie i JS zapis łańcuchowy znaczy co innego (zawsze prawda) |
| `Int(x >= 0 && x <= 100)` | `x` koliduje ze zwykłymi zmiennymi |
| zarezerwować `x` | to jedna z najczęściej używanych nazw, a modele ciągle by jej używały |
| `_`, `it` | `it` to też może być zwykła zmienna; `_ > 0` czyta się gorzej |
| `$`, `?`, `@`, `#` | działają, ale nie wyglądają; `#` kojarzy się z komentarzem, `@` z adnotacją |
| `{x in Int \| x > 0}` | ładna notacja zbiorów, ale dłuższa |
| `Int(> 0, <= 100)` (sam operator) | krótkie, ale to druga forma zapisu i reguła „przecinek znaczy i” |
| **`Int(α >= 0 && α <= 100)`** | **wybrane** |

Dlaczego `α`:

- W zwykłym kodzie nikt nie nazywa tak zmiennych, więc nie ma kolizji.
- W matematyce często oznacza „dowolny element”.
- Brak klawisza nie jest problemem, skoro kod pisze głównie agent. A jak ktoś chce pisać z klawiatury, podaje własną nazwę (`n => ...`) albo używa `\a` w edytorze.
- Plik wygląda tak samo wszędzie: w edytorze, na GitHubie, w terminalu i na Slacku. Przy triku „w pliku `$`, na ekranie `α`” poza edytorem widać by było co innego.
- Jedna forma dla wszystkich warunków: `Money(α > 0)` i `List(len(α) > 0)` wyglądają tak samo, więc nie ma osobnej reguły dla prostych porównań.

Ogólna lekcja: skróty w stylu Ruby'ego sprawdzają się w zwykłym kodzie, ale w warstwie, którą się weryfikuje, jawność wygrywa z elegancją.

## Inne decyzje

- **`fn` zamiast `def` / `function` / `func`.** Krótkie, jednoznaczne i pasuje do typów w nawiasach. Najważniejsze, żeby w całym języku było jedno słowo.
- **Bez `let`, z `var`.** Porównaliśmy trzy warianty:

  | | stała | zmienna modyfikowalna | wada |
  |---|---|---|---|
  | tylko `var` (Go, stary JS) | `var pct = ...` | `var total = 0` | nie widać, co się zmienia; ginie gwarancja po `as Percent` |
  | `let` + `var` (Swift, Kotlin) | `let pct = ...` | `var total = 0` | słowo kluczowe w każdej linii, dwa słowa do odróżnienia |
  | **bez `let`, `var`** | `pct = ...` | `var total = 0` | **wybrane**; jedyny koszt: brak przesłaniania nazw |

  Bez `let` samo `=` byłoby niejednoznaczne (nowa nazwa czy nadpisanie?), dlatego nadpisanie wymaga `var`, a nieużywana nazwa to błąd. Brak przesłaniania (`input = input.trim()`) traktujemy jako plus, bo wymusza opisowe nazwy.
- **`as ... or` zamiast `as ... else`.** `else` bez `if` zaskakuje: czytelnik szuka warunku, a ten jest ukryty w `as`. Przeniesione do nowej linii wygląda jak osobny blok. `or` mieści się w jednej linii i czyta się jak zdanie („zamień albo zwróć błąd”), podobnie jak `open(...) or die` w Perlu i Rubym. Słowo jest wolne, bo do warunków logicznych używamy `&&` i `||`. Zwykły `if input is not Percent` zostaje jako alternatywa zbudowana tylko ze znanych konstrukcji.
- **Ruby: składnia tak, semantyka nie.** Z Ruby'ego warto wziąć brak średników i lekkość zapisu. Nie bierzemy monkey-patchingu, `method_missing`, metaprogramowania ani DSL-i, w których nie wiadomo, skąd bierze się metoda. Tą drogą poszły już Elixir (składnia z Ruby'ego, semantyka z Erlanga) i Crystal.
- **Jawne zamiast skrótów:**
  - `unless x` → `if not x`
  - `users.map(&:email)` → `users.map(u => u.email)`
  - `cache ||= load()` → jawny `if`
  - `user&.email` → `match` albo typ bez `nil`
  - `3.days.ago` → `now() - days(3)`

## Jak to robią inni

| | nazwa wartości | warunki na typach |
|---|---|---|
| Ruby | `\|n\|`, `_1`, `it` (3.4; `it` nie jest zarezerwowane, zmienna wygrywa) | tylko runtime (`validates`, dry-types) |
| Rust | zawsze własna (`\|n\|`) | runtime, biblioteki `nutype`, `deranged`; [Flux](https://github.com/flux-rs/flux) w kompilacji: `i32{v: 0 <= v && v <= 100}` |
| TypeScript | zawsze własna (`n =>`) | runtime: Zod, Valibot, ArkType (`type("0 <= number.integer <= 100")`); typy oznaczone (brand) jako obejście |

Sowa łączy domyślną nazwę jak w Rubym (`α`), własną nazwę jak w Ruście i TS, i sprawdzanie w kompilacji jak we Fluksie.

Jeśli trzeba czegoś w TS już dziś: pod względem czytelności wygrywa ArkType, a pod względem ekosystemu Zod z `.brand()`.

## Podobne projekty

Katalog [agentlanguages.dev](https://agentlanguages.dev) (stan na 21.09.2026) śledzi 42 projekty w trzech nurtach: składniowym, weryfikacyjnym i orkiestracyjnym. Sowa należy do nurtu weryfikacyjnego. Pełne tabele: [porownanie.md](porownanie.md).

- **Vera:** obowiązkowe kontrakty, Z3 z przejściem na sprawdzanie w runtime, wywołanie LLM jako typowany efekt.
- **Thermite:** klauzule `req` / `ens` / `fx`, poziom pewności każdego zobowiązania (Verus, Lean).
- **Aver:** intencja, efekty i blok weryfikacji przy każdej funkcji, eksport do Lean 4 i Dafny.
- **Vow:** „przysięgi” sprawdzane maszynowo, skill dla Claude Code.
- **Zero** (Vercel Labs): diagnostyka w JSON ze stałymi kodami i planami naprawy.
- **Boruna:** deterministyczne wykonanie z uprawnieniami i łańcuchy dowodowe.
- **MoonBit:** najdojrzalszy z nich.
- Pojedyncze elementy: Koka (efekty), Dafny, F\* i Lean (kontrakty), Unison (kod jako baza danych), Roc i Austral (capabilities), Elixir i Gleam (procesy).

Czym Sowa może się wyróżnić (to akcenty, a nie przełom):

- stopniowane gwarancje zamiast obowiązkowych kontraktów wszędzie,
- status recenzji jako część języka, powiązany z polityką wdrożeń,
- obserwowalny runtime do diagnozowania produkcji przez agenta,
- celowo znajoma, minimalistyczna składnia.

Uczciwie: realistyczna droga może też polegać na dodaniu efektów, kontraktów i śledzenia recenzji do istniejącego stosu (linter lub reguła w CI) zamiast budowania nowego języka.

## Szkice na później

```
spec apply_discount(total: Money, pct: Percent) -> Money
  ensures result <= total
  ensures pct == 0 => result == total
  test property for all total, pct
```
Człowiek pisze `spec`, agent pisze implementację, a kompilator generuje testy property-based z `ensures`.

```
query callers(charge)
  where effects contains Net
  and not tested
```
Zapytania o strukturę programu. To samo zapytanie może działać jako reguła w CI.

```
@origin(agent: "claude", reviewed: false)
fn migrate_users()
  effects Db.write
```
Przykładowa polityka: funkcja z `Db.write` nie trafi na produkcję bez `reviewed: true`.

```
process InvoiceWorker supervised(restart: 3/min)
  state: Queue<InvoiceId> observable
```
Izolowane procesy z supervisorem. Stan `observable` jest odpytywalny na żywo.

## Otwarte pytania

- **Bloki:** wcięcia (jak w przykładach), `{ }` czy `do ... end`? W rozmowie pojawiały się wszystkie trzy.
- **Błędy:** tylko typowane warianty (`InvalidDiscount`) czy także `error("tekst")`? Przykłady używają wariantów, bo tak działa `match`.
- **Efekty:** jaka jest granulacja (`Db`, `Db.read`, `Db.write`)? Czy `Db.write` obejmuje `Db.read`? Czy można tworzyć własne efekty?
- **Weryfikacja:** co sprawdzać statycznie (solver, np. Z3), a co w runtime? Jak daleko idzie wnioskowanie po `if` (czy `if input >= 0 && input <= 100` wystarczy do `Percent(input)`)?
- **`α` w typach złożonych:** jak zapisać warunek na polu wewnątrz typu z warunkiem, np. `Order(α.items: List(len(α) > 0))`? Który `α` jest który?
- **Liczby:** czy `Money` to decimal? Jak działa dzielenie i zaokrąglanie w `total * pct / 100`?
- **Efekty a funkcje wyższego rzędu:** jak zapisać `map(items, f)`, gdy `f` ma efekty, żeby sygnatura została czytelna? (Koka: wiersze efektów, ale mało czytelne.) Zob. [ocena.md](ocena.md).
- **Granice dowodzenia:** czy ograniczyć warunki do arytmetyki liniowej? Co robić, gdy solver nie da rady (np. `total * pct / 100`)?
- **`or` z wartością domyślną:** czy `input as Percent or 0` nie połyka po cichu błędnych danych? Może dopuszczać tylko `return` i blok.
- **Komentarze:** `//` czy `--`? (Nie `#`.)
- **Rozszerzenie plików:** `.sowa`.
