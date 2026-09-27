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
- **Dokumentacja w języku, dłuższe teksty w `.md`.** `desc` to krótki tekst bez cudzysłowu (jedna linia albo blok z wcięciem), a `doc` i `why` to zawsze odnośniki do `.md`. Długie opisy i instrukcje dla użytkownika żyją w `.md`, bo tam łatwiej je pisać i czytać, a kompilator pilnuje, żeby odnośniki w obie strony nie były martwe. Droga do tego zapisu:

  | Zapis | Dlaczego nie |
  |---|---|
  | `doc "tekst"`, `doc` + blok, `doc see plik.md` | trzy formy jednego słowa, a `why see docs/...` jest długie; długie `why` w bloku przy kodzie rozprasza założenia po plikach |
  | `doc plik.md` obok `doc "tekst"` (bez `see`) | krócej, ale trzeba znać regułę „cudzysłów to tekst, bez cudzysłowu ścieżka” |
  | `why @decyzje/002.md` | krótkie, ale trzeba wiedzieć, co znaczy `@` |
  | `why [decyzja 002](decyzje/002.md)` | znajome z Markdowna, ale dłuższe |
  | `desc "tekst"` | cudzysłów nic nie dodaje, skoro po `desc` zawsze stoi tekst; do tego jedna linia to za mało |
  | **`desc tekst`, `doc plik.md`, `why plik.md`** | **wybrane**: po słowie widać, czy to tekst, czy odnośnik; `desc` na kilka linii to blok z wcięciem jak po `or`; ścieżki od katalogu `docs` z `sowa.toml` |

  `desc` w SQL-u znaczy „malejąco”, ale przy funkcji trudno to pomylić. Rozważaliśmy też `summary`.

  Nagłówek funkcji dzielimy pustymi liniami na cztery grupy: `effects`, opis (`desc`, `doc`, `why`), przykłady (`example`), kod. Bez odstępów linie `why` zlewały się z pierwszą linią kodu. Przykłady były początkowo w grupie opisu, ale to kod, a nie tekst: `example apply_discount(100, 20) == 80` pod `why decyzje/...` zlewało się z odnośnikami, a przy trzech przykładach grupa robiła się długa. Z tego samego powodu przed `fn` zawsze stoi pusta linia: `desc` typu zlewał się z następnym `fn`. Typy, także z opisem, mogą stać jeden pod drugim, bo krótkie definicje czyta się razem.

  Długość `desc` i liczbę przykładów przy kodzie ograniczają limity z `sowa.toml` (`[limits]`, z wartościami domyślnymi w języku). Po przekroczeniu `sowa check`, a nie formatter, prosi o przeniesienie do `.md`. Żeby agent nie „naprawiał” ostrzeżenia usuwaniem przykładów, `sowa check` pokazuje przy funkcji wszystkie jej testy, z kodu i z `.md`. `{Symbol}` działa też w `desc` i wiąże sekcję `.md` z symbolem w `docs.lock`.

  Jak robią to inni:
  - Rust: `#![doc = include_str!("../README.md")]` wczytuje `.md` jako dokumentację, a bloki kodu uruchamia jako doctesty; rustdoc sprawdza linki do symboli,
  - Elixir: `@doc`, `@moduledoc File.read!(...)` z `@external_resource`, doctesty `iex>`,
  - Go: funkcje `Example` jako testy i dokumentacja,
  - Eiffel, Dafny: kontrakty trafiają do dokumentacji automatycznie,
  - Unison: dokumentacja jako wartość z typowanym kodem,
  - reqlan, Aver, Prove, Pact: nazwane wymagania lub bloki `intent` przypięte do kodu,
  - Cucumber / Gherkin: specyfikacja w języku naturalnym wykonywana jako testy.

  Wykrywania nieaktualnego opisu po zmianie sygnatury (`docs.lock`) nie znaleźliśmy w takiej formie nigdzie. To może być wyróżnik Sowy.
- **Ograniczenia efektów w `sowa.toml`.** Deklaracja `effects` przy funkcji mówi, co funkcja robi, ale nie mówi, czy to w ogóle wolno. Sekcja `[effects]` w `sowa.toml` ustala politykę projektu: jakie efekty są dozwolone, w których plikach i czego wymagają (`why`, zatwierdzenie przez człowieka). Tabela „który plik ma jakie efekty” w `.md` była opisem, który mógł się rozjechać z kodem, a teraz sprawdza ją kompilator. Podobne pomysły: uprawnienia w Deno (`--allow-net`), `capabilities` w AILANG i Mog, reguły warstw w ArchUnit.

  Zatwierdzanie (`approve`) rozważaliśmy na trzech poziomach (całość w [zatwierdzanie.md](zatwierdzanie.md)):

  | Sposób | Ocena |
  |---|---|
  | sam plik `.lock` zapisywany przez `sowa review` | wygodne, ale agent może uruchomić polecenie albo dopisać wiersz sam |
  | podpis kluczem SSH | mocne, ale wymaga konfiguracji; zostaje jako opcja `[review] sign = true` |
  | **`.lock` + CODEOWNERS na `*.lock` i `sowa.toml`** | **wybrane jako domyślne**: korzysta z review PR, który zespół i tak robi, a diff pliku `.lock` jest listą kontrolną dla recenzenta |

  Hash w `effects.lock` obejmuje sygnaturę i efekty, a nie ciało: zatwierdza się „ta funkcja może łączyć się z siecią”, a nie każdą zmianę w jej kodzie. Ostrzejszy wariant to `approve = "body"`.

  Drugi poziom zatwierdzania to mapa efektów modułu (`[review.files]`, `sowa effects`): człowiek zatwierdza listę „gdzie w module powstaje jaki efekt”, a nie funkcje po kolei. Odrzuciliśmy `effects` bez wcięcia w nagłówku pliku. Byłoby widać przy czytaniu kodu, ale dublowałoby `[effects.files]` z `sowa.toml`, a tylko `sowa.toml` jest chroniony przez CODEOWNERS.
- **Domyślnie człowiek nie czyta kodu (tryb `spec`).** Agent pisze więcej, niż człowiek przeczyta, więc „człowiek czyta i zatwierdza” kończy się zatwierdzaniem bez czytania. Uczciwiej jest powiedzieć wprost, co człowiek zatwierdza: specyfikację (typy, sygnatury, efekty, opisy, przykłady). Kod leży osobno, w `impl/`, a pilnują go kompilator, testy, których agent nie może osłabić, i uprawnienia w runtime. Czytanie kodu zostaje jako opcja: wybrane pliki (`[review] read`) albo cały projekt (`mode = "code"`). Szczegóły w [tryby.md](tryby.md).

  | Wariant | Ocena |
  |---|---|
  | jeden plik, ciała funkcji zwinięte w edytorze i w PR | CODEOWNERS działa na plikach, a nie na ich fragmentach, więc nie da się chronić sygnatur bez chronienia ciał |
  | osobny `spec.lock` z hashami sygnatur | więcej mechanizmu, a to samo daje CODEOWNERS na `src/` |
  | **`src/` ze specyfikacją, `impl/` z ciałami** | **wybrane**: granica w plikach, więc działa z CODEOWNERS, `.gitattributes` i zwykłym review PR; znane z Ady (`.ads`/`.adb`) i OCamla (`.mli`/`.ml`) |

  Plik w `impl/` powtarza linię `fn` i `effects`. To dublowanie, ale bez niego plik z ciałami nie dałby się czytać bez otwierania `src/` obok. Kompilator sprawdza, czy obie linie są identyczne.

  Efekty dostały zasoby (`Net(mail)` i `[effects.resources]`), bo w nieczytanym kodzie samo `Net` znaczy „dowolny adres”.
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
- **`[effects]` w `sowa.toml`:**
  - wzorce ścieżek (`"src/db/*"`) czy tylko pojedyncze pliki?
  - więcej reguł: `rules.Net.errors = true` (wynik musi zawierać błąd, bo sieć zawodzi) wymaga odróżnienia typów błędów od zwykłych wariantów; `rules."Db.write".requires = ["Audit"]` (każdy zapis musi też logować)?
  - zatwierdzanie: pytania w [zatwierdzanie.md](zatwierdzanie.md#otwarte-pytania).
- **Weryfikacja:** co sprawdzać statycznie (solver, np. Z3), a co w runtime? Jak daleko idzie wnioskowanie po `if` (czy `if input >= 0 && input <= 100` wystarczy do `Percent(input)`)?
- **`α` w typach złożonych:** jak zapisać warunek na polu wewnątrz typu z warunkiem, np. `Order(α.items: List(len(α) > 0))`? Który `α` jest który?
- **Liczby:** czy `Money` to decimal? Jak działa dzielenie i zaokrąglanie w `total * pct / 100`?
- **Efekty a funkcje wyższego rzędu:** jak zapisać `map(items, f)`, gdy `f` ma efekty, żeby sygnatura została czytelna? (Koka: wiersze efektów, ale mało czytelne.) Zob. [ocena.md](ocena.md).
- **Granice dowodzenia:** czy ograniczyć warunki do arytmetyki liniowej? Co robić, gdy solver nie da rady (np. `total * pct / 100`)?
- **`or` z wartością domyślną:** czy `input as Percent or 0` nie połyka po cichu błędnych danych? Może dopuszczać tylko `return` i blok.
- **Wartości limitów:** czy 3 linie `desc`, 3 przykłady i 5 linii na przykład to dobre wartości domyślne? Sprawdzić na większym kodzie.
- **`docs.lock`:** jaki ostateczny format? Zatwierdzanie opisane w [zatwierdzanie.md](zatwierdzanie.md), tam też kolejne otwarte pytania.
- **Renderowanie `{Percent}`:** w jakim języku (polski, angielski)? Skąd brać tłumaczenia? Jak wyrenderować warunek z wywołaniem funkcji, np. `{Nip}` z `nip_checksum_ok(α)`? Może wtedy brać `desc` typu.
- **Z przykładowego projektu [invoices](../examples/invoices/):**
  - typy z polami: `type Line` z polami w bloku z wcięciem?
  - generyki: `List<Line>`? Jak łączą się z warunkami (`List<Line>(len(α) > 0)`)?
  - argumenty nazwane: `Line(name: "A", quantity: 1)`, czy obowiązkowe?
  - kopia z jednym zmienionym polem: `invoice with status: Paid`?
  - moduły: czy wszystkie pliki w `src/` to jedna przestrzeń nazw (jak pakiet w Go), czy potrzebne są importy?
  - transakcje: zwykła funkcja z lambdą czy osobna konstrukcja?
  - odczyt daty i czasu jako efekt `Clock`? (Podobnie `Log` w przykładzie 02.)
  - manifest projektu: `sowa.toml`, jakie pola?
  - łamanie długich sygnatur i `example` na kilka linii.
  - nagłówek pliku: hash obejmuje sygnatury całego pliku, więc ostrzeżenie o nieaktualnym opisie pojawi się przy każdej zmianie w pliku. Czy to nie za często? Może tylko zmiana efektów?
- **Tryb `spec`** (zob. [tryby.md](tryby.md#otwarte-pytania)):
  - zapis wywołań bibliotek spoza Sowy w `src/`, np. `extern fn`, i ich efektów,
  - `Secret` / `Pii` dla danych wrażliwych: typ opakowujący czy etykieta na polu?
  - zasoby dla bazy: `Db.write(invoices)`, czyli tabela jako zasób?
  - czy `impl/` musi powtarzać sygnatury, czy wystarczy sama nazwa funkcji?
- **Komentarze:** `//` czy `--`? (Nie `#`.)
- **Rozszerzenie plików:** `.sowa`.
