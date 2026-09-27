# Faktury

Instrukcja dla użytkownika. Odnośniki w nawiasach klamrowych, np. {Percent}, sprawdza kompilator Sowy, a typy z warunkami renderują się jako opis warunku. Bloki kodu `sowa` uruchamiają się jako testy.

## Wystawianie faktury

Aby wystawić fakturę ({issue_invoice}), podaj:

- nazwę nabywcy,
- NIP nabywcy ({Nip}); możesz wpisać go z kreskami lub spacjami, np. `123-456-32-18`,
- adres e-mail nabywcy, na który wyślemy fakturę,
- co najmniej jedną pozycję,
- opcjonalnie rabat ({Percent}).

Faktura dostaje numer w chwili zapisu, np. `FV/2026/0042` ({InvoiceNumber}). Numery idą po kolei w ramach roku i nie mają luk. Wystawionej faktury nie da się usunąć.

Jeśli czegoś brakuje albo dane są błędne, faktura nie zostanie wystawiona, a numer nie zostanie zużyty.

## Stawki VAT

Każda pozycja ma jedną stawkę ({VatRate}): 23%, 8%, 5%, 0% albo „zw.” (zwolniona). Pozycja zwolniona liczy się w sumach jak 0%.

```sowa
vat_percent(Vat8) == 8
```

## Rabat

Rabat ({Percent}) dotyczy całej faktury i odejmuje się go od wartości netto każdej pozycji ({line_net}). Wartość po rabacie zaokrąglamy do grosza.

```sowa
line_net(Line(name: "Usługa", quantity: 3, unit_net: 100, vat: Vat23), 20) == 240
```

## Sumy i zaokrąglenia

Na fakturze widzisz sumę netto, VAT i brutto ({totals}). VAT liczymy od sumy wartości netto w każdej stawce, a nie od każdej pozycji osobno. Dlatego VAT na fakturze może się różnić o grosz od sumy VAT-ów policzonych w pamięci dla każdej pozycji.

```sowa
totals([Line(name: "Usługa", quantity: 1, unit_net: 100, vat: Vat23)], 0).gross == 123
```

## Płatność

Fakturę oznaczamy jako opłaconą ({mark_paid}), gdy wpłata jest równa kwocie brutto. Wpłata w innej kwocie nie zmienia statusu. Opłaconej faktury nie można opłacić drugi raz.

## Wysyłka e-mailem

Fakturę wysyłamy e-mailem na adres nabywcy ({send_invoice}). Wysyłka jest osobnym krokiem: jeśli e-mail nie dojdzie, faktura nadal jest wystawiona i można wysłać ją ponownie.
