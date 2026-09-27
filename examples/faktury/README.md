# Przykładowy projekt: faktury

Mały system do wystawiania faktur, napisany w Sowie. Pokazuje, jak w jednym projekcie współpracują kod, dokumentacja dla użytkownika i zapisane decyzje.

Reguły biznesowe są uproszczone na potrzeby przykładu. To ilustracja języka, a nie wzór poprawnego rozliczania VAT.

## Struktura

```
faktury/
  sowa.toml                    katalog główny projektu; ścieżki po doc i why liczą się od docs/
  docs.lock                    hashe sygnatur z chwili przejrzenia opisów (szkic formatu)
  effects.lock                 zatwierdzone funkcje z efektami Net i Db.write (szkic formatu)
  .github/CODEOWNERS           *.lock i sowa.toml wymagają zgody właściciela w PR
  src/
    types.sowa                 Percent, Nip, VatRate, ...
    invoice.sowa               Line, Invoice, line_net, totals
    issuing.sowa               issue_invoice: od formularza do zapisanej faktury
    numbering.sowa             numery FV/2026/0001 nadawane w transakcji
    payments.sowa              mark_paid, pay_from_bank
    sending.sowa               send_invoice: PDF i e-mail
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
| `src/issuing.sowa` | cała walidacja formularza przez `as ... or` i `try`; `effects Db.read, Db.write, Clock` bez `Net` |
| `src/sending.sowa` | wysyłka z `effects Net` jako osobna funkcja (decyzja 003) |
| `src/payments.sowa` | warunek na polu w typie parametru: `Invoice(α.status == Issued)` |
| `docs/decyzje/001-vat-od-sumy-w-stawce.md` | przykład z trzema pozycjami po 0,33 zł jako blok `sowa`: test, który pilnuje decyzji, leży przy jej opisie |
| `sowa.toml` | sekcja `[limits]`: ile linii `desc` i ile przykładów może stać przy kodzie |
| `effects.lock`, `.github/CODEOWNERS` | zatwierdzanie: `sowa review` zapisuje, kto i kiedy zatwierdził, a CODEOWNERS pilnuje, żeby zrobił to człowiek |
| `sowa.toml` | sekcja `[effects]`: jakie efekty są dozwolone w projekcie, w których plikach i co musi mieć funkcja z `Net` albo `Db.write` |
| `docs/uzytkownik/faktury.md` | odnośniki `{Percent}`, `{issue_invoice}` i bloki `sowa` uruchamiane jako testy |
| nagłówki plików w `src/` | `desc` i `why zalozenia.md#...` na górze pliku, zamiast komentarza |
| każda funkcja | trzy grupy oddzielone pustą linią: `effects`, dokumentacja (`desc`, `doc`, `why`, `example`), kod |
| `docs.lock` | które opisy trzeba przejrzeć po zmianie sygnatury |

## Czego jeszcze nie ustaliliśmy

Przykład używa kilku rzeczy, których nie ma jeszcze w [specyfikacji](../../docs/zalozenia.md). Są na liście otwartych pytań w [przemyslenia.md](../../docs/przemyslenia.md):

- typy z polami (`type Line` z polami w bloku z wcięciem),
- generyki (`List<Line>`),
- argumenty nazwane (`Line(name: ..., quantity: ...)`),
- kopia z jednym zmienionym polem (`invoice with status: Paid`),
- moduły: tu wszystkie pliki w `src/` widzą się nawzajem bez importów,
- transakcje (`transaction(() => ...)`),
- efekt `Clock` dla odczytu bieżącej daty,
- plik `sowa.toml`.
