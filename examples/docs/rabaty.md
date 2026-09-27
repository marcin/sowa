# Rabaty

Dokumentacja dla użytkownika. Odnośniki w nawiasach klamrowych sprawdza kompilator Sowy.

## Naliczanie rabatu

Przy zamówieniu możesz podać rabat procentowy. Dozwolona wartość: {Percent}.

Rabat odejmuje się od kwoty brutto ({apply_discount}). Jeśli podasz wartość
spoza zakresu, zamówienie nie przejdzie i zobaczysz komunikat o błędzie
({read_discount}). Wartość nie jest po cichu zaokrąglana do 0 ani do 100.

Przykład, który kompilator uruchamia jako test:

```sowa
apply_discount(200, 25) == 150
```

## Rabat zerowy

Rabat 0 oznacza brak rabatu. Kwota zostaje bez zmian.

```sowa
apply_discount(99, 0) == 99
```
