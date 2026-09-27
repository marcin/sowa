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
- **Warianty z danymi: pola nazwane, bez rozpakowania.** `SentToKsef(reference: KsefReference, sent_at: DateTime)` ma pola jak rekord, a w `match` nazwa wiąże cały wariant: `SentToKsef s => s.reference`. Reguły w [zalozenia.md](zalozenia.md#warianty).

  | Wariant | Ocena |
  |---|---|
  | pola pozycyjne `SentToKsef(KsefReference, DateTime)` | krótko, ale w `match` i w konstruktorze nie widać, które pole jest które |
  | rozpakowanie w `match`: `SentToKsef(reference: r, sent_at: t) =>` | druga składnia do nauczenia; zmiana pól psuje każdy `match`, nawet ten, który ich nie używa |
  | **pola nazwane, nazwa na cały wariant (`SentToKsef s => s.reference`)** | **wybrane**: ten sam dostęp do pól co w rekordach; warunek w typie działa od razu: `Invoice(α.status is SentToKsef)` |

  Nazwy wariantów są unikalne w projekcie, więc `Issued` czy `NoInvoice` wystarczy bez przedrostka typu. Koszt: dwie unie nie mogą mieć wariantu o tej samej nazwie, np. `Get` w aplikacji i we wbudowanym `Method` ([fakturownia_web](../examples/fakturownia_web/src/web.sowa)).
- **Zasoby w testach: `[resources.test]` i atrapa jako czysta funkcja.** Funkcja z `db: Db` albo `ksef: Http` musi mieć przykłady, bo inaczej najważniejsza ścieżka (formularz → zapis → wysyłka) nie ma testu. Przykład dostaje świeże zasoby z `[resources.test]`: bazę w pamięci, zatrzymany zegar, atrapę serwera. Reguły w [zalozenia.md](zalozenia.md#zasoby-w-testach-resourcestest).

  | Wariant | Ocena |
  |---|---|
  | mocki w kodzie testu (`mock(ksef).returns(...)`) | agent pisze i test, i mocka, więc może dopasować oba do kodu; mock w każdym teście osobno |
  | nagrane odpowiedzi (VCR, cassettes) | prawdziwe, ale nieczytelne dla człowieka i starzeją się; agent może nagrać odpowiedź, która pasuje do kodu |
  | **czysta funkcja `fn(HttpRequest) -> HttpResponse` w `src/`, ciało w CODEOWNERS** | **wybrane**: jedna atrapa na zasób; założenia o cudzym serwerze są w kilkunastu liniach, które człowiek czyta; zmiana atrapy to „usunięty test” w `sowa review`; nie potrzeba nowej konstrukcji |

  Koszt: czysta atrapa nie pamięta poprzednich zapytań, więc nie opisze serwera ze stanem (zob. otwarte pytania). Pierwsza próba na [fakturownia_web](../examples/fakturownia_web/): atrapa ksef.pl ma 12 linii, a założenie o ponownej wysyłce trafiło do niej i do decyzji 002.
- **Kompilacja: liczenie referencji zamiast borrow checkera.** Pytanie brzmiało: czy Sowa może działać tak szybko jak Rust, a kompilować się szybciej. Rust zawdzięcza szybkość temu, że nie ma GC, a własność (`&`, `&mut`, czasy życia) sprawdza w kompilacji. Tyle że czasy życia trafiłyby do sygnatur w `src/`, a te czyta człowiek.

  | Wariant | Ocena |
  |---|---|
  | własność i pożyczanie jak w Ruście | odrzucone: `fn totals<'a>(lines: &'a [Line])` w specyfikacji to szum, który nic nie mówi o fakturach, a agent traciłby czas na walkę z borrow checkerem |
  | GC jak w Go albo w JVM | proste, ale z pauzami i większym zużyciem pamięci; tak działa pierwszy backend (TypeScript) |
  | **liczenie referencji z modyfikacją w miejscu (Perceus w Koce, Lean 4, Roc)** | **kierunek**: wartości są niezmienne, więc nie tworzą cykli; gdy licznik wynosi 1, `invoice with status: Paid` zmienia rekord w miejscu zamiast go kopiować; sygnatury zostają bez zmian |

  Co jeszcze przyspiesza program:
  - typy z warunkami usuwają sprawdzenia w runtime: indeks typu `Int(α >= 0 && α < len(xs))` nie potrzebuje sprawdzenia granic, a `Percent` nie wymaga walidacji w każdej funkcji,
  - uprawnienia nic nie kosztują: to zwykłe parametry, bez obsługi efektów w runtime,
  - czyste funkcje kompilator może bezpiecznie przestawiać, łączyć i liczyć równolegle.

  Dlaczego kompilacja może być szybsza niż w Ruście:
  - nie ma makr, więc nie trzeba ich rozwijać przed sprawdzeniem typów,
  - sygnatury są jawne, więc każdą funkcję sprawdza się osobno i równolegle,
  - `src/` działa jak `.mli` w OCamlu: zmiana w `impl/` nie wymaga ponownego sprawdzania modułów, które z niego korzystają,
  - generyki bez pełnej monomorfizacji w pracy (kształty jak w Go, GC shape stenciling), pełne kopie tylko w buildzie produkcyjnym,
  - dwa backendy: szybki (np. Cranelift) w pracy i LLVM w buildzie produkcyjnym.

  Nowy koszt, którego Rust nie ma, to solver. Ograniczenia:
  - warunki w arytmetyce liniowej, która jest rozstrzygalna i szybka; resztę sprawdza runtime,
  - wynik solvera w cache według hasha funkcji i jej zależności,
  - limit pracy solvera w jednostkach, a nie w sekundach (jak `--resource-limit` w Dafny), żeby wynik buildu nie zależał od szybkości maszyny. Po przekroczeniu limitu warunek sprawdza runtime, a `sowa review` to pokazuje.

  Realistyczny cel: kompilacja jak w Go, a działanie blisko Swifta albo Roca. W ciasnych pętlach liczbowych Sowa będzie wolniejsza od Rusta o koszt liczników. Kolejność: najpierw backend do TypeScriptu i Deno (szybko do działania, gotowe `--allow-net`), własny backend dopiero, gdy język się ustabilizuje.
- **Kompilator najpierw w Ruście, potem w Sowie.** Kompilator potrzebuje parsera, sprawdzania typów z warunkami, solvera, backendu TS, później WASM i szybkiego backendu w pracy, a do tego LSP.

  | Język | Ocena |
  |---|---|
  | TypeScript/Deno | najszybciej do prototypu, pierwszy backend i tak generuje TS, LSP w VS Code prawie za darmo; ale trudno o kompilację jak w Go, Z3 tylko przez WASM, brak Cranelift i wasmtime |
  | OCaml | klasyczny język do kompilatorów (pierwszy kompilator Rusta, Flow, Hack), `.mli`/`.ml` to `src/`/`impl/`, oficjalne bindingi Z3; mniejszy ekosystem, WASM i Cranelift do dorobienia |
  | Go | szybka kompilacja, tą drogą idzie TypeScript 7; brak typów sumowych, więc drzewo składni to interfejsy i `switch` na typie, co źle przekłada się na Sowę |
  | Zig, Haskell | Roc przeszedł z Rusta na Ziga, Haskell dobrze nadaje się do sprawdzania typów; Zig jest jeszcze niestabilny, a Haskella zna mało osób |
  | **Rust** | **wybrane**: warianty i `match` mają ten sam kształt co w Sowie, więc przepisanie kompilatora na Sowę to prawie tłumaczenie 1:1; biblioteki z planu są w Ruście: `z3` (solver), `cranelift` (backend w pracy), `wasmtime` ([kod użytkowników](#kod-użytkowników)), `salsa` (kompilacja przyrostowa jak w rust-analyzer); Gleam przeszedł podobną drogę: kompilator w Ruście generuje JS |

  Koszt: wolniejsze pisanie na początku i borrow checker.

  Droga do kompilatora w Sowie:

  1. Etap 0 w Ruście: parser, sprawdzanie uprawnień, `sowa review`, backend TS.
  2. Kompilator w Sowie jako zwykły projekt Sowy (`src/`, `impl/`, `sowa.toml`). To najlepszy test języka: kompilator dostaje tylko `files: Files`, więc z sygnatury widać, że niczego nie wysyła do sieci, a jego zmiany recenzuje `sowa review`.
  3. Bootstrap: etap 0 kompiluje kompilator w Sowie (etap 1), etap 1 kompiluje sam siebie (etap 2), a etapy 1 i 2 muszą dać identyczny wynik.
  4. Kompilator w Ruście zostaje wzorcem, dopóki wersja w Sowie nie przejdzie wszystkich testów, a potem się go zamraża. Go zrobił to w wersji 1.5, przechodząc z C na Go.

  Przed przepisaniem Sowa potrzebuje generyków, słowników (`Map`), modułów, `Files` i liczenia referencji z modyfikacją w miejscu. Bez tego ostatniego każda zmiana w drzewie składni kopiuje całość.

  Nie przepisywać za wcześnie: przy każdej zmianie składni trzeba wtedy poprawiać dwa kompilatory i łańcuch bootstrapu. Rust przepisał kompilator na siebie po około 5 latach, Go po 3. Dobry moment to chwila, gdy specyfikacja przestanie się zmieniać co tydzień.
- **Ruby: składnia tak, semantyka nie.** Z Ruby'ego warto wziąć brak średników i lekkość zapisu. Nie bierzemy monkey-patchingu, `method_missing`, metaprogramowania ani DSL-i, w których nie wiadomo, skąd bierze się metoda. Tą drogą poszły już Elixir (składnia z Ruby'ego, semantyka z Erlanga) i Crystal.
- **Jawne zamiast skrótów:**
  - `unless x` → `if not x`
  - `users.map(&:email)` → `users.map(u => u.email)`
  - `cache ||= load()` → jawny `if`
  - `user&.email` → `match` albo typ bez `nil`
  - `3.days.ago` → `clock.now() - days(3)`

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

## Aplikacja webowa

Próba: czy w Sowie da się napisać prostą aplikację webową. Szkic leży w przykładzie [invoices](../examples/invoices/): `src/web.sowa`, `impl/web.sowa` i `main` z `web: Server`. Aplikacja ma formularz nowej faktury, stronę faktury i przycisk wysyłki e-mailem. Druga, pełniejsza próba to [fakturownia_web](../examples/fakturownia_web/): wystawienie, zapis w bazie i wysyłka do ksef.pl, napisane tak, jakby pisał je agent, z [wynikiem `sowa review`](../examples/fakturownia_web/PR.md) dla człowieka.

Wniosek: da się, a uprawnienia pasują do weba lepiej niż do reszty przykładu. Brakuje jednak kilku konstrukcji, bez których aplikacji nie da się napisać, i części biblioteki standardowej.

Co pasuje:

- **Zapytanie i odpowiedź to zwykłe wartości.** `handle(req, db, clock, mail) -> Response` to zwykła funkcja, a routing (`route(method, path) -> Route`) jest czysty i ma przykłady w `src/`. Serwer tylko zamienia HTTP na `Request` i `Response` na HTTP.
- **Uprawnienia per adres.** Komplet ma tylko `handle`. Strona faktury (`get_invoice`) dostaje samo `DbRead`, a zapis formularza (`post_invoice`) nie dostaje `Mailer`. Recenzent widzi w `src/`, że wyświetlenie faktury niczego nie zapisze ani nie wyśle. W typowym frameworku każdy handler ma dostęp do wszystkiego.
- **Bez nowych reguł w `main`.** `web.serve(req => handle(req, db, clock, mail))` korzysta z reguły o lambdach: `serve` może zrobić tylko to, co przekazana funkcja.
- **Błędy na odpowiedzi przez `match`.** Nowy wariant w `IssueError` nie skompiluje się, dopóki nie dostanie komunikatu w `error_message`. Założenie „komunikaty powstają w interfejsie” ma teraz konkretne miejsce.
- **Równoległe zapytania bez wyścigów w pamięci.** Wartości są niezmienne i nie ma zmiennych globalnych, więc jedyny wspólny stan to baza. Numerację chroni transakcja.
- **`async` z sygnatury (propozycja).** W backendzie TS funkcja z uprawnieniem kompiluje się do `async`, a czysta do zwykłej. Nikt nie pisze `await`, a „kolorowanie funkcji” wynika z tego, co już stoi w sygnaturze.
- **`property` jako test bezpieczeństwa.** `not contains(to_string(form_page(form, messages)), "<script")` sprawdza escapowanie na dowolnych danych z formularza.

Czego brakuje:

| Luka | W szkicu | Uwagi |
|---|---|---|
| ~~warianty z danymi~~ | `ShowInvoice(number: InvoiceNumber)`, `Ok(body: Html)` | **w specyfikacji** ([Warianty](zalozenia.md#warianty)); razem z błędami z danymi, np. `InvalidLine(position: 2, field: LineQuantity)` |
| `match` na kilku wartościach i na liście | `match method, segments(path)` z `["invoices", year, seq]` | alternatywa to łańcuch `if`, czytelny gorzej |
| `_` w `match` | `_ => return UnknownPath` | kłóci się z regułą, że nowy wariant psuje niepełny `match`; propozycja: `_` tylko dla typów bez skończonej listy wariantów (`String`, `List`, `Int`), nigdy dla unii |
| ~~blok po `=>`~~ | dwie linie w gałęzi `route` | **w specyfikacji** ([Warianty](zalozenia.md#warianty)) |
| odczyt formularza i JSON | `req.body as InvoiceForm or ...` | `as` z tekstu na rekord, dekoder generuje kompilator z typu (bez makr, jak `derive` w Ruście); `as` mówi tylko „nie pasuje”, a formularz potrzebuje błędu przy polu, dlatego `InvoiceForm` trzyma tekst, a sprawdza go `read_buyer`; lista pozycji w formularzu HTML to nazwy `lines[0].name` albo JSON |
| HTML | `html"..."` z `{...}` | wstawiony tekst escapowany według miejsca (treść, atrybut, adres), jak w `html/template` w Go; zwykłe napisy nie mają interpolacji, a literały wieloliniowe nie mają reguł wcięć |
| uprawnienie `Server` | `web = { type = "Server", listen = "0.0.0.0:8080" }` | nowy wbudowany typ; przy Deno to kolejny adres w `--allow-net` |
| ~~testy funkcji z uprawnieniami~~ | `post_invoice` bez `example` | **w specyfikacji** ([`[resources.test]`](zalozenia.md#zasoby-w-testach-resourcestest)); w fakturownia_web cały przepływ przez `handle` jest testem w instrukcji |
| zapytania do bazy | `find_invoice` bez ciała | typowane i zawsze parametryzowane (bez SQL injection), wiersz zamieniany na rekord z warunkami |
| sesje, logowanie, CSRF | nie ma | dziś każdy może wystawić fakturę; potrzeba ciasteczek w `Request` i `Response`, `Random` na tokeny i hashowania haseł (biblioteka spoza Sowy, `extern fn`) |
| `as` na unii | `parse_int(year) as Int(α >= 2000)` | `as` zawęża `Int \| NotANumber` do `Int` z warunkiem |

Przy okazji wyszedł słaby typ. Naturalna `property route(Get, invoice_path(number)) == ShowInvoice(number: number)` nie przejdzie, bo `InvoiceNumber` wymaga tylko prefiksu „FV/”: generator poda np. `"FV/x"`, a `route` zwróci `UnknownPath`. Typ powinien opisywać cały format, np. `matches(α, "FV/[0-9]{4}/[0-9]{4,}")`. W invoices tego nie poprawiono, bo to zmiana w numeracji, której kod czyta człowiek. W fakturownia_web typ opisuje już cały format i `property` przechodzi. To dobry argument za `property`: jedna linia pokazała lukę w specyfikacji, której nie wychwycił żaden przykład.

Poza próbą zostały: wydajność serwera, strumieniowanie, WebSockety i przesyłanie plików.

Warianty z danymi, blok po `=>` i `[resources.test]` są już w specyfikacji. Następny krok: `match` na listach i `_`, a potem biblioteka standardowa (`html"..."`, odczyt formularza i JSON, API bazy, `Server`, `Http`).

## Kod użytkowników

Pomysł na później: klient aplikacji pisze w Sowie własną regułę, np. rabat, a serwer ją kompiluje i uruchamia albo kompiluje do WASM. Pytanie: czy łatwo dodać to jako opcję kompilatora?

Wniosek: dostęp do bazy, sieci i plików Sowa blokuje prawie za darmo, bo uprawnień nie da się utworzyć w kodzie. Pętli bez końca, zużycia pamięci i błędu w samym kompilatorze typy nie powstrzymają. To wymaga backendu WASM z limitami.

### Tryb `--sandbox`

```
sowa build --sandbox --api host/src --target wasm user/impl
```

Gospodarz daje `src/`, a użytkownik pisze `impl/`. Podział na specyfikację i kod jest więc gotowym interfejsem wtyczek:

```
fn discount_rule(order: Order) -> Percent
  desc Rabat dla zamówienia, reguła klienta.

  example discount_rule(Order(items: [], total: 0)) == 0
```

Kompilator w tym trybie:

- sprawdza, że `impl/` ma dokładnie te sygnatury co `src/` gospodarza, jak przy zwykłym projekcie,
- nie przyjmuje `main` ani `[resources]`, więc uprawnienia przychodzą tylko od gospodarza, przez parametry, np. `log: Log`,
- nie przyjmuje `extern fn`, bo kod spoza Sowy to jedyna droga obok uprawnień,
- sprawdza warunki wyniku w runtime tam, gdzie ich nie udowodni: użytkownik nie zwróci rabatu 150%, bo wynik to `Percent`,
- uruchamia przykłady gospodarza jako testy kodu użytkownika.

Warunek po stronie języka: biblioteka standardowa nie ma żadnej globalnej funkcji z efektem. Czas daje tylko `clock: Clock`, losowość tylko `random: Random`.

Po stronie kompilatora to kilka dni pracy, gdy działa już sprawdzanie uprawnień.

### Czego typy nie dadzą

| Zagrożenie | Co pomaga | Uwagi |
|---|---|---|
| pętla bez końca, głęboka rekursja | limit pracy (fuel) i czasu | pętla bez końca też jest czysta; w wasmtime to konfiguracja (fuel, epoch interruption) |
| zużycie pamięci | limit pamięci instancji WASM | w backendzie TS tylko przez osobny proces albo worker |
| kompilator jako cel ataku | limit pracy solvera w jednostkach (już w planie), limity dla parsera i sprawdzania typów | głębokie zagnieżdżenia i duże generyki mogą zablokować kompilację |
| błąd w sprawdzaniu uprawnień | WASM bez importów albo tylko z importami od gospodarza | kod fizycznie nie wykona wejścia-wyjścia, nawet przy błędzie kompilatora |
| wyciek danych w wyniku | gospodarz przekazuje tylko dane, które użytkownik może zobaczyć | czysta funkcja nadal może zwrócić to, co dostała |
| nadużycie uprawnień od gospodarza | limity po stronie gospodarza, np. liczba wpisów w `log` | typ mówi, co kod może, a nie ile razy |

Właściwą granicą jest WASM bez importów. Typy są drugą warstwą: użytkownik dostaje czytelny błąd od razu, np. „ta funkcja nie ma `Http`”, zamiast awarii przy uruchomieniu.

### Backend

| Wariant | Ocena |
|---|---|
| backend TS w tym samym procesie co gospodarz | odrzucone: obcy kod w V8 w jednym procesie to nie piaskownica |
| backend TS w osobnym workerze Deno bez uprawnień, z timeoutem | wystarczy na reguły pisane przez własnych klientów; za słabe przy wrogim kodzie od wielu klientów naraz |
| **WASM bez importów, z limitem fuel i pamięci** | **kierunek**: granica niezależna od kompilatora; działa na serwerze i w przeglądarce |

Backend TS ma powstać pierwszy, więc przez jakiś czas tryb `--sandbox` da tylko sprawdzenie w kompilatorze i worker. Pełna gwarancja przyjdzie z backendem WASM.

Dodatkowa zaleta: kod bez `Clock` i `Random` jest deterministyczny. Wynik można cache'ować, a zgłoszony błąd odtworzyć na tych samych danych.

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
- **`[resources]` w `sowa.toml`:** jak podać sekrety, np. hasło do bazy albo token do ksef.pl? W fakturownia_web jest szkic `token_env = "KSEF_TOKEN"`. Czy runtime ma ukrywać sekret przed kodem (kod dostaje `Http` z tokenem, ale nie widzi tokenu)?
- **`[resources.test]`:**
  - schemat bazy w pamięci: skąd wiadomo, jakie są tabele, i czy te same migracje co na produkcji?
  - serwery ze stanem, np. ksef.pl, które za drugim razem odpowiadają inaczej: atrapa jako `fn(HttpRequest, state) -> (HttpResponse, state)`?
  - czy przykład może zmienić zegar w trakcie, np. „po 30 dniach faktura jest przeterminowana”?
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
- **Kompilacja:** kiedy własny backend zamiast TypeScriptu? Czy liczenie referencji wystarczy przy bibliotekach spoza Sowy, które mogą tworzyć cykle? Jaki domyślny limit pracy solvera? Po czym poznać, że język jest dość stabilny, żeby przepisać kompilator na Sowę? Czy etap 0 w Ruście utrzymywać dalej, np. do bootstrapu na nowej platformie?
- **Aplikacja webowa:** luki z [próby](#aplikacja-webowa): `match` na listach i `_`, odczyt formularza i JSON, `html"..."`, `Server`, typy `HttpRequest` i `HttpResponse`, zapytania do bazy, sesje.
- **Kod użytkowników:** czy kompilacja cudzego kodu Sowy na serwerze albo do WASM to osobny tryb kompilatora (bez `main`, bez `[resources]`, z limitami)? Czy pierwszy backend (TS) wystarczy do czasu WASM? Jakie domyślne limity fuel i pamięci? Czy użytkownik widzi błędy kompilatora po polsku? Zob. [Kod użytkowników](#kod-użytkowników).
- **Z przykładu [ttfx_decrypt](../examples/ttfx_decrypt/)** (propozycje, kompilator już je obsługuje; szczegóły w [README](../examples/ttfx_decrypt/README.md#propozycje-do-decyzji)):
  - `while warunek` z blokiem: przyjąć, czy wystarczy `for` po zakresie i rekurencja?
  - `Random` z `int(min, max)` i `choice(lista)`, ziarno z `seed` albo `seed_env`: czy bez ziarna losować z zegara?
  - `Terminal` z `read()`, `write(text)`, `exit(code)`: czy kod wyjścia ma raczej wynikać z wyniku `main`?
  - wbudowane `at`, `join`, `split`, `chars`, `char`: nazwy i czy `at` poza listą to błąd programu, czy `Option`?
  - pauza (`clock.sleep(ms)`?), żeby tempo animacji ustawiać w Sowie.
- **Komentarze:** `//` czy `--`? (Nie `#`.)
- **Rozszerzenie plików:** `.sowa`.
