# Przykładowy projekt: invoices

Mały system do wystawiania faktur, napisany w Sowie. Pokazuje, jak w jednym projekcie współpracują kod, dokumentacja dla użytkownika i zapisane decyzje.

Projekt działa w [trybie `spec`](../../docs/tryby.md): człowiek zatwierdza specyfikację w `src/` i dokumentację w `docs/`, a kodu w `impl/` nie czyta. Wyjątkiem jest numeracja, której kod człowiek czyta.

Reguły biznesowe są uproszczone na potrzeby przykładu. To ilustracja języka, a nie wzór poprawnego rozliczania VAT.

## Struktura

```
invoices/
  sowa.toml                    katalog główny projektu; ścieżki po doc i why liczą się od docs/
  docs.lock                    hashe sygnatur z chwili przejrzenia opisów (szkic formatu)
  effects.lock                 zatwierdzony kod numeracji i mapa efektów issuing (szkic formatu)
  .github/CODEOWNERS           src/, docs/, sowa.toml, *.lock i impl/numbering.sowa wymagają zgody w PR
  .gitattributes               impl/ zwinięty w PR, poza impl/numbering.sowa
  src/                         specyfikacja: typy, sygnatury, efekty, opisy, przykłady
    types.sowa                 Percent, Nip, VatRate, ...
    invoice.sowa               Line, Invoice, line_net, totals
    issuing.sowa               issue_invoice: od formularza do zapisanej faktury
    numbering.sowa             numery FV/2026/0001 nadawane w transakcji
    payments.sowa              mark_paid, pay_from_bank
    sending.sowa               send_invoice: PDF i e-mail przez Net(mail)
  impl/                        ciała funkcji, te same nazwy plików co w src/
    numbering.sowa             ten plik człowiek czyta ([review] read)
    ...
  docs/
    zalozenia.md               ogólne zasady: kwoty, efekty, błędy
    uzytkownik/faktury.md      instrukcja dla użytkownika
    decyzje/                   dlaczego działa tak, a nie inaczej
      001-vat-od-sumy-w-stawce.md
      002-numeracja-bez-luk.md
      003-wysylka-osobno.md
```

## Co warto zobaczyć

| Gdzie | Co pokazuje |
|---|---|
| `src/` i `impl/` | ten sam moduł w dwóch plikach: w `src/` specyfikacja bez ciał, w `impl/` ciała z powtórzoną linią `fn` i `effects` |
| `sowa.toml` | sekcja `[review]`: `mode = "spec"` i `read = ["impl/numbering.sowa"]` |
| `impl/sending.sowa` | `subject`: prywatna funkcja pomocnicza, której nie ma w `src/` |
| `impl/issuing.sowa` | cała walidacja formularza przez `as ... or` i `try`; `effects Db.read, Db.write, Clock` bez `Net` |
| `src/sending.sowa`, `sowa.toml` | wysyłka z `effects Net(mail)` jako osobna funkcja (decyzja 003); adres serwera tylko w `[effects.resources]` |
| `src/payments.sowa` | warunek na polu w typie parametru: `Invoice(α.status == Issued)` |
| `docs/decyzje/001-vat-od-sumy-w-stawce.md` | przykład z trzema pozycjami po 0,33 zł jako blok `sowa`: test, który pilnuje decyzji, leży przy jej opisie |
| `sowa.toml` | sekcja `[limits]`: ile linii `desc` i ile przykładów może stać przy kodzie |
| `.github/CODEOWNERS`, `.gitattributes` | izolacja kodu: `impl/` bez właściciela i zwinięty w PR, `src/` i `docs/` pod ochroną |
| `effects.lock` | zatwierdzanie kodu, który człowiek czyta: `sowa review` zapisuje, kto i kiedy zatwierdził |
| `sowa.toml` | sekcja `[effects]`: jakie efekty są dozwolone w projekcie, w których plikach i co musi mieć funkcja z `Net` albo `Db.write` |
| `sowa.toml`, `effects.lock` | sekcja `[review.files]`: dla `issuing` zatwierdza się mapę efektów modułu, czyli gdzie powstaje jaki efekt, bez czytania kodu |
| `docs/uzytkownik/faktury.md` | odnośniki `{Percent}`, `{issue_invoice}` i bloki `sowa` uruchamiane jako testy |
| nagłówki plików w `src/` | `desc` i `why zalozenia.md#...` na górze pliku, zamiast komentarza |
| każda funkcja w `src/` | grupy oddzielone pustą linią: `effects`, opis (`desc`, `doc`, `why`), przykłady (`example`); kod jest w `impl/` |
| `docs.lock` | które opisy trzeba przejrzeć po zmianie sygnatury |

## Czego jeszcze nie ustaliliśmy

Przykład używa kilku rzeczy, których nie ma jeszcze w [specyfikacji](../../docs/zalozenia.md). Są na liście otwartych pytań w [przemyslenia.md](../../docs/przemyslenia.md):

- typy z polami (`type Line` z polami w bloku z wcięciem),
- generyki (`List<Line>`),
- argumenty nazwane (`Line(name: ..., quantity: ...)`),
- kopia z jednym zmienionym polem (`invoice with status: Paid`),
- moduły: tu wszystkie pliki w `src/` widzą się nawzajem bez importów,
- zapis wywołań bibliotek spoza Sowy w `src/` (`mail_send`),
- transakcje (`transaction(() => ...)`),
- efekt `Clock` dla odczytu bieżącej daty,
- plik `sowa.toml`.
