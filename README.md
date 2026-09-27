# Sowa

Szkic języka programowania na erę AI: **kod pisze agent, człowiek go czyta i zatwierdza**.

Dlatego Sowa stawia na łatwość weryfikacji, a nie na wygodę pisania. Każda rzecz, którą czytelnik musi sprawdzić w dokumentacji, to koszt.

```
type Percent = Int(α >= 0 && α <= 100)

fn checkout(user: User, input: Int) -> Receipt | CheckoutError
  effects Net, Db.write

  pct = input as Percent or return InvalidDiscount
  amount = apply_discount(cart_total(user), pct)
  return try pay(user, amount)
```

Z samej sygnatury widać, że funkcja łączy się z siecią, zapisuje do bazy i może zwrócić `CheckoutError`. Nie trzeba czytać ciała.

## Pliki

- [docs/zalozenia.md](docs/zalozenia.md) – zasady i ustalona składnia
- [docs/przemyslenia.md](docs/przemyslenia.md) – skąd te decyzje, odrzucone warianty, otwarte pytania, podobne projekty
- [docs/zatwierdzanie.md](docs/zatwierdzanie.md) – jak człowiek zatwierdza zmiany agenta: `sowa review`, pliki `.lock`, CODEOWNERS
- [docs/porownanie.md](docs/porownanie.md) – tabele porównawcze z popularnymi językami i z językami ery AI
- [docs/ocena.md](docs/ocena.md) – szczera ocena: mocne i słabe strony, następny krok
- [examples/](examples/) – pierwsze przykłady kodu
- [examples/faktury/](examples/faktury/) – przykładowy projekt: kod, dokumentacja dla użytkownika i decyzje

## Status

Tylko projekt na papierze. Nie ma jeszcze parsera ani kompilatora.
