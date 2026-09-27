# Sowa

Szkic języka programowania na erę AI: **kod pisze agent, człowiek zatwierdza specyfikację**.

Agent w godzinę napisze 800 linii kodu. Nikt ich uczciwie nie przeczyta, więc review kończy się na „wygląda OK”. Sowa odwraca ten układ: człowiek czyta i zatwierdza krótką specyfikację, a kod, którego nie czyta, nie może wyjść poza to, co zatwierdził. Pilnuje tego kompilator, a nie dobra wola agenta.

## Jak to wygląda

Moduł ma dwa pliki. W `src/` jest specyfikacja, którą zatwierdza człowiek:

```
// src/sending.sowa
fn send_invoice(invoice: Invoice) -> Sent | SendError
  effects Net(mail)

  doc uzytkownik/faktury.md#wysyłka-e-mailem
  why decyzje/003-wysylka-osobno.md
```

W `impl/` jest kod, który pisze agent i którego człowiek nie musi czytać:

```
// impl/sending.sowa
fn send_invoice(invoice: Invoice) -> Sent | SendError
  effects Net(mail)

  pdf = render_pdf(invoice)
  return try mail_send(mail, invoice.buyer.email, subject(invoice), pdf)
```

A `mail` to jeden adres zatwierdzony w `sowa.toml`:

```toml
[effects.resources]
mail = "smtp.firma.pl:587"
```

Jeśli agent w `impl/` spróbuje wysłać fakturę gdzie indziej, zapisać coś do bazy albo zwrócić nowy rodzaj błędu, kod się nie skompiluje. Żeby to zrobić, musi zmienić `src/`, a tego nie da się scalić bez zgody człowieka.

Zamiast diffu 800 linii człowiek dostaje listę decyzji, od najbardziej ryzykownej:

```
$ sowa review

Zmiany w specyfikacji (src/, docs/):

  [1] niebezpieczne   send_reminder  src/sending.sowa:24   nowa funkcja, effects Net(mail)
  [2] osłabienie      Percent        src/types.sowa:5      warunek: α <= 100  →  α <= 1000
  [3] usunięty test   totals         src/invoice.sowa:45   example totals([...], 0) == Totals(...)

Kod (impl/): 3 pliki, 120 linii, nie wymaga przeglądu.
  mutacje wykryte przez testy: 41 z 44
```

## Czego nie znaleźliśmy w innych językach

Szukaliśmy wśród popularnych języków i kilkudziesięciu projektów języków dla AI ([porównanie](docs/porownanie.md)). Pojedyncze elementy istnieją, ale nie takie połączenie i nie w tym celu.

1. **Specyfikacja zatwierdzana, kod odizolowany.** `src/` i `impl/` to nie tylko podział plików, jak `.mli` w OCamlu czy `.ads` w Adzie. To granica odpowiedzialności: człowiek zatwierdza `src/` przez CODEOWNERS, `impl/` jest w PR zwinięty, a kompilator nie pozwala mu wyjść poza specyfikację. Kod możesz też czytać, w wybranych modułach albo w całym projekcie. [Tryby](docs/tryby.md)
2. **Kompilator mówi, co wymaga decyzji człowieka.** Nowa funkcja z siecią, luźniejszy warunek w typie, usunięty test: `sowa review` wyciąga je z całej zmiany i ustawia na górze. Zwykły diff pokazuje to tak samo jak zmianę nazwy zmiennej. [Zatwierdzanie](docs/zatwierdzanie.md)
3. **Efekty przypięte do konkretnego adresu.** `effects Net(mail)` nie znaczy „sieć”, tylko „ten jeden serwer z `sowa.toml`”. Sprawdza to kompilator dla każdej funkcji, a runtime dodatkowo dla procesu. Deno ogranicza sieć tylko dla całego procesu, a języki z efektami (Koka, Unison) nie mówią dokąd.
4. **Polityka efektów projektu w jednym pliku.** `sowa.toml` mówi, jakie efekty wolno mieć w projekcie i w których plikach: sieć tylko w wysyłce, zegar tylko przy wystawianiu faktury. To zdanie z dokumentacji architektury, tyle że sprawdzane przy każdym buildzie.
5. **Testy, których agent nie osłabi.** Przykłady w `src/` są częścią specyfikacji. Agent nie poprawi testu, żeby przeszedł, bo to zmiana wymagająca zgody. Dane testowe generują się z typów, a wynik testów mutacyjnych człowiek widzi zamiast kodu.
6. **Dokumentacja, która nie kłamie.** Odnośniki z kodu do `.md` i `{Symbol}` w `.md` sprawdza kompilator. Przykłady w `.md` uruchamiają się jako testy. Gdy zmieni się sygnatura, `docs.lock` wskaże akapity, które trzeba przejrzeć.

## A jeśli chcesz czytać kod

Kod w Sowie ma być zrozumiały dla kogoś, kto zna tylko `if`, `return` i wywołania funkcji. Bez makr, bez magii, jeden sposób na jedną rzecz:

```
type Percent = Int(α >= 0 && α <= 100)

fn checkout(user: User, input: Int) -> Receipt | CheckoutError
  effects Net, Db.write

  pct = input as Percent or return InvalidDiscount
  amount = apply_discount(cart_total(user), pct)
  return try pay(user, amount)
```

Z samej sygnatury widać, że funkcja łączy się z siecią, zapisuje do bazy i może zwrócić `CheckoutError`, a `Percent` nigdy nie będzie spoza zakresu 0–100.

## Zobacz

- [examples/invoices/](examples/invoices/): cały mały projekt do wystawiania faktur w trybie `spec`. Najszybciej pokazuje, o co chodzi.
- [docs/tryby.md](docs/tryby.md): specyfikacja i kod osobno, izolacja, co blokuje które zagrożenie.
- [docs/zatwierdzanie.md](docs/zatwierdzanie.md): `sowa review`, pliki `.lock`, CODEOWNERS i kiedy to nie wystarcza.
- [docs/zalozenia.md](docs/zalozenia.md): zasady i ustalona składnia.
- [docs/przemyslenia.md](docs/przemyslenia.md): skąd te decyzje, odrzucone warianty, otwarte pytania, podobne projekty.
- [docs/porownanie.md](docs/porownanie.md): tabele porównawcze z popularnymi językami i z językami ery AI.
- [docs/ocena.md](docs/ocena.md): szczera ocena, mocne i słabe strony, następny krok.
- [examples/](examples/): krótkie przykłady składni.
- [editors/vscode/](editors/vscode/): kolorowanie składni w VS Code.

## Status

Na razie to projekt na papierze: specyfikacja i przykłady, bez parsera i kompilatora. Następny krok to `sowa check` dla efektów, specyfikacji i dokumentacji, sprawdzony na [examples/invoices](examples/invoices/) ([plan](docs/ocena.md#następny-krok)). Uwagi i krytyka mile widziane.
