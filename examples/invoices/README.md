# Przykładowy projekt: invoices

Mały system do wystawiania faktur z prostą aplikacją webową, napisany w Sowie. Pokazuje, jak w jednym projekcie współpracują kod, dokumentacja dla użytkownika i zapisane decyzje.

Człowiek zatwierdza [specyfikację](../../docs/specyfikacja.md) w `src/` i dokumentację w `docs/`, a kodu w `impl/` nie czyta. Wyjątkiem jest numeracja, której kod człowiek czyta.

Reguły biznesowe są uproszczone na potrzeby przykładu. To ilustracja języka, a nie wzór poprawnego rozliczania VAT.

## Struktura

```
invoices/
  sowa.toml                    katalog główny projektu; ścieżki po doc i why liczą się od docs/
  docs.lock                    hashe sygnatur z chwili przejrzenia opisów (szkic formatu)
  .github/CODEOWNERS           src/, docs/, sowa.toml, docs.lock i impl/numbering.sowa wymagają zgody w PR
  .gitattributes               impl/ zwinięty w PR, poza impl/numbering.sowa
  src/                         specyfikacja: typy, sygnatury z uprawnieniami, opisy, przykłady
    types.sowa                 Percent, Nip, VatRate, ...
    invoice.sowa               Line, Invoice, line_net, totals
    issuing.sowa               issue_invoice: od formularza do zapisanej faktury
    numbering.sowa             numery FV/2026/0001 nadawane w transakcji
    payments.sowa              mark_paid, pay_from_bank
    sending.sowa               send_invoice: PDF i e-mail przez mail: Mailer
    web.sowa                   aplikacja webowa: route, handle, strony HTML
    main.sowa                  main: jedyne miejsce, w którym program dostaje uprawnienia
  impl/                        ciała funkcji, te same nazwy plików co w src/
    numbering.sowa             ten plik człowiek czyta (jest w CODEOWNERS)
    ...
  docs/
    zalozenia.md               ogólne zasady: kwoty, uprawnienia, błędy
    uzytkownik/faktury.md      instrukcja dla użytkownika
    decyzje/                   dlaczego działa tak, a nie inaczej
      001-vat-od-sumy-w-stawce.md
      002-numeracja-bez-luk.md
      003-wysylka-osobno.md
```

## Co warto zobaczyć

| Gdzie | Co pokazuje |
|---|---|
| `src/` i `impl/` | ten sam moduł w dwóch plikach: w `src/` specyfikacja bez ciał, w `impl/` ciała z powtórzoną linią `fn` |
| `impl/sending.sowa` | `subject`: prywatna funkcja pomocnicza, której nie ma w `src/` |
| `impl/issuing.sowa` | cała walidacja formularza przez `as ... or` i `try`; `db: Db, clock: Clock` w parametrach, bez `Mailer`; data z `clock.today()` |
| `src/sending.sowa`, `sowa.toml` | wysyłka z `mail: Mailer` jako osobna funkcja (decyzja 003); adres serwera tylko w `[resources]` |
| `src/main.sowa`, `sowa.toml` | `fn main(db: Db, clock: Clock, mail: Mailer, web: Server)`: runtime przekazuje zasoby z `[resources]` według nazwy parametru |
| `src/web.sowa` | `Request` i `Response` jako zwykłe wartości; czysty `route` z przykładami; `handle` z kompletem uprawnień, a każdy adres z węższym zestawem (`get_invoice` tylko z `DbRead`) |
| `impl/web.sowa` | `match` na błędach zamienia je na odpowiedzi HTTP; strony z literałów `html"..."`, w których wstawiony tekst jest escapowany |
| `src/numbering.sowa` | `last_invoice_seq(year, db: DbRead)`: funkcja, która tylko czyta, dostaje węższe uprawnienie |
| `impl/numbering.sowa` | `db.transaction(tx => ...)`: transakcja jako operacja na `Db` |
| `src/payments.sowa` | warunek na polu w typie parametru i w wyniku: `Invoice(α.status == Issued)` → `Invoice(α.status == Paid)` |
| `src/invoice.sowa` | warunek wyniku `Totals(α.gross == α.net + α.vat)` i `property` sprawdzane na danych generowanych z typów |
| `docs/decyzje/001-vat-od-sumy-w-stawce.md` | przykład z trzema pozycjami po 0,33 zł jako blok `sowa`: test, który pilnuje decyzji, leży przy jej opisie |
| `sowa.toml` | sekcja `[limits]`: ile linii `desc` i ile przykładów może stać przy kodzie |
| `.github/CODEOWNERS`, `.gitattributes` | izolacja kodu: `impl/` bez właściciela i zwinięty w PR, `src/` i `docs/` pod ochroną; `impl/numbering.sowa` czytany ręcznie, bo dopisany do obu plików |
| `docs/uzytkownik/faktury.md` | odnośniki `{Percent}`, `{issue_invoice}` i bloki `sowa` uruchamiane jako testy |
| nagłówki plików w `src/` | `desc` i `why zalozenia.md#...` na górze pliku, zamiast komentarza |
| każda funkcja w `src/` | grupy oddzielone pustą linią: opis (`desc`, `doc`, `why`), przykłady (`example`, `property`); kod jest w `impl/` |
| `docs.lock` | które opisy trzeba przejrzeć po zmianie sygnatury |

## Czego jeszcze nie ma w specyfikacji

Przykład używa kilku rzeczy, których nie ma jeszcze w [specyfikacji](../../docs/zalozenia.md). Są na liście otwartych pytań w [przemyslenia.md](../../docs/przemyslenia.md):

- typy z polami (`type Line` z polami w bloku z wcięciem),
- generyki (`List<Line>`),
- argumenty nazwane (`Line(name: ..., quantity: ...)`),
- kopia z jednym zmienionym polem (`invoice with status: Paid`),
- moduły: tu wszystkie pliki w `src/` widzą się nawzajem bez importów,
- zapis wywołań bibliotek spoza Sowy w `src/`,
- transakcje (`db.transaction(tx => ...)`),
- pełna lista operacji na wbudowanych uprawnieniach (`clock.today()`, `mail.send(...)`),
- plik `sowa.toml`,
- w aplikacji webowej: `match` na kilku wartościach i na liście segmentów, `_` w `match`, odczyt formularza przez `as InvoiceForm`, literały `html"..."`, uprawnienie `Server`. Pełna lista w [przemyslenia.md](../../docs/przemyslenia.md#aplikacja-webowa).
