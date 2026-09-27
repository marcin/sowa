# Założenia projektu invoices

Ogólne zasady, które obowiązują w wielu plikach naraz. Pliki z kodem wskazują tu przez `why` w nagłówku albo przy typie.

Reguły są uproszczone na potrzeby przykładu.

## Kwoty

- Wszystkie kwoty są w złotych, typ `Money` to liczba dziesiętna, a nie zmiennoprzecinkowa.
- Ceny pozycji są netto. Brutto zawsze liczymy, nigdy nie zapisujemy ręcznie.
- Zaokrąglamy do grosza (`round(..., 2)`) w dwóch miejscach: wartość pozycji po rabacie i VAT w każdej stawce. Nigdzie indziej, żeby sumy się nie rozjeżdżały. Szczegóły w [decyzji 001](decyzje/001-vat-od-sumy-w-stawce.md).
- Jedna waluta. Faktur w walutach obcych nie obsługujemy.

## NIP

Kreski i spacje usuwamy przy wejściu ({read_nip}), a w systemie trzymamy tylko cyfry ({Nip}). Dzięki temu porównanie dwóch NIP-ów to zwykłe `==`.

## Obliczenia i uprawnienia

Obliczenia są czyste, a baza, zegar i poczta przychodzą do funkcji jako parametry (`db: Db`, `clock: Clock`, `mail: Mailer`). Dzięki temu logikę faktury (sumy, VAT, rabaty) da się testować bez bazy i sieci, a recenzent widzi w sygnaturze, co funkcja może zrobić.

Jakie zasoby ma program, stoi w `sowa.toml` w sekcji `[resources]`. Runtime przekazuje je do `main` ({main}), a dalej płyną tylko przez parametry, więc pilnuje tego kompilator, a nie ten opis. Tu jest tylko uzasadnienie:

- `Mailer` dostaje tylko {send_invoice}, bo wysyłka jest osobnym krokiem ([decyzja 003](decyzje/003-wysylka-osobno.md)).
- `Clock` dostaje tylko {issue_invoice}: datę odczytuje się raz, przy wystawieniu, a dalej przekazuje jako zwykłą wartość.
- Funkcje, które tylko czytają z bazy, dostają `DbRead` zamiast `Db`, np. {last_invoice_seq}.
- W aplikacji webowej komplet uprawnień ma tylko {handle}. Dalej każdy adres dostaje tylko to, czego potrzebuje: zapis formularza ({post_invoice}) nie dostaje `Mailer`, a strona faktury ({get_invoice}) dostaje tylko `DbRead`.

Nowy zasób albo nowa funkcja w `src/` z uprawnieniem to zmiana tego założenia. `sowa review` pokazuje ją na samej górze, a PR wymaga zgody właściciela z `.github/CODEOWNERS`.

## Błędy

- Każdy błąd, który może zobaczyć użytkownik, jest wariantem w typie wyniku (`IssueError`, `PaymentError`, `SendError`). Nie ma wyjątków.
- Błąd niesie tylko informację, co poszło nie tak. Treść komunikatu dla użytkownika powstaje w interfejsie, nie w logice: w {error_message} w `src/web.sowa`.
- Dane z formularza zamieniamy na typy z warunkami (`as ... or return`) na samym początku, w `src/issuing.sowa`. Dalej kod pracuje już na sprawdzonych wartościach.

## Poza zakresem

Faktury korygujące, zaliczkowe, mechanizm podzielonej płatności, KSeF.
