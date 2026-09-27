# Szczera ocena (27.09.2026)

## Aktualizacja: uprawnienia i zatwierdzenie na końcu (27.09.2026)

Po poprzedniej ocenie zmieniło się pięć rzeczy: uprawnienia jako parametry zamiast `effects`, jedno zatwierdzenie na PR, gdy całość działa, `sowa review` z solverem, warunki wyniku i `property` oraz mniej konfiguracji (bez trybów, `effects.lock`, `approve` i `[review.files]`).

### Co się zmieniło na plus

- **Mniej do nauczenia.** `sowa.toml` ma trzy krótkie sekcje (`[project]`, `[limits]`, `[resources]`), a zamiast dwóch plików `.lock` jest jeden, `docs.lock`. Punkt 2 z poprzedniej listy („proces może urosnąć w biurokrację”) jest w dużej części rozwiązany.
- **Uprawnienia nie potrzebują nowej składni.** To zwykłe parametry. Znika problem funkcji wyższego rzędu (lambda przechwytuje uprawnienie) i znika osobna mapa „który plik co może”, bo widać to z sygnatur.
- **Zmęczenie zatwierdzaniem jest mniejsze.** Nie ma zatwierdzania funkcji po kolei. Człowiek zatwierdza raz, działającą zmianę, a lista w `sowa review` ma stałą kolejność od najbardziej ryzykownych.
- **Osłabienie specyfikacji widać zawsze.** Solver porównuje warunki z `main` i pokazuje kontrprzykład. To rozbraja główną pułapkę zatwierdzania gotowego kodu: specyfikację dopasowaną do kodu.
- **Mocniejsze testy w specyfikacji.** `property` i warunki wyniku mówią więcej niż trzy przykłady, a agent ich nie ruszy bez zgody.

### Co niepokoi

1. **Solver jest teraz w środku obietnicy.** Wcześniej przewaga nie zależała od solvera. Teraz `sowa review` potrzebuje go do rozpoznania osłabień. Gdy solver nie rozstrzyga, zmiana idzie do osłabień, więc przy trudnych warunkach lista może być pełna fałszywych alarmów.
2. **Strażnik zależy od API GitHuba.** `sowa check --ci` musi wiedzieć, na którym commicie człowiek zatwierdził PR. Na GitLabie i innych platformach trzeba to zrobić osobno.
3. **Przekazywanie uprawnień przez wiele warstw.** Sygnatury się wydłużają. Agent może zacząć dawać `Db` wszędzie „na zapas”, co osłabia sens sygnatur. Potrzebne jest ostrzeżenie o nieużywanym uprawnieniu.
4. **Warunki wyniku w runtime.** Warunek, którego solver nie udowodni, zamienia się w sprawdzenie w runtime. Niespełniony na produkcji to błąd programu, a decyzja, co wtedy robić, jest otwarta.
5. **Drugi agent jako recenzent kusi, żeby wyłączyć człowieka.** Opcja jest ograniczona kategoriami z `sowa review`, ale zespół może chcieć ją poszerzyć. Wtedy wraca zatwierdzanie bez czytania, tylko przez model.

### Następny krok

1. parser dla `fn`, `type`, `desc`, `doc`, `why`, `example`, `property` i warunków wyniku,
2. sprawdzanie uprawnień: nie da się ich utworzyć, płyną tylko przez parametry, `impl/` ma te same sygnatury co `src/`,
3. `sowa review --base main` z kategoriami, na początek z solverem tylko dla arytmetyki liniowej,
4. `sowa check --ci` z odczytem zatwierdzenia z API GitHuba,
5. tłumaczenie `example`, `property` i bloków `sowa` do TypeScripta i uruchamianie ich, warunki wyniku na razie tylko w runtime.

## Aktualizacja: po dokumentacji i zatwierdzaniu (27.09.2026)

Stan: specyfikacja, 7 przykładów i przykładowy projekt [invoices](../examples/invoices/), nadal bez parsera i kompilatora.

**W skrócie:** jest wyraźnie lepiej niż przy pierwszej ocenie, ale z innego powodu, niż zakładaliśmy na początku.

### Co się zmieniło na plus

- **Mocniejsza teza.** W pierwszej ocenie czytelna składnia wypadła jako słaba przewaga, bo łatwo ją skopiować. Teraz Sowa ma coś lepszego: kompilator sam wskazuje człowiekowi, które zmiany agenta wymagają jego decyzji. Składają się na to efekty w sygnaturach, polityka efektów w `sowa.toml`, `approve` i wykrywanie nieaktualnych opisów. Tego nie znaleźliśmy nigdzie indziej (zob. [zatwierdzanie.md](zatwierdzanie.md)). To odpowiada na realny problem: ktoś ma przejrzeć 800 linii kodu z agenta i musi wiedzieć, na które 3 patrzeć.
- **Ta przewaga nie zależy od najtrudniejszej części.** Typy z warunkami potrzebują solvera, który może zawieść na arytmetyce nieliniowej. Sprawdzanie efektów to prosta analiza grafu wywołań, którą da się zbudować w kilka dni i która działa zawsze.
- **Dokumentacja jest wpięta w kod.** `desc`, `doc`, `why`, `{Symbol}`, przykłady w `.md` uruchamiane jako testy i `docs.lock` tworzą spójny system, a nie zbiór pomysłów.

### Co niepokoi

1. **Najlepszy pomysł może nie potrzebować nowego języka.** Politykę efektów i zatwierdzanie da się zrobić jako narzędzie do TypeScripta: adnotacje, linter importów, pliki `.lock`. Byłoby to mniej szczelne (w TS nie udowodnisz, że funkcja nie woła `fetch`), ale dla wielu zespołów wystarczające. Trzeba uczciwie odpowiedzieć, po co nowy język. Odpowiedź: efekty są w systemie typów, więc gwarancja jest pełna, a nie „prawie”. Trzeba to jednak pokazać, a nie założyć.
2. **Wiedza do nauczenia rośnie, tylko w innym miejscu.** Kod czyta się łatwo, ale projekt ma już `sowa.toml` z czterema sekcjami, dwa pliki `.lock`, CODEOWNERS, `sowa review` i limity. Kryterium „jak najmniej wiedzy” stosowaliśmy do składni, a nie do procesu.
3. **Zmęczenie zatwierdzaniem.** Jeśli każdy `Db.write` wymaga zgody, po tygodniu ludzie zatwierdzają bez czytania, jak banery cookies. `approve` powinno zostać dla rzadkich efektów. W projekcie invoices wystarczyłoby na `Net`.
4. **CODEOWNERS nie chroni, gdy agent działa na koncie człowieka.** Autor PR i właściciel to wtedy ta sama osoba. Działa to tylko przy osobnym koncie dla agenta i ochronie `main` bez wyjątku dla adminów. Inaczej zostaje podpis kluczem, który wymaga potwierdzenia przy każdym użyciu. Opisane w [zatwierdzanie.md](zatwierdzanie.md#kiedy-codeowners-nie-wystarcza).
5. **Stara słabość wróciła.** Znowu dyskutowaliśmy o powierzchni (`see`, cudzysłowy, `desc`). Otwartych pytań przybywa szybciej, niż ubywa, jest ich ponad 30. Projekt invoices używa składni spoza specyfikacji: rekordów, generyków, `with`, `transaction`. „Cały język na jednej stronie” przestaje być prawdą przy pierwszym prawdziwym programie.

### Ocena

| | pierwsza ocena | teraz |
|---|---|---|
| teza | czytelność (słaba przewaga) | kompilator wskazuje, co wymaga decyzji człowieka (mocna) |
| spójność projektu | dobra | dobra |
| język do użycia | zero | nadal zero |
| ryzyko | brak implementacji | brak implementacji i proces, który może urosnąć w biurokrację |

### Następny krok

Zamrozić składnię i zbudować `sowa check` tylko dla tego, co wyróżnia Sowę:

1. parser dla `fn`, `type`, `effects`, `desc`, `doc`, `why`, `example`,
2. sprawdzanie efektów z grafu wywołań i z polityką `[effects]` w `sowa.toml`,
3. sprawdzanie odnośników do `.md`, `{Symbol}`, `docs.lock` i `effects.lock`,
4. tłumaczenie `example` i bloków `sowa` do TypeScripta i uruchamianie ich,
5. typy z warunkami na razie tylko jako sprawdzenie `as` w runtime, solver później.

Test na `examples/invoices` pokaże, czy teza działa. Najważniejsze pytanie: czy `sowa review` realnie skraca przegląd kodu z agenta. Jeśli tak, Sowa ma sens niezależnie od tego, jak skończą się typy z warunkami.

## Pierwsza ocena (27.09.2026)

Stan: specyfikacja składni i 6 przykładów, bez parsera i kompilatora.

**W skrócie:** jako pomysł Sowa jest spójna i ma dobrą tezę. Jako język jeszcze nie istnieje, a najtrudniejsza praca jest przed nami.

### Co jest dobre

- **Jasne kryterium, stosowane konsekwentnie.** „Jak najmniej wiedzy do przeczytania” rozstrzygało każdą decyzję: `> 0` zamiast `.positive?`, `&&` zamiast `0 < x < 100`, stałe bez `let`, `or` zamiast wiszącego `else`. Rzadko który projekt ma takie kryterium, a to jest najcenniejsza część Sowy.
- **Sygnatura mówi wszystko.** Z jednej linii widać, co funkcja przyjmuje, jakie są ograniczenia, jakie ma efekty i jakie błędy może zwrócić. To realnie rozwiązuje problem recenzji kodu napisanego przez agenta.
- **Mało pojęć.** Póki co da się wytłumaczyć cały język na jednej stronie.

### Co jest słabe

#### 1. Zajmowaliśmy się głównie składnią, a to najłatwiejsza część

Większość dyskusji dotyczyła tego, czy użyć `α`, `$` czy `x`. Tymczasem to dokładnie ten błąd, przed którym ostrzegał Valim. Nie mamy nic o tym, co jest naprawdę trudne:

- generyki,
- moduły,
- współbieżność,
- model pamięci,
- to, co dokładnie kompilator potrafi udowodnić.

#### 2. Obietnica „kompilator nie przepuści” może się nie utrzymać

Warunki typu `α >= 0 && α <= 100` są łatwe dla solvera. Ale już `total - total * pct / 100` to mnożenie zmiennych (arytmetyka nieliniowa), z którym Z3 często sobie nie radzi. Doświadczenia LiquidHaskella i Fluxa są takie, że programista co chwilę dostaje „nie można udowodnić” i albo się frustruje, albo wszystko obchodzi sprawdzeniem w runtime.

Widać to w naszym własnym przykładzie. W `examples/05_checkout.sowa` jest `apply_discount(...) as Price or return EmptyCart`, czyli sprawdzenie w runtime, bo udowodnienie, że wynik jest większy od zera, wymagałoby kontraktu `ensures`, którego jeszcze nie mamy.

#### 3. Efekty nie skalują się tak prosto, jak wyglądają

Co z `items.map(f)`, gdy `f` łączy się z siecią? Wtedy `map` musi być generyczny po efektach. Koka rozwiązuje to przez wiersze efektów, ale sygnatury robią się wtedy mało czytelne. Nikt jeszcze nie pokazał prostego zapisu. Jeśli go nie wymyślimy, nasze główne kryterium zderzy się z efektami przy pierwszej funkcji wyższego rzędu.

#### 4. Czytelność to słaba przewaga konkurencyjna

Vera, Thermite i Prove mają już kontrakty, efekty i działające kompilatory (zob. [porownanie.md](porownanie.md)). Wyróżnia nas składnia, a składnię najłatwiej skopiować.

#### 5. Brak ekosystemu przesądza sprawę na produkcji

Do SaaS-a potrzeba HTTP, bazy, JSON-a, kolejek i autoryzacji. Nikt nie wybierze języka, w którym trzeba to wszystko napisać od zera.

#### 6. Otwarte dziury w tym, co już jest

- zaokrąglanie przy `Money / 100`,
- `α` w zagnieżdżonych warunkach,
- `or 0`, które może po cichu połykać błędne dane wejściowe.

### Ocena

| | |
|---|---|
| jako ćwiczenie projektowe i specyfikacja pomysłu | bardzo dobrze: spójna, przemyślana, z uzasadnieniami |
| jako język do użycia | na razie zero, bo nie istnieje nic poza papierem |
| szansa na zostanie popularnym językiem | niska, jak dla każdego z 42 projektów w katalogu |

### Proponowany następny krok

Zamiast dalej dopracowywać składnię, sprawdzić tezę w praktyce:

1. **Mały checker** dla podzbioru języka: typy z warunkami (tylko arytmetyka liniowa), efekty, `as ... or` i `match`.
2. **Kompilacja do TypeScripta.** Ekosystem npm dostajemy za darmo, a problem nr 5 znika. Thermite robi to samo z Rustem.
3. **Test na przykładach z `examples/`.** Po nim będzie wiadomo, gdzie checker sobie radzi, a gdzie mówi „nie można udowodnić”.

Taki prototyp pokaże w kilka dni, czy Sowa ma sens, lepiej niż kolejne tygodnie dyskusji o symbolach.
