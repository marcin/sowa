# Specyfikacja i kod: człowiek zatwierdza specyfikację, agent pisze kod

Agent pisze szybciej, niż człowiek czyta. Przy dużych zmianach „człowiek czyta i zatwierdza” zamienia się w zatwierdzanie bez czytania. Dlatego w Sowie człowiek nie czyta kodu, tylko zatwierdza specyfikację, i to wtedy, gdy całość już działa:

| | |
|---|---|
| co czyta człowiek | specyfikację: typy, sygnatury z uprawnieniami, warunki wyniku, `desc`, `doc`, `why`, `example`, `property`, dokumentację |
| kiedy | raz, gdy PR jest gotowy i testy przechodzą (zob. [Zatwierdzenie na końcu](#zatwierdzenie-na-końcu)) |
| układ plików | `src/` ze specyfikacją, `impl/` z ciałami funkcji |
| co chroni CODEOWNERS | `src/`, `docs/`, `sowa.toml`, `docs.lock` |
| co pilnuje ciał funkcji | kompilator, uprawnienia, testy, których agent nie może osłabić, i runtime |

Nie ma trybów ani ustawień. Kod wybranego modułu można czytać, ale to decyzja w CODEOWNERS, a nie w `sowa.toml` (zob. [Czytanie kodu](#czytanie-kodu)).

## Układ plików

Plik w `src/` to specyfikacja modułu. Są w nim typy, sygnatury, dokumentacja i przykłady, a nie ma ciał funkcji:

```
// src/invoice.sowa
fn totals(lines: List<Line>(len(α) > 0), discount: Percent) -> Totals(α.gross == α.net + α.vat)
  desc Sumy netto, VAT i brutto dla całej faktury.
  doc uzytkownik/faktury.md#sumy-i-zaokrąglenia
  why decyzje/001-vat-od-sumy-w-stawce.md

  example totals([Line(name: "Usługa", quantity: 1, unit_net: 100, vat: Vat23)], 0)
    == Totals(net: 100, vat: 23, gross: 123)
  property totals(lines, discount).net <= totals(lines, 0).net
```

Plik o tej samej nazwie w `impl/` zawiera ciała. Powtarza linię `fn`, żeby dało się go czytać bez skakania między plikami, ale nie powtarza dokumentacji:

```
// impl/invoice.sowa
fn totals(lines: List<Line>(len(α) > 0), discount: Percent) -> Totals(α.gross == α.net + α.vat)
  var net = 0
  var vat = 0
  for rate in distinct(lines.map(l => l.vat))
    ...
  return Totals(net: net, vat: vat, gross: net + vat)
```

Reguły:

- Sygnatura w `impl/` musi być identyczna jak w `src/`. Zmiana sygnatury w samym `impl/` to błąd kompilacji, więc agent musi zmienić też `src/`, a to wymaga zgody człowieka.
- `impl/` może mieć własne funkcje pomocnicze i typy. Są prywatne: spoza modułu widać tylko to, co jest w `src/`.
- Funkcja pomocnicza może mieć uprawnienie w parametrze, ale dostanie je tylko od funkcji z `src/`, która je ma. Moduł, w którego `src/` nie ma uprawnień, jest w całości czysty.
- `example` i `property` w `impl/` to własne testy agenta. Uruchamiają się, ale nie są specyfikacją i nie wymagają zgody.

## Zatwierdzenie na końcu

Specyfikacja zatwierdzana przed kodem kosztuje dwa razy: człowiek czyta pomysł, który nie wiadomo, czy zadziała, a w trakcie implementacji i tak coś się zmienia. Dlatego agent najpierw buduje, a człowiek zatwierdza raz, gdy całość działa:

1. Agent w jednym PR zmienia `src/`, `impl/` i `docs/`, aż testy przejdą. Specyfikacja zmienia się razem z kodem i nikt jej po drodze nie zatwierdza.
2. CI wkleja do PR wynik `sowa review --base main`. Człowiek sprawdza, że program działa, czyta listę zmian w znaczeniu i zatwierdza PR.
3. Po zatwierdzeniu agent może jeszcze poprawiać `impl/`. `sowa check --ci` sprawdza, czy od zatwierdzonego commitu nie zmienił się żaden plik z właścicielem w CODEOWNERS. Jeśli się zmienił, trzeba zatwierdzić jeszcze raz.

Zatwierdzanie gotowego kodu ma jedną pułapkę: specyfikacja jest już dopasowana do kodu, a agent mógł poluzować warunek, żeby test przeszedł. Tę pułapkę rozbraja `sowa review`. Porównuje z `main`, a nie z poprzednią wersją w PR, i każde osłabienie oraz każdy usunięty test stawia na górze, z kontrprzykładem. Człowiek nie musi śledzić, co działo się w PR. Widzi wynik netto. Szczegóły: [zatwierdzanie.md](zatwierdzanie.md#zatwierdzenie-na-końcu).

## Co zatwierdza człowiek

Tylko to, co leży w `src/`, `docs/`, `sowa.toml` i `docs.lock`, czyli wszystko poza ciałami funkcji:

- typy i warunki na nich, także warunki wyniku,
- sygnatury, razem z uprawnieniami, np. `mail: Mailer`,
- `desc`, `doc`, `why`,
- `example` i `property` w `src/` oraz bloki `sowa` w `.md`, czyli testy, które agent musi spełnić,
- zasoby w `sowa.toml`, np. adres serwera pocztowego.

`sowa review` pokazuje zmiany w znaczeniu, uporządkowane od najbardziej ryzykownych, a nie w kolejności plików:

```
$ sowa review --base main

Specyfikacja (src/, docs/, sowa.toml): 5 zmian

  [1] uprawnienie     send_reminder  src/sending.sowa:24   nowa funkcja z Mailer (mail: smtp.firma.pl:587)
  [2] osłabienie      Percent        src/types.sowa:5      α <= 100  →  α <= 1000
                                                           dopuszcza np. 101
  [3] usunięty test   totals         src/invoice.sowa:50   property totals(lines, discount).net <= totals(lines, 0).net
  [4] rozszerzenie    read_nip       src/types.sowa:24     parametr: String(len(α) <= 20)  →  String
  [5] zwykłe          line_net       src/invoice.sowa:38   nowy example

Kod (impl/): 3 pliki, 120 linii, nie wymaga przeglądu.
  warunki wyniku: 3 udowodnione, 1 sprawdzany w runtime (line_net)
  testy: 14 przykładów, 3 właściwości na 300 generowanych danych
  mutacje wykryte przez testy: 41 z 44
```

Osłabienie rozpoznaje solver i pokazuje kontrprzykład. Gdy solver nie umie rozstrzygnąć, zmiana i tak trafia do osłabień. Kategorie i ich kolejność: [zalozenia.md](zalozenia.md#sowa-review).

## Izolacja kodu

Kod w `impl/` jest odizolowany na trzech poziomach:

1. **W repozytorium.** `impl/` nie ma właściciela w CODEOWNERS, więc zmiany w nim nie wymagają zgody człowieka. W PR jest domyślnie zwinięty (`linguist-generated` w `.gitattributes`), żeby recenzent widział tylko specyfikację.
2. **W kompilatorze.** Kod w `impl/` nie wyjdzie poza specyfikację: nie dostanie uprawnienia, którego nie ma w sygnaturze, nie doda wariantu błędu, nie osłabi warunku i nie wywoła kodu spoza Sowy. Uprawnienia nie da się utworzyć w kodzie, więc jedyne, jakie istnieją, to te z `[resources]` w `sowa.toml`. Wywołania bibliotek spoza Sowy (np. z npm) mogą stać tylko w `src/`, więc każda taka furtka jest zatwierdzana.
3. **W runtime.** `sowa run` uruchamia skompilowany program z uprawnieniami wziętymi z `[resources]`. Przy kompilacji do TypeScriptu i uruchamianiu w Deno: `--allow-net=smtp.firma.pl:587,localhost:5432` i nic więcej. To druga linia obrony, na wypadek błędu w kompilatorze.

## Co blokuje zagrożenia w nieczytanym kodzie

| Zagrożenie | Co to blokuje |
|---|---|
| kod robi coś poza liczeniem | uprawnienia: bez `Db`, `Mailer` czy `Clock` w parametrach nie ma czym tego zrobić |
| kod wysyła dane nie tam, gdzie trzeba | uprawnienia z zasobem: każdy `Mailer` to serwer z `sowa.toml`, adresu nie da się podać w kodzie |
| wyciek danych wrażliwych | typ `Secret` / `Pii`, którego nie da się wysłać ani zalogować poza funkcjami z `src/` (do ustalenia) |
| furtka: kod spoza Sowy, `eval`, refleksja | brak w języku; wywołania bibliotek spoza Sowy tylko w `src/` |
| nowa biblioteka | zależności w `sowa.toml`, pod CODEOWNERS |
| operacje nieodwracalne | osobne uprawnienie, np. `DbDelete` (do ustalenia), widoczne w sygnaturze i na górze `sowa review` |
| kod liczy źle | typy z warunkami, warunki wyniku, `example` i `property` w `src/`, których agent nie może osłabić; dane generowane z typów i mutacje |
| agent dopasowuje specyfikację do kodu | `sowa review` porównuje z `main` i stawia osłabienia z kontrprzykładem na górze; check wykrywa zmianę specyfikacji po zatwierdzeniu |

## Uprawnienia z zasobem

`effects Net` pozwalałoby wysłać cokolwiek dokądkolwiek. W Sowie funkcja dostaje uprawnienie do jednego zasobu:

```
fn send_invoice(invoice: Invoice, mail: Mailer) -> Sent | SendError
```

```toml
[resources]
mail = { type = "Mailer", server = "smtp.firma.pl:587" }
```

Uprawnienia nie da się utworzyć w kodzie, więc każdy `Mailer` w programie pochodzi od `main`, który dostaje go od runtime według `[resources]`. `mail.send(to, subject, body)` nie przyjmuje adresu serwera. Funkcji w rodzaju `http_post("https://gdzies.com", invoice)` nie ma, a `Http` jest przypięty do jednego adresu z `sowa.toml`. Człowiek zatwierdza raz: „faktury wolno wysyłać przez smtp.firma.pl”.

Reguły uprawnień: [zalozenia.md](zalozenia.md#uprawnienia).

## Specyfikacja, której agent nie osłabi

Jeśli agent pisze i kod, i testy, testy niczego nie dowodzą: agent może poprawić test, żeby przeszedł. Tu `example` i `property` w `src/` oraz bloki `sowa` w `.md` są częścią specyfikacji, więc ich zmiana albo usunięcie wymaga zgody. Agent może dopisywać własne testy w `impl/`.

Sam `example` to słaba specyfikacja: sprawdza jeden przypadek, a kod można do niego dopasować. Dlatego specyfikacja ma trzy poziomy:

- **warunek wyniku**, np. `-> Totals(α.gross == α.net + α.vat)`: kompilator próbuje go udowodnić, a jeśli nie umie, sprawdza go w runtime i w testach,
- **`property`**, np. `property totals(lines, discount).net <= totals(lines, 0).net`: warunek dla dowolnych danych, sprawdzany na danych generowanych z typów,
- **`example`**: jeden konkretny przypadek, najłatwiejszy do przeczytania.

Do tego `sowa check`:

- **generuje dane z typów z warunkami**, np. dla `Percent`: 0, 1, 99, 100 i losowe, i sprawdza na nich `property` i warunki wyniku,
- **sprawdza testy mutacjami**: zmienia kod w `impl/` (np. `-` na `+`, `<` na `<=`) i sprawdza, czy któryś test to wykrywa. Wynik, np. „mutacje wykryte 41 z 44”, człowiek widzi w `sowa review` zamiast kodu.

## Czego to nie załatwi

Błędów w logice, których specyfikacja nie opisuje, np. kto może oznaczyć fakturę jako zapłaconą. Człowiek ufa specyfikacji: jeśli ona ma dziurę, kod też ją ma. Rzeczy krytyczne trzeba wynosić do typów, np. `Invoice(α.owner == user.id)`, bo wtedy sprawdza je kompilator. Moduły, w których to nie wystarcza, można czytać (zob. niżej).

## Czytanie kodu

Kod wybranego modułu czyta się tak samo jak specyfikację, przez CODEOWNERS. Wystarczy dopisać plik:

```
# .github/CODEOWNERS
impl/numbering.sowa   @marcin
```

```
# .gitattributes
impl/numbering.sowa   -linguist-generated
```

Plik trafia wtedy pod ochronę i nie jest zwinięty w PR. Check z [Zatwierdzenia na końcu](#zatwierdzenie-na-końcu) obejmuje wszystkie pliki z właścicielem w CODEOWNERS, więc zmiana w `impl/numbering.sowa` po zatwierdzeniu też wymaga ponownej zgody. W projekcie invoices tak czyta się numerację, bo numeracja bez luk to wymóg prawny, a jej poprawności nie da się wyrazić samym typem.

`sowa.toml` nic o tym nie wie. Nie ma trybu „cały projekt czytany”: kto chce czytać cały kod, dopisuje `impl/` do CODEOWNERS.

## Jak to robią inni

- **Ada** (`.ads` i `.adb`), **OCaml** (`.mli` i `.ml`), pliki nagłówkowe w C: specyfikacja modułu osobno od implementacji. Stąd układ `src/` i `impl/`, ale tam chodzi o kompilację i widoczność, a nie o to, kto co zatwierdza.
- **SPARK** (podzbiór Ady): kontrakty w specyfikacji, dowód, że ciało je spełnia. To najbliżej idei „zatwierdzasz specyfikację, a kompilator pilnuje reszty”.
- **Obiekty uprawnień** (E, [Pony](https://www.ponylang.io), [Austral](https://austral-lang.org), WASI): funkcja może zrobić tylko to, na co pozwalają wartości, które dostała. Stąd uprawnienia w parametrach.
- **Deno** (`--allow-net=host`): uprawnienia w runtime dla całego procesu. Sowa używa ich jako drugiej linii obrony.
- **Testy mutacyjne** ([Stryker](https://stryker-mutator.io), [PIT](https://pitest.org)) i **testy właściwości** (QuickCheck, [Hypothesis](https://hypothesis.works)): sprawdzanie, czy testy są dość mocne, i generowanie danych.
- **Śledzenie przepływu danych** (Jif, analiza taint): etykiety na danych wrażliwych. Stąd pomysł na `Secret` / `Pii`.

## Otwarte pytania

- Zapis wywołań bibliotek spoza Sowy w `src/`: `extern fn`? Jakie uprawnienia mają dostawać?
- `Secret` / `Pii`: typ opakowujący czy etykieta na polu? Które funkcje mogą je „odpakować”?
- Uprawnienia węższe niż baza: `Db(invoices)`, czyli tabela jako zasób?
- Czy `sowa check` ma wymagać minimalnego wyniku mutacji dla modułów, których nikt nie czyta?
- Czy ciała w `impl/` powinny w ogóle trafiać do repozytorium, czy mogą być generowane od nowa ze specyfikacji?
- Porównanie warunków solverem: jak często kończy się „nie udało się porównać” na prawdziwym kodzie? Jeśli za często, lista osłabień straci wartość.
