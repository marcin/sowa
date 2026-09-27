# Założenia projektu faktury

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

Obliczenia są czyste, a efekty siedzą w kilku wyznaczonych plikach:

| Plik | Efekty |
|---|---|
| `src/types.sowa`, `src/invoice.sowa` | brak |
| `src/numbering.sowa` | `Db.read`, `Db.write` |
| `src/issuing.sowa` | `Db.read`, `Db.write`, `Clock` |
| `src/payments.sowa` | `Db.write` |
| `src/sending.sowa` | `Net` |

Nowy efekt w innym pliku to zmiana tego założenia i trzeba ją opisać tutaj.

## Błędy

- Każdy błąd, który może zobaczyć użytkownik, jest wariantem w typie wyniku (`IssueError`, `PaymentError`, `SendError`). Nie ma wyjątków.
- Błąd niesie tylko informację, co poszło nie tak. Treść komunikatu dla użytkownika powstaje w interfejsie, nie w logice.
- Dane z formularza zamieniamy na typy z warunkami (`as ... or return`) na samym początku, w `src/issuing.sowa`. Dalej kod pracuje już na sprawdzonych wartościach.

## Poza zakresem

Faktury korygujące, zaliczkowe, mechanizm podzielonej płatności, KSeF.
