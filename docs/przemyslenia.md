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
- **Siła gwarancji a koszt.** Pełna weryfikacja formalna jest za droga dla typowego SaaS-a. Gwarancje są więc stopniowane: domyślnie typy i uprawnienia, opcjonalnie warunki wyniku i `property`, a dowody tylko w krytycznych modułach.

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

  Nagłówek funkcji dzielą puste linie na trzy grupy: opis (`desc`, `doc`, `why`), przykłady (`example`, `property`), kod. Wcześniej była też czwarta grupa, `effects`, ale uprawnienia przeszły do parametrów (zob. niżej). Bez odstępów linie `why` zlewały się z pierwszą linią kodu. Przykłady były początkowo w grupie opisu, ale to kod, a nie tekst: `example apply_discount(100, 20) == 80` pod `why decyzje/...` zlewało się z odnośnikami, a przy trzech przykładach grupa robiła się długa. Z tego samego powodu przed `fn` zawsze stoi pusta linia: `desc` typu zlewał się z następnym `fn`. Typy, także z opisem, mogą stać jeden pod drugim, bo krótkie definicje czyta się razem.

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
- **Uprawnienia jako parametry zamiast `effects`.** Funkcja, która wysyła e-mail, dostaje `mail: Mailer`, a funkcja, która czyta z bazy, `db: DbRead`. Uprawnienia nie da się utworzyć w kodzie, zwrócić z funkcji ani zapisać w polu rekordu, więc płynie tylko przez parametry od `main`, który dostaje je od runtime według `[resources]` w `sowa.toml`. Funkcja bez uprawnień i bez parametrów-funkcji jest czysta.

  | Wariant | Ocena |
  |---|---|
  | `effects Db.write, Net` przy funkcji i `[effects]` w `sowa.toml` (poprzednia wersja) | dwa miejsca do utrzymania; mapa „który plik ma jakie efekty” to osobna konfiguracja; funkcje wyższego rzędu wymagają wierszy efektów jak w Koce |
  | `effects Net(mail)` z zasobem | adres jest w sygnaturze, ale nadal jest globalna funkcja `mail_send(mail, ...)` i osobna polityka plików |
  | **uprawnienie jako parametr (`mail: Mailer`)** | **wybrane**: to zwykły parametr, więc nie ma nowej składni; czystość modułu widać z sygnatur w `src/`, bez konfiguracji; lambda przechwytuje uprawnienie, więc `each` i `map` nie potrzebują niczego; znane z object capabilities (E, Pony, Austral, WASI) |

  Koszt: dłuższe sygnatury i przekazywanie `db` przez kilka warstw. Formatter stawia uprawnienia na końcu listy parametrów, żeby było je łatwo znaleźć. Uprawnień w rekordach nie ma celowo: struktura „wszystkie zależności” zamieniłaby każdą funkcję, która ją dostaje, w funkcję, która może wszystko.

  Odrzucona została też lista modułów z uprawnieniami w `sowa.toml` (`[resources.files]`). Dublowałaby to, co i tak widać w sygnaturach w `src/`, a te są chronione przez CODEOWNERS.
- **Zatwierdzanie bez plików z zatwierdzeniami.** Wcześniej funkcje z `Net` albo `Db.write` trafiały do `effects.lock` (`approve = true`), a wybrane moduły do mapy efektów (`[review.files]`, `sowa effects`). Całość w [zatwierdzanie.md](zatwierdzanie.md).

  | Sposób | Ocena |
  |---|---|
  | plik `.lock` zapisywany przez `sowa review` | agent może uruchomić polecenie albo dopisać wiersz sam |
  | `effects.lock` z zatwierdzaniem funkcji po kolei | drugi mechanizm obok review PR; po tygodniu klika się bez czytania |
  | **review PR, `sowa review` w komentarzu i check, że po zatwierdzeniu specyfikacja się nie zmieniła** | **wybrane**: korzysta z review, które zespół i tak robi; jedyny plik z zatwierdzeniami to `docs.lock`, bo tekstu nie da się sprawdzić inaczej |
  | podpisany pusty commit | zostaje jako opcja przy pracy na jednym koncie (`[review] sign = true`) |
- **Zatwierdzenie na końcu, a nie specyfikacja przed kodem.** Człowiek zatwierdza PR raz, gdy całość działa, a po zatwierdzeniu `sowa check --ci` nie przepuszcza zmian w plikach z właścicielem w CODEOWNERS.

  | Wariant | Ocena |
  |---|---|
  | dwa PR-y: najpierw specyfikacja, potem kod | `main` przez chwilę ma specyfikację bez kodu; dwa review |
  | specyfikacja zatwierdzana w szkicu PR, zanim powstanie kod | człowiek czyta pomysł, który w trakcie implementacji i tak się zmieni, więc zatwierdza dwa razy, a za pierwszym razem na próżno |
  | „Dismiss stale approvals” włączone | każda poprawka w `impl/` kasuje zatwierdzenie, więc człowiek zatwierdza kod, którego nie czyta |
  | **zatwierdzenie na końcu i check na zmiany specyfikacji po nim** | **wybrane**: człowiek widzi działający program i zatwierdza wersję końcową |

  Pułapka zatwierdzania na końcu: specyfikacja jest już dopasowana do kodu, a agent mógł poluzować warunek, żeby test przeszedł. Rozbraja ją `sowa review --base main`: pokazuje zmianę netto względem `main`, a osłabienia i usunięte testy stawia na górze, z kontrprzykładem.
- **Drugi agent jako recenzent.** Jeden agent pisze, drugi, „pewniejszy” (inny model, bez prawa zapisu), sprawdza zmianę, zanim zobaczy ją człowiek. Szczegóły w [zatwierdzanie.md](zatwierdzanie.md#drugi-agent-jako-recenzent).

  | Wariant | Ocena |
  |---|---|
  | agent-recenzent zamiast człowieka | odrzucone: modele mają podobne ślepe plamy, więc błędy się nakładają; agent piszący może w PR przekonać recenzenta; recenzent nie wie, czego człowiek chciał, poza treścią zadania |
  | **agent-recenzent jako filtr** | **wybrane jako opcja**: sam zatwierdza zmiany z kategorii „rozszerzenie” i „zwykłe”, a przy uprawnieniu, osłabieniu i usuniętym teście pisze uwagi i czeka na człowieka |

  Dzięki stałym kategoriom w `sowa review` granicę między „agent może” a „tylko człowiek” wyznacza kompilator, a nie ocena recenzenta.
- **`sowa review` bez konfiguracji.** Kategorie mają stałą kolejność: uprawnienie, osłabienie, usunięty test, rozszerzenie, zwykłe. Wcześniej ryzyko ustawiało się w `sowa.toml` (`[effects.rules]`, `approve`), ale nowe uprawnienie zawsze znaczy „kod może zrobić coś, czego wcześniej nie mógł”, więc nie ma czego ustawiać. Osłabienie rozpoznaje solver, sprawdzając, czy stary warunek wynika z nowego. Gdy nie umie rozstrzygnąć, zmiana trafia do osłabień: fałszywy alarm kosztuje mniej niż przepuszczone osłabienie. Podobnie działają buf breaking i cargo-semver-checks, ale z listą reguł zamiast solvera.
- **Warunek wyniku i `property` zamiast `ensures`.**

  | Wariant | Ocena |
  |---|---|
  | `ensures result <= total` pod sygnaturą | nowe słowo i nowa nazwa `result`; warunek oderwany od typu wyniku |
  | **`-> Money(α <= total)`** | **wybrane**: ta sama składnia co warunki w typach i na parametrach, `α` znaczy „wynik” |
  | `test property for all total, pct` | osobna konstrukcja z kwantyfikatorem |
  | **`property apply_discount(total, 0) == total`** | **wybrane**: wygląda jak `example`, tylko nazwy parametrów znaczą „dowolna wartość tego typu”; dane generują się z warunków na typach |

  Warunek wyniku, którego solver nie udowodni, nie jest błędem kompilacji. Zamienia się w sprawdzenie w runtime i w wygenerowane testy, a `sowa review` pokazuje, które warunki są udowodnione, a które tylko sprawdzane. Niespełniony warunek w runtime to błąd programu, a nie wariant wyniku.
- **Człowiek nie czyta kodu.** Agent pisze więcej, niż człowiek przeczyta, więc „człowiek czyta i zatwierdza” kończy się zatwierdzaniem bez czytania. Uczciwiej jest powiedzieć wprost, co człowiek zatwierdza: specyfikację (typy, sygnatury z uprawnieniami, opisy, przykłady). Kod leży osobno, w `impl/`, a pilnują go kompilator, testy, których agent nie może osłabić, i uprawnienia. Szczegóły w [specyfikacja.md](specyfikacja.md).

  | Wariant | Ocena |
  |---|---|
  | jeden plik, ciała funkcji zwinięte w edytorze i w PR | CODEOWNERS działa na plikach, a nie na ich fragmentach, więc nie da się chronić sygnatur bez chronienia ciał |
  | osobny `spec.lock` z hashami sygnatur | więcej mechanizmu, a to samo daje CODEOWNERS na `src/` |
  | **`src/` ze specyfikacją, `impl/` z ciałami** | **wybrane**: granica w plikach, więc działa z CODEOWNERS, `.gitattributes` i zwykłym review PR; znane z Ady (`.ads`/`.adb`) i OCamla (`.mli`/`.ml`) |
  | tryby w `sowa.toml` (`mode = "spec"` albo `"code"`, `[review] read`) | odrzucone: dwa sposoby pracy to dwa razy więcej reguł; czytanie wybranego pliku to wpis w CODEOWNERS i `.gitattributes`, a `sowa.toml` nie musi o tym wiedzieć |

  Plik w `impl/` powtarza linię `fn`. To dublowanie, ale bez niego plik z ciałami nie dałby się czytać bez otwierania `src/` obok. Kompilator sprawdza, czy obie linie są identyczne.

  Uprawnienia do sieci mają zasób (`Mailer` do jednego serwera, `Http` do jednego adresu), bo w nieczytanym kodzie samo „sieć” znaczy „dowolny adres”.
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
query callers(charge)
  where takes Http
  and not tested
```
Zapytania o strukturę programu. To samo zapytanie może działać jako reguła w CI.

```
@origin(agent: "claude", reviewed: false)
fn migrate_users(db: Db)
```
Przykładowa polityka: funkcja z `Db` nie trafi na produkcję bez `reviewed: true`.

```
process InvoiceWorker supervised(restart: 3/min)
  state: Queue<InvoiceId> observable
```
Izolowane procesy z supervisorem. Stan `observable` jest odpytywalny na żywo.

## Otwarte pytania

- **Bloki:** wcięcia (jak w przykładach), `{ }` czy `do ... end`? W rozmowie pojawiały się wszystkie trzy.
- **Błędy:** tylko typowane warianty (`InvalidDiscount`) czy także `error("tekst")`? Przykłady używają wariantów, bo tak działa `match`.
- **Uprawnienia:** czy wbudowane (`Db`, `DbRead`, `Clock`, `Random`, `Log`, `Mailer`, `Http`, `Files`) wystarczą? Czy można tworzyć własne, np. zawężając `Http` do jednej ścieżki? Pełna lista operacji (`clock.today()`, `log.write(...)`, `db.transaction(...)`)? Kolejne pytania w [specyfikacja.md](specyfikacja.md#otwarte-pytania).
- **`[resources]` w `sowa.toml`:** jak podać sekrety, np. hasło do bazy (zmienne środowiskowe)? Inne zasoby w testach i na produkcji?
- **Zatwierdzanie:** pytania w [zatwierdzanie.md](zatwierdzanie.md#otwarte-pytania).
- **Weryfikacja:** co sprawdzać statycznie (solver, np. Z3), a co w runtime? Jak daleko idzie wnioskowanie po `if` (czy `if input >= 0 && input <= 100` wystarczy do `Percent(input)`)?
- **`α` w typach złożonych:** jak zapisać warunek na polu wewnątrz typu z warunkiem, np. `Order(α.items: List(len(α) > 0))`? Który `α` jest który?
- **Liczby:** czy `Money` to decimal? Jak działa dzielenie i zaokrąglanie w `total * pct / 100`?
- **Uprawnienia w długich łańcuchach wywołań:** czy przekazywanie `db` przez kilka warstw nie zaśmieci sygnatur na tyle, że agent zacznie dawać `Db` wszędzie „na zapas”? Może ostrzeżenie o nieużywanym uprawnieniu.
- **Warunki wyniku sprawdzane w runtime:** czy niespełniony warunek na produkcji ma przerywać program, czy tylko logować?
- **`property` dla złożonych typów:** jak generować dane dla rekordów z warunkami na polach, np. `Invoice(α.status == Issued)`?
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
  - transakcje: operacja na `Db` z lambdą (`db.transaction(tx => ...)`) czy osobna konstrukcja?
  - manifest projektu: `sowa.toml`, jakie pola?
  - łamanie długich sygnatur i `example` na kilka linii.
  - nagłówek pliku: hash obejmuje sygnatury całego pliku, więc ostrzeżenie o nieaktualnym opisie pojawi się przy każdej zmianie w pliku. Czy to nie za często? Może tylko zmiana uprawnień?
- **Specyfikacja i kod** (zob. [specyfikacja.md](specyfikacja.md#otwarte-pytania)):
  - zapis wywołań bibliotek spoza Sowy w `src/`, np. `extern fn`, i jakie uprawnienia dostają,
  - `Secret` / `Pii` dla danych wrażliwych: typ opakowujący czy etykieta na polu?
  - zasoby dla bazy: `Db` zawężony do jednej tabeli?
  - czy `impl/` musi powtarzać sygnatury, czy wystarczy sama nazwa funkcji?
- **Komentarze:** `//` czy `--`? (Nie `#`.)
- **Rozszerzenie plików:** `.sowa`.
