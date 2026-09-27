# Założenia projektu fakturownia_web

Ogólne zasady, które obowiązują w wielu plikach naraz. Pliki z kodem wskazują tu przez `why` w nagłówku albo przy typie.

Reguły są uproszczone na potrzeby przykładu, a ksef.pl to serwer wymyślony na potrzeby przykładu, z własnym formatem JSON. To nie jest prawdziwy KSeF.

## Kwoty

- Kwoty są w złotych, typ `Money` to liczba dziesiętna, a nie zmiennoprzecinkowa.
- Ceny pozycji są netto. Brutto zawsze się liczy ({totals}), nigdy nie zapisuje ręcznie.
- Zaokrąglenie do grosza jest w dwóch miejscach: wartość pozycji ({line_net}) i VAT w każdej stawce.

## NIP

Kreski i spacje usuwa się przy wejściu ({read_nip}), a w systemie jest tylko 10 cyfr ({Nip}).

## Numeracja

- Numer ma format `FV/rok/kolejny numer`, np. `FV/2026/0042` ({InvoiceNumber}). Typ opisuje cały format, a nie tylko prefiks, więc adres faktury da się zamienić z powrotem na numer.
- Numery idą po kolei w ramach roku i nie mają luk. Numer nadaje się w tej samej transakcji co zapis faktury ({save_new_invoice}), więc błąd przy zapisie nie zużywa numeru.
- Wystawionej faktury nie da się usunąć ani zmienić. Zmienia się tylko status: z `Issued` na `SentToKsef`, nigdy odwrotnie ({mark_sent}).

## Dane z formularza

- Formularz ({InvoiceForm}) to sam tekst. Odczyt formularza może się nie udać tylko wtedy, gdy brakuje pola.
- Sprawdzanie jest osobno ({read_buyer}, {read_line}), żeby każdy błąd wskazywał pole, np. „Pozycja 2: nieprawidłowa ilość”. Samo `as` powiedziałoby tylko „nie pasuje”.
- Dalej kod pracuje już na typach z warunkami (`Name`, `Nip`, `Quantity`).

## Uprawnienia

Obliczenia są czyste, a baza, zegar, KSeF i serwer przychodzą jako parametry. Zasoby są w `sowa.toml` w `[resources]`, a runtime przekazuje je do {main}. Tu jest uzasadnienie, kto co dostaje:

- Komplet uprawnień ma tylko {handle}. Każdy adres dostaje dalej tylko to, czego potrzebuje.
- `Http` do ksef.pl ma tylko {send_to_ksef} (i funkcje, które je wywołują: {post_ksef}, {handle}). Wystawienie faktury ({issue_invoice}, {post_invoice}) nie ma `Http`, więc niczego nie wysyła ([decyzja 001](decyzje/001-ksef-osobnym-krokiem.md)).
- Strony, które tylko pokazują dane ({get_invoice}, {get_list}), dostają `DbRead`, więc niczego nie zapiszą.

Nowy zasób albo nowa funkcja w `src/` z uprawnieniem to zmiana tego założenia. `sowa review` pokazuje ją na samej górze.

## Błędy

- Błąd, który może zobaczyć użytkownik, to wariant w typie wyniku (`IssueError`, `KsefError`). Nie ma wyjątków.
- Błąd niesie tylko dane o tym, co poszło nie tak, np. `InvalidLine(position: 2, field: LineQuantity)`. Komunikat dla użytkownika powstaje w jednym miejscu, w {error_message}.
- Błąd KSeF nie cofa wystawienia faktury. Faktura zostaje w statusie `Issued` i można wysłać ją ponownie.

## Testy

- Testy działają na zasobach z `[resources.test]`: pusta baza w pamięci, zegar zatrzymany na 27.09.2026 10:00 i atrapa ksef.pl ({ksef_fake}). Żaden test nie łączy się z siecią.
- Atrapa opisuje założenia o ksef.pl i jest częścią specyfikacji. Jej kod człowiek czyta, tak jak numerację.
- Cały przepływ (formularz, zapis, wysyłka) sprawdza przykład w [instrukcji](uzytkownik/faktury.md#wysyłka-do-ksef).

## Poza zakresem

- Logowanie i uprawnienia użytkowników. Dziś każdy, kto otworzy adres, może wystawić fakturę, więc aplikacja nadaje się tylko do sieci wewnętrznej.
- Ochrona przed CSRF, faktury korygujące, e-mail do nabywcy, prawdziwy format KSeF (XML FA).
