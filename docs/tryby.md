# Tryby: zatwierdzanie specyfikacji albo czytanie kodu

Agent pisze szybciej, niż człowiek czyta. Przy dużych zmianach „człowiek czyta i zatwierdza” zamienia się w zatwierdzanie bez czytania. Dlatego Sowa ma dwa tryby:

| | `spec` (domyślny) | `code` |
|---|---|---|
| co czyta człowiek | specyfikację: typy, sygnatury, efekty, `desc`, `doc`, `why`, przykłady, dokumentację | wszystko, razem z ciałami funkcji |
| układ plików | `src/` ze specyfikacją, `impl/` z ciałami funkcji | `src/`, funkcja z ciałem w jednym pliku |
| co chroni CODEOWNERS | `src/`, `docs/`, `sowa.toml`, `*.lock` | `sowa.toml`, `*.lock` |
| co pilnuje ciał funkcji | kompilator, testy i izolacja | człowiek, z pomocą `approve`, mapy efektów i `effects.lock` |

Tryb ustawia się w `sowa.toml`:

```toml
[review]
mode = "spec"                    # domyślnie; albo "code"
read = ["impl/numbering.sowa"]   # w trybie spec: moduły, których kod człowiek i tak czyta
```

Język jest w obu trybach ten sam. Różni się tylko to, gdzie leżą ciała funkcji i co wymaga zgody człowieka.

## Tryb `spec`: człowiek nie czyta kodu

### Układ plików

Plik w `src/` to specyfikacja modułu. Są w nim typy, sygnatury z efektami, dokumentacja i przykłady, a nie ma ciał funkcji:

```
// src/invoice.sowa
fn totals(lines: List<Line>(len(α) > 0), discount: Percent) -> Totals
  desc Sumy netto, VAT i brutto dla całej faktury.
  doc uzytkownik/faktury.md#sumy-i-zaokrąglenia
  why decyzje/001-vat-od-sumy-w-stawce.md

  example totals([Line(name: "Usługa", quantity: 1, unit_net: 100, vat: Vat23)], 0)
    == Totals(net: 100, vat: 23, gross: 123)
```

Plik o tej samej nazwie w `impl/` zawiera ciała. Powtarza linie `fn` i `effects`, żeby dało się go czytać bez skakania między plikami, ale nie powtarza dokumentacji:

```
// impl/invoice.sowa
fn totals(lines: List<Line>(len(α) > 0), discount: Percent) -> Totals
  var net = 0
  var vat = 0
  for rate in distinct(lines.map(l => l.vat))
    ...
  return Totals(net: net, vat: vat, gross: net + vat)
```

Reguły:

- Sygnatura i `effects` w `impl/` muszą być identyczne jak w `src/`. Zmiana sygnatury w samym `impl/` to błąd kompilacji, więc agent musi zmienić też `src/`, a to wymaga zgody człowieka.
- `impl/` może mieć własne funkcje pomocnicze i typy. Są prywatne: spoza modułu widać tylko to, co jest w `src/`.
- Funkcja pomocnicza może mieć efekty, ale tylko takie, jakie mają funkcje z `src/`, które ją wywołują. Kompilator i tak to sprawdza, bo efekt musi być zadeklarowany w każdej funkcji po drodze.
- `example` w `impl/` to własne testy agenta. Uruchamiają się, ale nie są specyfikacją i nie wymagają zgody.

### Co zatwierdza człowiek

Tylko to, co leży w `src/`, `docs/`, `sowa.toml` i plikach `*.lock`, czyli wszystko poza ciałami funkcji:

- typy i warunki na nich,
- sygnatury i efekty, razem z zasobami, np. `Net(mail)`,
- `desc`, `doc`, `why`,
- przykłady w `src/` i bloki `sowa` w `.md`, czyli testy, które agent musi spełnić,
- politykę w `sowa.toml`.

`sowa review` pokazuje zmiany w specyfikacji uporządkowane od najbardziej ryzykownych, a nie w kolejności plików:

```
$ sowa review

Zmiany w specyfikacji (src/, docs/):

  [1] niebezpieczne   send_reminder  src/sending.sowa:24   nowa funkcja, effects Net(mail)
  [2] osłabienie      Percent        src/types.sowa:5      warunek: α <= 100  →  α <= 1000
  [3] usunięty test   totals         src/invoice.sowa:45   example totals([...], 0) == Totals(...)
  [4] zwykłe          line_net       src/invoice.sowa:38   nowy example

Kod (impl/): 3 pliki, 120 linii, nie wymaga przeglądu.
  testy: 14 przykładów, 6 z danych generowanych z typów
  mutacje wykryte przez testy: 41 z 44
```

Za niebezpieczne `sowa review` uznaje efekty z regułą `approve` w `[effects.rules]` i każdy nowy zasób. Za osłabienie uznaje luźniejszy warunek, usunięty albo zmieniony przykład i nowy wariant błędu. Pozostałe zmiany są „zwykłe”.

### Izolacja kodu

Kod w `impl/` jest odizolowany na trzech poziomach:

1. **W repozytorium.** `impl/` nie ma właściciela w CODEOWNERS, więc zmiany w nim nie wymagają zgody człowieka. W PR jest domyślnie zwinięty (`linguist-generated` w `.gitattributes`), żeby recenzent widział tylko specyfikację.
2. **W kompilatorze.** Kod w `impl/` nie wyjdzie poza specyfikację: nie doda efektu, zasobu ani wariantu błędu, nie osłabi warunku i nie wywoła kodu spoza Sowy. Wywołania bibliotek spoza Sowy (np. z npm) mogą stać tylko w `src/`, więc każda taka furtka jest zatwierdzana.
3. **W runtime.** `sowa run` uruchamia skompilowany program z uprawnieniami wziętymi z `sowa.toml`. Przy kompilacji do TypeScriptu i uruchamianiu w Deno: `--allow-net=smtp.firma.pl:587` i nic więcej. To druga linia obrony, na wypadek błędu w kompilatorze.

### Co blokuje zagrożenia w nieczytanym kodzie

| Zagrożenie | Co to blokuje |
|---|---|
| kod robi coś poza liczeniem | efekty w sygnaturach w `src/` |
| kod wysyła dane nie tam, gdzie trzeba | efekty z zasobem: `Net(mail)`, adresy tylko z `sowa.toml` |
| wyciek danych wrażliwych | typ `Secret` / `Pii`, którego nie da się wysłać ani zalogować poza funkcjami z `src/` (do ustalenia) |
| furtka: kod spoza Sowy, `eval`, refleksja | brak w języku; wywołania bibliotek spoza Sowy tylko w `src/` |
| nowa biblioteka | zależności w `sowa.toml`, pod CODEOWNERS |
| operacje nieodwracalne | osobne efekty, np. `Db.delete`, z `approve` |
| kod liczy źle | typy z warunkami i przykłady w `src/`, których agent nie może osłabić; dane generowane z typów i mutacje |

### Efekty z zasobem

`effects Net` pozwala wysłać cokolwiek dokądkolwiek. W trybie `spec` efekt przypina się do zasobu nazwanego w `sowa.toml`:

```
fn send_invoice(invoice: Invoice) -> Sent | SendError
  effects Net(mail)
```

```toml
[effects.resources]
mail = "smtp.firma.pl:587"
```

Adres pochodzi z `sowa.toml`, a nie z tekstu w kodzie. Funkcje sieciowe przyjmują zasób zamiast adresu, np. `mail_send(mail, to, subject, pdf)`. `http_post("https://gdzies.com", invoice)` w `impl/` się nie skompiluje: adres jako tekst nie jest zasobem, a `send_invoice` ma tylko `Net(mail)`. Człowiek zatwierdza raz: „faktury wolno wysyłać przez smtp.firma.pl”.

### Specyfikacja, której agent nie osłabi

Jeśli agent pisze i kod, i testy, testy niczego nie dowodzą: agent może poprawić test, żeby przeszedł. W trybie `spec` przykłady w `src/` i bloki `sowa` w `.md` są częścią specyfikacji, więc ich zmiana albo usunięcie wymaga zgody. Agent może dopisywać własne testy w `impl/`.

Żeby specyfikacja nie była za słaba, `sowa check` dodatkowo:

- **generuje dane z typów z warunkami**, np. dla `Percent`: 0, 100, 1, 99, 50, i sprawdza, czy funkcja nie łamie swojej sygnatury,
- **sprawdza testy mutacjami**: zmienia kod w `impl/` (np. `-` na `+`, `<` na `<=`) i sprawdza, czy któryś test to wykrywa. Wynik, np. „mutacje wykryte 41 z 44”, człowiek widzi w `sowa review` zamiast kodu.

### Czego to nie załatwi

Błędów w logice, których specyfikacja nie opisuje, np. kto może oznaczyć fakturę jako zapłaconą. Człowiek ufa specyfikacji: jeśli ona ma dziurę, kod też ją ma. Rzeczy krytyczne trzeba wynosić do typów, np. `Invoice(α.owner == user.id)`, bo wtedy sprawdza je kompilator. Moduły, w których to nie wystarcza, można czytać (zob. niżej).

## Czytanie kodu

Są dwie możliwości:

- **Wybrane moduły w trybie `spec`:** `read = ["impl/numbering.sowa"]` w `sowa.toml`. Te pliki trafiają pod CODEOWNERS i nie są zwinięte w PR. W projekcie invoices czytamy tak numerację, bo numeracja bez luk to wymóg prawny, a jej poprawności nie da się wyrazić samym typem.
- **Cały projekt w trybie `code`:** `mode = "code"`. Nie ma `impl/`, funkcja ma ciało pod dokumentacją, jak w [przykładach](../examples/). Człowiek czyta kod w PR, a `approve`, mapa efektów modułu i `effects.lock` wskazują mu, na co patrzeć (zob. [zatwierdzanie.md](zatwierdzanie.md)).

Czy to nie komplikuje? Język jest ten sam, a tryb zmienia tylko układ plików i zakres CODEOWNERS. `sowa check` sprawdza, czy CODEOWNERS pokrywa to, co wynika z `[review]` (`src/`, `docs/`, pliki z `read`), bo inaczej tryb `spec` nic by nie chronił. Przejście między trybami robi `sowa split` (rozdziela ciała do `impl/`) i `sowa join` (scala z powrotem).

## Jak to robią inni

- **Ada** (`.ads` i `.adb`), **OCaml** (`.mli` i `.ml`), pliki nagłówkowe w C: specyfikacja modułu osobno od implementacji. Stąd układ `src/` i `impl/`, ale tam chodzi o kompilację i widoczność, a nie o to, kto co zatwierdza.
- **SPARK** (podzbiór Ady): kontrakty w specyfikacji, dowód, że ciało je spełnia. To najbliżej idei „zatwierdzasz specyfikację, a kompilator pilnuje reszty”.
- **Deno** (`--allow-net=host`): uprawnienia w runtime dla całego procesu. Sowa używa ich jako drugiej linii obrony.
- **Testy mutacyjne** ([Stryker](https://stryker-mutator.io), [PIT](https://pitest.org)) i **testy właściwości** (QuickCheck, [Hypothesis](https://hypothesis.works)): sprawdzanie, czy testy są dość mocne, i generowanie danych.
- **Śledzenie przepływu danych** (Jif, analiza taint): etykiety na danych wrażliwych. Stąd pomysł na `Secret` / `Pii`.

## Otwarte pytania

- Zapis wywołań bibliotek spoza Sowy w `src/`: `extern fn`? Jak opisać ich efekty?
- `Secret` / `Pii`: typ opakowujący czy etykieta na polu? Które funkcje mogą je „odpakować”?
- Zasoby dla bazy: `Db.write(invoices)`, czyli tabela jako zasób?
- Czy `sowa check` ma wymagać minimalnego wyniku mutacji dla modułów, których nikt nie czyta?
- Czy ciała w `impl/` powinny się w ogóle trafiać do repozytorium, czy mogą być generowane od nowa ze specyfikacji?
- Jak `sowa review` ma rozpoznawać „osłabienie” warunku? Proste przypadki (`<= 100` → `<= 1000`) da się sprawdzić solverem, ale nie wszystkie.
