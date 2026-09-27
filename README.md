# Sowa

Szkic języka programowania na erę AI: **kod pisze agent, człowiek zatwierdza specyfikację**.

Agent w godzinę napisze 800 linii kodu. Nikt ich uczciwie nie przeczyta, więc review kończy się na „wygląda OK”. Sowa odwraca ten układ: człowiek czyta i zatwierdza krótką specyfikację, a kod, którego nie czyta, nie może wyjść poza to, co zatwierdził. Pilnuje tego kompilator, a nie dobra wola agenta.

## Jak to wygląda

Moduł ma dwa pliki. W `src/` jest specyfikacja, którą zatwierdza człowiek:

```
// src/sending.sowa
fn send_invoice(invoice: Invoice, mail: Mailer) -> Sent | SendError
  doc uzytkownik/faktury.md#wysyłka-e-mailem
  why decyzje/003-wysylka-osobno.md
```

W `impl/` jest kod, który pisze agent i którego człowiek nie musi czytać:

```
// impl/sending.sowa
fn send_invoice(invoice: Invoice, mail: Mailer) -> Sent | SendError
  body = mail_body(invoice)
  return try mail.send(invoice.buyer.email, subject(invoice), body)
```

`mail: Mailer` to uprawnienie: bez tego parametru funkcja nie ma czym wysłać e-maila. Uprawnienia nie da się utworzyć w kodzie, więc jedyny `Mailer` w programie to serwer zatwierdzony w `sowa.toml`:

```toml
[resources]
mail = { type = "Mailer", server = "smtp.firma.pl:587" }
```

Jeśli agent w `impl/` spróbuje wysłać fakturę gdzie indziej, zapisać coś do bazy albo zwrócić nowy rodzaj błędu, kod się nie skompiluje. Żeby to zrobić, musi zmienić `src/`, a tego nie da się scalić bez zgody człowieka.

Agent pracuje swobodnie, aż całość działa. Wtedy zamiast diffu 800 linii człowiek dostaje listę decyzji, od najbardziej ryzykownej:

```
$ sowa review --base main

Specyfikacja (src/, docs/, sowa.toml): 3 zmiany

  [1] uprawnienie     send_reminder  src/sending.sowa:24   nowa funkcja z Mailer (mail: smtp.firma.pl:587)
  [2] osłabienie      Percent        src/types.sowa:5      α <= 100  →  α <= 1000
                                                           dopuszcza np. 101
  [3] usunięty test   totals         src/invoice.sowa:50   property totals(lines, discount).net <= totals(lines, 0).net

Kod (impl/): 3 pliki, 120 linii, nie wymaga przeglądu.
```

Człowiek zatwierdza raz. Potem agent może jeszcze poprawiać `impl/`, ale każda zmiana w `src/` wymaga ponownego zatwierdzenia i pilnuje tego CI.

## Czego nie ma w innych językach

Porównanie objęło popularne języki i kilkadziesiąt projektów języków dla AI ([porównanie](docs/porownanie.md)). Najbliżej jest [Aver](https://github.com/jasisz/aver), z tą samą tezą: „AI pisze, człowiek przegląda kontrakty”. Tam przegląd jest jednak zaleceniem, a agent może po cichu zmienić kontrakt albo test. W Sowie zatwierdzanie jest egzekwowane:

1. **Specyfikacja to granica, a nie widok.** Człowiek zatwierdza `src/` przez CODEOWNERS, `impl/` jest w PR zwinięty, a kompilator nie pozwala kodowi wyjść poza specyfikację. W OCamlu (`.mli`) i Adzie (`.ads`) podział plików służy kompilacji, a w Sowie odpowiedzialności. Kod wybranego modułu możesz też czytać: wystarczy dopisać go do CODEOWNERS. [Specyfikacja i kod](docs/specyfikacja.md)
2. **Zatwierdzasz raz, działającą całość.** Nie czytasz pomysłu, który za godzinę się zmieni. Agent buduje, aż testy przejdą, a ty zatwierdzasz wersję końcową. Po zatwierdzeniu `sowa check --ci` pilnuje, że specyfikacja już się nie zmieni, a poprawki w `impl/` nie wymagają ponownej zgody. [Zatwierdzanie](docs/zatwierdzanie.md)
3. **Lista decyzji zamiast diffu.** Nowe uprawnienie, luźniejszy warunek w typie, usunięty test: `sowa review` wyciąga je z całej zmiany i ustawia na górze. Czy warunek jest luźniejszy, rozstrzyga solver i pokazuje kontrprzykład („dopuszcza 101”). Zwykły diff pokazuje to tak samo jak zmianę nazwy zmiennej.
4. **Uprawnienia jako zwykłe parametry.** `mail: Mailer` w sygnaturze znaczy „ten jeden serwer z `sowa.toml`”, a funkcja bez takich parametrów jest czysta. Nie trzeba osobnej konfiguracji architektury: z sygnatur w `src/` widać, który moduł może wysyłać, a który tylko liczy. Aver i Deno mają listę dozwolonych hostów, ale dla całego programu i dopiero w runtime. Języki z object capabilities (E, Pony, Austral) znają ten pomysł. Sowa łączy go z zatwierdzaniem: nowe uprawnienie zawsze jest na górze listy.
5. **Testy, których agent nie osłabi.** `example`, `property` i warunki wyniku (`-> Money(α <= total)`) w `src/` są częścią specyfikacji, więc agent nie poprawi testu, żeby przeszedł. Dane do `property` generują się z typów. W planie są testy mutacyjne, których wynik człowiek zobaczy zamiast kodu.
6. **Opisy, które nie zestarzeją się po cichu.** Doctesty (Rust, Elixir) sprawdzają kod w dokumentacji, ale nie tekst. Gdy zmieni się sygnatura, `docs.lock` wskaże akapity, które trzeba przejrzeć. Odnośniki z kodu do `.md` i `{Symbol}` w `.md` sprawdza kompilator.
7. **Proces, którego nie da się obejść po cichu.** `sowa check` sprawdza też sam proces: czy CODEOWNERS obejmuje `src/`, `docs/`, `sowa.toml` i `docs.lock`. Bez tego wszystkie powyższe zabezpieczenia byłyby tylko umową.

## A jeśli chcesz czytać kod

Kod w Sowie ma być zrozumiały dla kogoś, kto zna tylko `if`, `return` i wywołania funkcji. Bez makr, bez magii, jeden sposób na jedną rzecz:

```
type Percent = Int(α >= 0 && α <= 100)

fn checkout(user: User, input: Int, bank: Http, db: Db) -> Receipt | CheckoutError
  pct = input as Percent or return InvalidDiscount
  amount = apply_discount(cart_total(user.cart), pct) as Price or return EmptyCart
  return try pay(user, amount, bank, db)
```

Z samej sygnatury widać, że funkcja łączy się z bankiem, zapisuje do bazy i może zwrócić `CheckoutError`, a `Percent` nigdy nie będzie spoza zakresu 0–100.

## Zobacz

- [examples/invoices/](examples/invoices/): cały mały projekt do wystawiania faktur. Najszybciej pokazuje, o co chodzi.
- [examples/fakturownia_web/](examples/fakturownia_web/): aplikacja webowa napisana tak, jakby pisał ją agent: wystawienie, zapis w bazie, wysyłka do KSeF. W [PR.md](examples/fakturownia_web/PR.md) jest wynik `sowa review`, z którego człowiek ją zatwierdza.
- [examples/ttfx_decrypt/](examples/ttfx_decrypt/): efekt `decrypt` z [ttfx](https://github.com/omacom/ttfx) przepisany na Sowę. Wyjście jest identyczne z ttfx bajt w bajt, a przykład służy do pomiaru szybkości (niżej). [examples/ttfx_decrypt_fast/](examples/ttfx_decrypt_fast/) to ten sam efekt z inną pętlą animacji: w każdej klatce przelicza tylko zmienione wiersze. W Ruście jest ok. 1,3–1,6 raza szybszy od ttfx.
- [docs/specyfikacja.md](docs/specyfikacja.md): specyfikacja i kod osobno, izolacja, uprawnienia, co blokuje które zagrożenie.
- [docs/zatwierdzanie.md](docs/zatwierdzanie.md): zatwierdzenie na końcu, `sowa review`, `docs.lock`, CODEOWNERS i kiedy to nie wystarcza.
- [docs/zalozenia.md](docs/zalozenia.md): zasady i ustalona składnia.
- [docs/gramatyka.md](docs/gramatyka.md): gramatyka na jedną stronę: wcięcia, deklaracje, instrukcje i pierwszeństwo operatorów.
- [docs/przemyslenia.md](docs/przemyslenia.md): skąd te decyzje, odrzucone warianty, otwarte pytania, podobne projekty.
- [docs/porownanie.md](docs/porownanie.md): tabele porównawcze z popularnymi językami i z językami ery AI.
- [docs/kompilacja.md](docs/kompilacja.md): dwie drogi kompilacji (Rust i testowo JS dla Buna), ich czasy, Cranelift i inne możliwe cele.
- [docs/ocena.md](docs/ocena.md): szczera ocena, mocne i słabe strony, następny krok.
- [examples/](examples/): krótkie przykłady składni.
- [editors/vscode/](editors/vscode/): kolorowanie składni w VS Code.

## Szybkość

Program w Sowie kompiluje się do Rusta, a do testów i porównań także do JavaScriptu dla Buna (`--bun`). Backend Rust zna typy w kompilacji. Wartość czytaną ostatni raz przenosi zamiast ją klonować, więc `map` i `with` zmieniają listę i rekord w miejscu, jak w Koka, Lean i Roc.

[examples/ttfx_decrypt](examples/ttfx_decrypt/) robi dokładnie tę samą pracę co ttfx, czyli Rust pisany ręcznie. Wejście to 60 wierszy tekstu, a wynik 1664 klatki, razem 44 MB wyjścia (macOS, Apple Silicon, mediana z 5 uruchomień):

| Program | Czas | RAM |
|---|---|---|
| ttfx (Rust pisany ręcznie) | 59 ms | 28 MB |
| Sowa → Rust | 53 ms | 14 MB |
| Sowa → Bun | 2,48 s | 406 MB |

Na wejściu 4 razy większym Sowa → Rust jest ok. 12% wolniejsza od ttfx. Szczegóły, zgodność bajt w bajt i sposób powtórzenia pomiaru są w [porownanie.md](examples/ttfx_decrypt/docs/porownanie.md).

[examples/ttfx_decrypt_fast](examples/ttfx_decrypt_fast/) daje to samo wyjście, ale w każdej klatce przelicza tylko wiersze w trakcie sceny, a tekst składa tylko ze zmienionych wierszy. W Ruście liczy 60 wierszy w 41 ms zamiast 55 ms (ttfx), a 180 wierszy w 130 ms zamiast 213 ms, czyli ok. 1,3–1,6 raza szybciej. W Bunie jest wolniejszy od pierwszej wersji. Szczegóły są w [porownanie.md](examples/ttfx_decrypt_fast/docs/porownanie.md).

## Status

Na razie to głównie projekt na papierze: specyfikacja i przykłady. [compiler/](compiler/) to kompilator do Rusta (testowo także do JavaScriptu dla Buna): `sowa check`, `sowa test` i `sowa run` działają na [examples/fakturownia_web](examples/fakturownia_web/), [examples/invoices](examples/invoices/), [examples/ttfx_decrypt](examples/ttfx_decrypt/) i [examples/ttfx_decrypt_fast](examples/ttfx_decrypt_fast/). Działają też `sowa review --base main` z małym solverem, który podaje kontrprzykłady, `docs.lock` i `sowa check --ci` z zatwierdzeniem z GitHuba ([przykład wyniku](examples/fakturownia_web/PR.md)). Nie ma jeszcze dowodzenia warunków wyniku, testów mutacyjnych ani zatwierdzania podpisanym commitem. Uwagi i krytyka mile widziane.
