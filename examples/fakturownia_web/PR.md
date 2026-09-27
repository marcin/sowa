# PR: fakturownia_web

Przykład, jak wygląda PR, w którym agent AI napisał całą aplikację, a człowiek ją zatwierdza. Kompilator z [compiler/](../../compiler) umie już `sowa check`, `sowa test` i `sowa run`, ale nie `sowa review`, więc wynik `sowa review` to szkic tego, co pokaże narzędzie. Numery linii zgadzają się z plikami w tym katalogu.

## Zadanie dla agenta

> Aplikacja webowa do faktur. Formularz: nabywca (nazwa, NIP, e-mail) i pozycje (nazwa, ilość, cena netto, stawka VAT). Faktura dostaje numer FV/rok/kolejny i trafia do bazy. Z jej strony można wysłać ją do KSeF (https://ksef.pl/api, POST /invoices, JSON, odpowiedź z numerem KSeF). Błąd KSeF nie może zgubić faktury.

## Opis PR od agenta

> Nowa aplikacja `fakturownia_web`.
>
> - Wystawienie i wysyłka to dwa kroki ([decyzja 001](docs/decyzje/001-ksef-osobnym-krokiem.md)). Wystawienie nie ma `Http`.
> - Numer nadaje się w transakcji razem z zapisem, bez luk (`impl/storage.sowa`).
> - Ponowna wysyłka zakłada, że ksef.pl dla tego samego numeru zwraca ten sam numer KSeF ([decyzja 002](docs/decyzje/002-ponowna-wysylka.md)). To założenie jest w atrapie `ksef_fake`. Do potwierdzenia: dokumentacja ksef.pl tego nie opisuje.
> - Testy: 26 przykładów, 5 `property`, 2 bloki `sowa` w instrukcji. Wszystkie na `[resources.test]`, bez sieci.
> - Poza zakresem: logowanie, CSRF, korekty, prawdziwy format FA.

## Wynik `sowa review`

```
$ sowa review --base main

Specyfikacja (src/, docs/, sowa.toml): nowy projekt examples/fakturownia_web

Uprawnienia
  [1] uprawnienie     db             sowa.toml:14             nowy zasób Db: sqlite:fakturownia.db
  [2] uprawnienie     clock          sowa.toml:15             nowy zasób Clock
  [3] uprawnienie     ksef           sowa.toml:19             nowy zasób Http: https://ksef.example/api, token z KSEF_TOKEN
  [4] uprawnienie     web            sowa.toml:20             nowy zasób Server: nasłuch 0.0.0.0:8080
  [5] uprawnienie     main           src/main.sowa:4          db: Db, clock: Clock, ksef: Http, web: Server
  [6] uprawnienie     handle         src/web.sowa:37          db: Db, clock: Clock, ksef: Http
  [7] uprawnienie     post_ksef      src/web.sowa:52          db: Db, clock: Clock, ksef: Http
  [8] uprawnienie     send_to_ksef   src/ksef.sowa:37         ksef: Http, db: Db, clock: Clock
  [9] uprawnienie     post_invoice   src/web.sowa:46          db: Db, clock: Clock
 [10] uprawnienie     issue_invoice  src/issuing.sowa:42      db: Db, clock: Clock
 [11] uprawnienie     save_new_invoice src/storage.sowa:8     db: Db
 [12] uprawnienie     mark_sent      src/storage.sowa:22      db: Db
 [13] uprawnienie     get_invoice    src/web.sowa:50          db: DbRead
 [14] uprawnienie     get_list       src/web.sowa:43          db: DbRead
 [15] uprawnienie     find_invoice   src/storage.sowa:13      db: DbRead
 [16] uprawnienie     list_invoices  src/storage.sowa:16      db: DbRead

      Http (ksef) mają tylko: main → handle → post_ksef → send_to_ksef

Zasoby testowe i atrapy
 [17] usunięty test   db             sowa.toml:26             nowy zasób testowy: Db w pamięci
 [18] usunięty test   clock          sowa.toml:27             nowy zasób testowy: zegar zatrzymany na 2026-09-27T10:00:00
 [19] usunięty test   ksef           sowa.toml:28             nowa atrapa: ksef_fake
 [20] usunięty test   ksef_fake      src/ksef_fake.sowa:6     nowa atrapa, ciało w impl/ksef_fake.sowa (12 linii, do przeczytania)

Osłabienia: brak (nowy projekt)
Rozszerzenia: brak (nowy projekt)

Zwykłe
      27 typów, 15 czystych funkcji, 26 przykładów, 5 property, 2 bloki sowa w docs/

Warunki wyniku
      udowodnione (8):  totals, to_ksef, vat_percent, invoice_path, issue_invoice,
                        save_new_invoice, mark_sent, send_to_ksef
      sprawdzane w runtime i testami (2):
                        line_net       src/invoice.sowa:47   Money(α >= 0): mnożenie z zaokrągleniem
                        format_number  src/invoice.sowa:43   InvoiceNumber: wyrażenie regularne

impl/ (zwinięte): 8 plików, 313 linii
      do przeczytania (CODEOWNERS): impl/storage.sowa (40), impl/ksef_fake.sowa (12)
      mutacje wykryte 61 z 64; przeżyły:
        impl/storage.sowa:34   list_invoices: usunięte .reverse()      żaden test nie ma dwóch faktur
        impl/web.sowa:141      status_label: "wystawiona" → ""         żaden test nie sprawdza listy
        impl/ksef_fake.sowa:9  == 0 → <= 0                            równoważna: brutto nie jest ujemne

Opisy do przejrzenia (docs.lock pusty, wszystkie nowe):
      zalozenia.md#kwoty, #nip, #numeracja, #dane-z-formularza, #uprawnienia, #błędy
      uzytkownik/faktury.md#wystawianie-faktury, #stawki-vat, #sumy, #wysyłka-do-ksef
      decyzje/001-ksef-osobnym-krokiem.md, decyzje/002-ponowna-wysylka.md
```

## Co sprawdza człowiek

Najpierw uruchamia aplikację (`sowa run --fake ksef examples/fakturownia_web`, potem http://localhost:8080) i przechodzi cały przepływ: formularz z błędną ilością, poprawna faktura, wysyłka, drugi klik. Potem czyta wynik od góry:

1. **Zasoby [1]–[4].** Czy program ma łączyć się tylko z tą bazą i z ksef.pl. `web` nasłuchuje na `0.0.0.0:8080`, a aplikacja nie ma logowania ([poza zakresem](docs/zalozenia.md#poza-zakresem)). Tu człowiek decyduje, czy to ma być `127.0.0.1`.
2. **Kto ma `Http` [5]–[8].** Łańcuch jest jeden, a `post_invoice` i `issue_invoice` go nie mają. To wystarczy, żeby wiedzieć, że wystawienie niczego nie wysyła, bez czytania `impl/`.
3. **Kto ma `Db`, a kto `DbRead` [9]–[16].** Strony, które tylko pokazują, nie zapiszą niczego.
4. **Atrapa [19]–[20].** Czyta 12 linii `impl/ksef_fake.sowa` i potwierdza założenie z decyzji 002 albo je odrzuca. Tego żaden solver nie rozstrzygnie, bo dotyczy cudzego serwera.
5. **Numeracja.** Czyta `impl/storage.sowa`: licznik i faktura w jednej transakcji.
6. **Mutacje, które przeżyły.** Pierwsza pokazuje brak testu kolejności listy. Człowiek może poprosić agenta o przykład z dwiema fakturami. Nowy `example` to zmiana w `src/`, więc wejdzie do tego samego zatwierdzenia.
7. **Opisy.** Czyta `docs/`. Potwierdzenie w `sowa review` dopisuje wiersze do `docs.lock`.

Kodu w `impl/web.sowa`, `impl/issuing.sowa` i pozostałych człowiek nie czyta. Pilnują go typy w `src/`, przykłady, `property` i mutacje.

## Po zatwierdzeniu

Agent dostaje z CI uwagę o literówce w komunikacie i poprawia `impl/web.sowa`. To przechodzi, bo `impl/web.sowa` nie ma właściciela.

Potem agent „przy okazji” zmienia atrapę, żeby 422 zwracała też dla pustej listy pozycji. `sowa check --ci` tego nie przepuszcza:

```
błąd: specyfikacja zmieniła się po zatwierdzeniu (zatwierdzone na 8c41d0e)
        impl/ksef_fake.sowa
      potrzebne ponowne zatwierdzenie PR
```

Przy ponownym zatwierdzeniu `sowa review --base main` pokaże tę zmianę w kategorii „usunięty test”, bo zmiana atrapy zmienia to, wobec czego przechodzą istniejące testy.
