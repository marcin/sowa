# Szczera ocena (27.09.2026)

Stan: specyfikacja składni i 6 przykładów, bez parsera i kompilatora.

**W skrócie:** jako pomysł Sowa jest spójna i ma dobrą tezę. Jako język jeszcze nie istnieje, a najtrudniejsza praca jest przed nami.

## Co jest dobre

- **Jasne kryterium, stosowane konsekwentnie.** „Jak najmniej wiedzy do przeczytania” rozstrzygało każdą decyzję: `> 0` zamiast `.positive?`, `&&` zamiast `0 < x < 100`, stałe bez `let`, `or` zamiast wiszącego `else`. Rzadko który projekt ma takie kryterium, a to jest najcenniejsza część Sowy.
- **Sygnatura mówi wszystko.** Z jednej linii widać, co funkcja przyjmuje, jakie są ograniczenia, jakie ma efekty i jakie błędy może zwrócić. To realnie rozwiązuje problem recenzji kodu napisanego przez agenta.
- **Mało pojęć.** Póki co da się wytłumaczyć cały język na jednej stronie.

## Co jest słabe

### 1. Zajmowaliśmy się głównie składnią, a to najłatwiejsza część

Większość dyskusji dotyczyła tego, czy użyć `α`, `$` czy `x`. Tymczasem to dokładnie ten błąd, przed którym ostrzegał Valim. Nie mamy nic o tym, co jest naprawdę trudne:

- generyki,
- moduły,
- współbieżność,
- model pamięci,
- to, co dokładnie kompilator potrafi udowodnić.

### 2. Obietnica „kompilator nie przepuści” może się nie utrzymać

Warunki typu `α >= 0 && α <= 100` są łatwe dla solvera. Ale już `total - total * pct / 100` to mnożenie zmiennych (arytmetyka nieliniowa), z którym Z3 często sobie nie radzi. Doświadczenia LiquidHaskella i Fluxa są takie, że programista co chwilę dostaje „nie można udowodnić” i albo się frustruje, albo wszystko obchodzi sprawdzeniem w runtime.

Widać to w naszym własnym przykładzie. W `examples/05_checkout.sowa` jest `apply_discount(...) as Price or return EmptyCart`, czyli sprawdzenie w runtime, bo udowodnienie, że wynik jest większy od zera, wymagałoby kontraktu `ensures`, którego jeszcze nie mamy.

### 3. Efekty nie skalują się tak prosto, jak wyglądają

Co z `items.map(f)`, gdy `f` łączy się z siecią? Wtedy `map` musi być generyczny po efektach. Koka rozwiązuje to przez wiersze efektów, ale sygnatury robią się wtedy mało czytelne. Nikt jeszcze nie pokazał prostego zapisu. Jeśli go nie wymyślimy, nasze główne kryterium zderzy się z efektami przy pierwszej funkcji wyższego rzędu.

### 4. Czytelność to słaba przewaga konkurencyjna

Vera, Thermite i Prove mają już kontrakty, efekty i działające kompilatory (zob. [porownanie.md](porownanie.md)). Wyróżnia nas składnia, a składnię najłatwiej skopiować.

### 5. Brak ekosystemu przesądza sprawę na produkcji

Do SaaS-a potrzeba HTTP, bazy, JSON-a, kolejek i autoryzacji. Nikt nie wybierze języka, w którym trzeba to wszystko napisać od zera.

### 6. Otwarte dziury w tym, co już jest

- zaokrąglanie przy `Money / 100`,
- `α` w zagnieżdżonych warunkach,
- `or 0`, które może po cichu połykać błędne dane wejściowe.

## Ocena

| | |
|---|---|
| jako ćwiczenie projektowe i specyfikacja pomysłu | bardzo dobrze: spójna, przemyślana, z uzasadnieniami |
| jako język do użycia | na razie zero, bo nie istnieje nic poza papierem |
| szansa na zostanie popularnym językiem | niska, jak dla każdego z 42 projektów w katalogu |

## Proponowany następny krok

Zamiast dalej dopracowywać składnię, sprawdzić tezę w praktyce:

1. **Mały checker** dla podzbioru języka: typy z warunkami (tylko arytmetyka liniowa), efekty, `as ... or` i `match`.
2. **Kompilacja do TypeScripta.** Ekosystem npm dostajemy za darmo, a problem nr 5 znika. Thermite robi to samo z Rustem.
3. **Test na przykładach z `examples/`.** Po nim będzie wiadomo, gdzie checker sobie radzi, a gdzie mówi „nie można udowodnić”.

Taki prototyp pokaże w kilka dni, czy Sowa ma sens, lepiej niż kolejne tygodnie dyskusji o symbolach.
