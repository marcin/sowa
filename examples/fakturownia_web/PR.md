# PR: fakturownia_web

Przykład, jak wygląda PR, w którym agent AI napisał całą aplikację, a człowiek ją zatwierdza. Wynik `sowa review` niżej pochodzi z kompilatora z [compiler/](../../compiler), a numery linii zgadzają się z plikami w tym katalogu. Dowodzenia warunków wyniku i testów mutacyjnych kompilator jeszcze nie ma, więc w wyniku ich nie ma.

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

Prawdziwy wynik kompilatora dla PR, który dodaje ten katalog, przy pustym `docs.lock` (`main` bez projektu). W katalogu `docs.lock` jest już wypełniony, czyli taki, jaki zostaje po przejrzeniu opisów.

```
$ sowa review --base main

Specyfikacja (src/, docs/, sowa.toml): nowy projekt, 19 zmian, reszta zwykła

  [1] uprawnienie     db                sowa.toml:14          nowy zasób Db: sqlite:fakturownia.db
  [2] uprawnienie     clock             sowa.toml:15          nowy zasób Clock
  [3] uprawnienie     ksef              sowa.toml:19          nowy zasób Http: https://ksef.example/api, token z KSEF_TOKEN
  [4] uprawnienie     web               sowa.toml:20          nowy zasób Server: nasłuch 0.0.0.0:8080
  [5] uprawnienie     find_invoice      src/storage.sowa:13   nowa funkcja z DbRead (db: sqlite:fakturownia.db)
  [6] uprawnienie     get_invoice       src/web.sowa:50       nowa funkcja z DbRead (db: sqlite:fakturownia.db)
  [7] uprawnienie     get_list          src/web.sowa:43       nowa funkcja z DbRead (db: sqlite:fakturownia.db)
  [8] uprawnienie     handle            src/web.sowa:37       nowa funkcja z Db, Clock, Http (db: sqlite:fakturownia.db, clock, ksef: https://ksef.example/api)
  [9] uprawnienie     issue_invoice     src/issuing.sowa:42   nowa funkcja z Db, Clock (db: sqlite:fakturownia.db, clock)
 [10] uprawnienie     list_invoices     src/storage.sowa:16   nowa funkcja z DbRead (db: sqlite:fakturownia.db)
 [11] uprawnienie     main              src/main.sowa:4       nowa funkcja z Db, Clock, Http, Server (db: sqlite:fakturownia.db, clock, ksef: https://ksef.example/api, web: 0.0.0.0:8080)
 [12] uprawnienie     mark_sent         src/storage.sowa:22   nowa funkcja z Db (db: sqlite:fakturownia.db)
 [13] uprawnienie     post_invoice      src/web.sowa:46       nowa funkcja z Db, Clock (db: sqlite:fakturownia.db, clock)
 [14] uprawnienie     post_ksef         src/web.sowa:52       nowa funkcja z Db, Clock, Http (db: sqlite:fakturownia.db, clock, ksef: https://ksef.example/api)
 [15] uprawnienie     save_new_invoice  src/storage.sowa:8    nowa funkcja z Db (db: sqlite:fakturownia.db)
 [16] uprawnienie     send_to_ksef      src/ksef.sowa:37      nowa funkcja z Http, Db, Clock (ksef: https://ksef.example/api, db: sqlite:fakturownia.db, clock)

      DbRead (db) mają tylko: find_invoice, list_invoices, get_list, get_invoice
      Db (db) mają tylko: issue_invoice, send_to_ksef, main, save_new_invoice, mark_sent, handle, post_invoice, post_ksef
      Clock (clock) mają tylko: issue_invoice, send_to_ksef, main, handle, post_invoice, post_ksef
      Http (ksef) mają tylko: send_to_ksef, main, handle, post_ksef
      Server (web) ma tylko: main

 [17] usunięty test   db                sowa.toml:26          nowy zasób testowy Db: w pamięci
 [18] usunięty test   clock             sowa.toml:27          nowy zasób testowy Clock: zegar zatrzymany na 2026-09-27T10:00:00
 [19] usunięty test   ksef              sowa.toml:28          nowy zasób testowy Http: atrapa ksef_fake
      zwykłe: 26 typów, 16 czystych funkcji, 26 example, 5 property, 2 bloki sowa w docs/

Kod (impl/): 8 plików, 313 linii.
      do przeczytania (CODEOWNERS): impl/ksef_fake.sowa (12 linii), impl/storage.sowa (40 linii)
      reszta nie wymaga przeglądu

Opisy do przejrzenia (docs.lock):
      decyzje/001-ksef-osobnym-krokiem.md: cały plik, #001-wysyłka-do-ksef-jest-osobnym-krokiem, #decyzja, #dlaczego, #konsekwencje
      decyzje/002-ponowna-wysylka.md: cały plik, #002-ponowna-wysyłka-do-ksef-jest-bezpieczna, #jak-tego-pilnować
      uzytkownik/faktury.md: #faktury, #stawki-vat, #sumy, #wystawianie-faktury, #wysyłka-do-ksef
      zalozenia.md: #błędy, #dane-z-formularza, #kwoty, #nip, #numeracja, #testy, #uprawnienia
      po przejrzeniu: sowa review --confirm-docs
```

## Co sprawdza człowiek

Najpierw uruchamia aplikację (`sowa run --fake ksef examples/fakturownia_web`, potem http://localhost:8080) i przechodzi cały przepływ: formularz z błędną ilością, poprawna faktura, wysyłka, drugi klik. Potem czyta wynik od góry:

1. **Zasoby [1]–[4].** Czy program ma łączyć się tylko z tą bazą i z ksef.pl. `web` nasłuchuje na `0.0.0.0:8080`, a aplikacja nie ma logowania ([poza zakresem](docs/zalozenia.md#poza-zakresem)). Tu człowiek decyduje, czy to ma być `127.0.0.1`.
2. **Kto ma `Http`.** Linia „Http (ksef) mają tylko” pokazuje jeden łańcuch: `main`, `handle`, `post_ksef`, `send_to_ksef`. `post_invoice` i `issue_invoice` go nie mają, więc wystawienie niczego nie wysyła, i to bez czytania `impl/`.
3. **Kto ma `Db`, a kto `DbRead` [5]–[16].** Strony, które tylko pokazują (`get_list`, `get_invoice`), nie zapiszą niczego.
4. **Atrapa [19].** Czyta 12 linii `impl/ksef_fake.sowa` i potwierdza założenie z decyzji 002 albo je odrzuca. Tego żaden solver nie rozstrzygnie, bo dotyczy cudzego serwera.
5. **Numeracja.** Czyta `impl/storage.sowa`: licznik i faktura w jednej transakcji.
6. **Opisy.** Czyta sekcje z listy, a potem uruchamia `sowa review --confirm-docs`. Polecenie zapisuje w `docs.lock` bieżące hashe z jego adresem z `git config user.email`. Commit z `docs.lock` idzie do tego samego PR.

Osłabień i rozszerzeń nie ma, bo projekt jest nowy: nie ma starego warunku, z którym można by porównać. Kodu w `impl/web.sowa`, `impl/issuing.sowa` i pozostałych człowiek nie czyta. Pilnują go typy w `src/`, przykłady i `property`.

## Po zatwierdzeniu

Agent dostaje z CI uwagę o literówce w komunikacie i poprawia `impl/web.sowa`. To przechodzi, bo `impl/web.sowa` nie ma właściciela.

Potem agent „przy okazji” zmienia atrapę, żeby 422 zwracała też dla pustej listy pozycji. `sowa check --ci` tego nie przepuszcza:

```
błąd: specyfikacja zmieniła się po zatwierdzeniu (zatwierdzone na 8c41d0e)
        impl/ksef_fake.sowa
      potrzebne ponowne zatwierdzenie PR
```

Przy ponownym zatwierdzeniu `sowa review --base 8c41d0e` pokazuje tylko tę zmianę, w kategorii „usunięty test” („zmienione ciało atrapy”), bo zmiana atrapy zmienia to, wobec czego przechodzą istniejące testy.
