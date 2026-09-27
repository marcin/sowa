# Przykładowy projekt: fakturownia_web

Aplikacja webowa do faktur napisana w Sowie. Można w niej wystawić fakturę z formularza, zapisać ją w bazie i wysłać do KSeF. KSeF to tu serwer ksef.pl z własnym formatem JSON wymyślonym na potrzeby przykładu, a nie prawdziwy KSeF.

Przykład zakłada, że całą aplikację napisał agent AI w jednym PR, a człowiek ją zatwierdza. Co agent dostał jako zadanie, co opisał w PR i co zobaczy człowiek, jest w [PR.md](PR.md).

Reguły biznesowe są uproszczone. To ilustracja języka i procesu, a nie wzór rozliczania VAT.

## Uruchomienie

Potrzebny jest Rust (cargo) i [Bun](https://bun.sh). Z katalogu głównego repozytorium:

```
cargo build --manifest-path compiler/Cargo.toml
compiler/target/debug/sowa check examples/fakturownia_web
compiler/target/debug/sowa test examples/fakturownia_web
compiler/target/debug/sowa run --fake ksef examples/fakturownia_web
```

Aplikacja działa pod http://localhost:8080, a faktury zapisuje w `fakturownia.db` obok `sowa.toml`. `--fake ksef` podmienia ksef.pl na atrapę z `[resources.test]`. Bez tej flagi wysyłka kończy się komunikatem „KSeF nie odpowiada”, bo adres z `[resources]` to domena `.example`, która nie istnieje. Więcej w [compiler/README.md](../../compiler/README.md).

## Jak powstaje i kto co czyta

1. Agent pisze `src/`, `impl/`, `docs/` i `sowa.toml`, aż `sowa check` i `sowa test` przechodzą. Testy działają na zasobach z `[resources.test]`: baza w pamięci, zatrzymany zegar i atrapa ksef.pl. Żaden test nie łączy się z siecią.
2. CI wkleja do PR wynik `sowa review --base main` ([przykład](PR.md#wynik-sowa-review)).
3. Człowiek sprawdza, że aplikacja działa, i czyta:
   - `sowa.toml`: dokąd program może się połączyć, a dokąd testy,
   - `src/`: typy, sygnatury z uprawnieniami, przykłady,
   - `docs/`: założenia, instrukcję i decyzje,
   - `impl/storage.sowa`: numerację bez luk, bo typ jej nie wyrazi,
   - `impl/ksef_fake.sowa`: atrapę ksef.pl, bo testy przechodzą tylko wobec jej założeń.
   Resztę `impl/` PR pokazuje zwiniętą.
4. Po zatwierdzeniu agent może poprawiać tylko resztę `impl/`. Zmiana pliku z właścicielem w CODEOWNERS, np. atrapy, wymaga ponownego zatwierdzenia ([przykład](PR.md#po-zatwierdzeniu)).

## Struktura

```
fakturownia_web/
  sowa.toml                    [resources]: baza, zegar, ksef.pl, serwer; [resources.test]: to samo bez sieci
  docs.lock                    pusty: wiersze dopisze sowa review, gdy człowiek potwierdzi opisy
  PR.md                        zadanie, opis PR od agenta i wynik sowa review
  .github/CODEOWNERS           sowa.toml, docs.lock, src/, docs/, impl/storage.sowa, impl/ksef_fake.sowa
  .gitattributes               impl/ zwinięty w PR, poza storage i ksef_fake
  src/
    types.sowa                 Quantity, NetPrice, Name, Nip, VatRate, read_nip
    invoice.sowa               InvoiceNumber, InvoiceStatus, Invoice, totals
    issuing.sowa               InvoiceForm, IssueError, read_line, issue_invoice
    storage.sowa               save_new_invoice, find_invoice, list_invoices, mark_sent
    ksef.sowa                  KsefInvoice, KsefError, to_ksef, send_to_ksef
    ksef_fake.sowa             ksef_fake: atrapa ksef.pl dla testów
    web.sowa                   Request, Response, Route, route, handle, strony
    main.sowa                  main(db, clock, ksef, web)
  impl/                        ciała funkcji, te same nazwy plików co w src/
  docs/
    zalozenia.md               kwoty, NIP, numeracja, dane z formularza, uprawnienia, błędy, testy
    uzytkownik/faktury.md      instrukcja z testem całego przepływu (formularz → zapis → KSeF)
    decyzje/
      001-ksef-osobnym-krokiem.md
      002-ponowna-wysylka.md
```

## Co warto zobaczyć

| Gdzie | Co pokazuje |
|---|---|
| `src/invoice.sowa` | wariant z danymi: `SentToKsef(reference: ..., sent_at: ...)`; numer z KSeF istnieje tylko po wysyłce |
| `src/storage.sowa`, `src/ksef.sowa` | warunek na wariancie w typie: `Invoice(α.status == Issued)` na wejściu, `Invoice(α.status is SentToKsef)` na wyjściu; wysłanej faktury nie da się wysłać drugi raz |
| `src/issuing.sowa` | błąd z danymi: `InvalidLine(position: 2, field: LineQuantity)` zamiast jednego „zły formularz” |
| `src/web.sowa` | `handle` z kompletem uprawnień; `post_invoice` bez `Http` (wystawienie niczego nie wysyła); `get_invoice` i `get_list` tylko z `DbRead` |
| `src/web.sowa` | `Route` jako wariant z danymi i `property`: `route(Get, invoice_path(number)) == ShowInvoice(number: number)` |
| `impl/web.sowa` | `match` z blokiem po `=>`; ramię `IssueError e` dla całej unii; `as Invoice(α.status == Issued)` zawęża status |
| `impl/web.sowa` | `error_message`: jedno miejsce z komunikatami; nowy wariant błędu bez komunikatu to błąd kompilacji |
| `sowa.toml` | `[resources]` i `[resources.test]`: te same nazwy i typy, inne wartości; `ksef` w testach to `fake = "ksef_fake"` |
| `src/ksef_fake.sowa`, `impl/ksef_fake.sowa` | atrapa jako czysta funkcja `HttpRequest -> HttpResponse`; jej kod człowiek czyta |
| `src/issuing.sowa` | przykład w kilku liniach: `invoice = try issue_invoice(form, db, clock)` na świeżej bazie z `[resources.test]` |
| `docs/uzytkownik/faktury.md` | blok `sowa` przez `handle`: formularz, przekierowanie, wysyłka, drugi klik |
| `docs/decyzje/002-ponowna-wysylka.md` | założenie o cudzym serwerze zapisane w atrapie i w decyzji |
| `PR.md` | co zobaczy człowiek: uprawnienia, zasoby testowe, warunki udowodnione i sprawdzane w runtime |

## Czego jeszcze nie ma w specyfikacji

Warianty z danymi, blok po `=>` i `[resources.test]` są już w [specyfikacji](../../docs/zalozenia.md). Przykład używa też rzeczy, których w niej jeszcze nie ma. Wszystkie są w [przemyslenia.md](../../docs/przemyslenia.md#aplikacja-webowa). Kompilator obsługuje je tak, jak opisuje [compiler/README.md](../../compiler/README.md#reguły-których-nie-ma-w-specyfikacji); to propozycje, a nie decyzje:

- `match` na kilku wartościach i na liście segmentów, `_` w `match` (`route`),
- literały `html"..."` z escapowaniem według miejsca,
- `as` z tekstu na rekord: formularz (`req.body as InvoiceForm`) i JSON (`as KsefInvoice`), a w drugą stronę `to_json`,
- uprawnienie `Server` i `web.serve(...)`,
- wbudowane typy `HttpRequest`, `HttpResponse`, `Method` i operacja `ksef.post(...)`,
- API bazy: `db.save`, `db.get<T>`, `db.all<T>`, `db.transaction`, i schemat bazy (w kompilatorze: jedna tabela klucz–wartość w SQLite),
- sekrety: `token_env` w `[resources]`,
- logowanie, sesje i CSRF: aplikacja ich nie ma ([założenia](docs/zalozenia.md#poza-zakresem)).
