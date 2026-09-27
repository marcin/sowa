# 001. VAT liczymy od sumy netto w każdej stawce

Dotyczy: {totals}

## Decyzja

VAT na fakturze liczymy od sumy wartości netto pozycji w danej stawce i zaokrąglamy raz, na końcu. Nie liczymy VAT od każdej pozycji osobno.

## Dlaczego

Przy wielu tanich pozycjach zaokrąglanie każdej z osobna zawyża VAT. Trzy pozycje po 0,33 zł netto w stawce 23%:

| Sposób | VAT |
|---|---|
| od każdej pozycji: 3 × round(0,0759) = 3 × 0,08 | 0,24 zł |
| od sumy: round(0,99 × 0,23) = round(0,2277) | **0,23 zł** |

Ten przypadek jest testem. Kompilator uruchamia go tak jak `example` przy {totals}, więc zmiana sposobu liczenia wywali build:

```sowa
totals([
  Line(name: "A", quantity: 1, unit_net: 0.33, vat: Vat23),
  Line(name: "B", quantity: 1, unit_net: 0.33, vat: Vat23),
  Line(name: "C", quantity: 1, unit_net: 0.33, vat: Vat23),
], 0).vat == 0.23
```

## Konsekwencje

- VAT przy pojedynczej pozycji jest tylko informacyjny i może nie sumować się co do grosza do VAT faktury.
- Instrukcja dla użytkownika wyjaśnia to w sekcji „Sumy i zaokrąglenia”.
