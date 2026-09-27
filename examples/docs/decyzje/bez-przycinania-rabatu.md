# Rabatu spoza zakresu nie przycinamy

Dotyczy: {read_discount}

## Decyzja

Rabat spoza zakresu {Percent} kończy się błędem `InvalidDiscount`. Nie zamieniamy go po cichu na 0 ani na 100.

## Dlaczego

Przycinanie ukrywało literówki: ktoś wpisywał 150 zamiast 15 i dostawał 100% rabatu bez żadnego komunikatu.

## Konsekwencje

- Formularz musi pokazać błąd i pozwolić poprawić wartość.
- To samo dotyczy `or 0`: w wejściu od użytkownika nie używamy wartości domyślnej.
