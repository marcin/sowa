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

## Obliczenia i efekty

Obliczenia są czyste, a efekty siedzą w kilku wyznaczonych plikach. Dzięki temu logikę faktury (sumy, VAT, rabaty) da się testować bez bazy i sieci, a recenzent wie, gdzie szukać zapisów i wysyłki.

Które pliki mogą mieć jakie efekty, zapisaliśmy w `sowa.toml` w sekcji `[effects]`, więc pilnuje tego kompilator, a nie ten opis. Tu jest tylko uzasadnienie:

- `Net(mail)` tylko w `src/sending.sowa`, bo wysyłka jest osobnym krokiem ([decyzja 003](decyzje/003-wysylka-osobno.md)).
- `Clock` tylko w `src/issuing.sowa`: datę odczytujemy raz, przy wystawieniu, a dalej przekazujemy ją jako zwykłą wartość.
- Każda nowa funkcja, która zapisuje do bazy albo łączy się z siecią, wymaga zatwierdzenia przez człowieka (`approve = true`, lista w `effects.lock`). Zatwierdza się przez `sowa review`, a PR ze zmianą w `effects.lock` wymaga zgody właściciela z `.github/CODEOWNERS`.

Nowy efekt albo nowy plik z efektami to zmiana tego założenia: trzeba poprawić `sowa.toml` i opisać to tutaj.

## Błędy

- Każdy błąd, który może zobaczyć użytkownik, jest wariantem w typie wyniku (`IssueError`, `PaymentError`, `SendError`). Nie ma wyjątków.
- Błąd niesie tylko informację, co poszło nie tak. Treść komunikatu dla użytkownika powstaje w interfejsie, nie w logice.
- Dane z formularza zamieniamy na typy z warunkami (`as ... or return`) na samym początku, w `src/issuing.sowa`. Dalej kod pracuje już na sprawdzonych wartościach.

## Poza zakresem

Faktury korygujące, zaliczkowe, mechanizm podzielonej płatności, KSeF.
