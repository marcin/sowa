# Założenia

## Zasada nadrzędna

Agent pisze, człowiek czyta i zatwierdza. Z tego wynika reszta:

1. **Jak najmniej wiedzy do przeczytania.** Kod ma być zrozumiały dla kogoś, kto zna tylko podstawy: operatory, `if`, `return`, wywołania funkcji. Każda rzecz, którą trzeba sprawdzić w dokumentacji, to koszt.
2. **Dłuższe i oczywiste wygrywa z krótkim i sprytnym.** Agentowi nie przeszkadza pisanie więcej, a recenzentowi przeszkadza zgadywanie.
3. **Rygorystyczne granice, swobodne wnętrze.** Sygnatury, warunki na typach, efekty i błędy są jawne i sprawdzane. To one są recenzowane. Ciało funkcji może być żmudne, bo kompilator pilnuje zgodności z sygnaturą.
4. **Jeden sposób na jedną rzecz.** Mała, stabilna semantyka, bez makr i bez „magii” (monkey-patching, `method_missing`, otwarte klasy, ukryte hooki, globalny stan).
5. **Znajoma składnia, nowa semantyka.** Składnia ma przypominać to, co ludzie i modele już znają (Rust, TypeScript, Ruby). Nowość jest w tym, co kompilator sprawdza.
6. **Sprawdzanie w kompilacji.** Tam, gdzie się da, błąd ma wyjść przed uruchomieniem, a nie na produkcji.

## Składnia (ustalona)

### Funkcje

Słowo kluczowe to `fn`. Zawsze jawne typy parametrów i wyniku. Zawsze jawne `return`, bez niejawnego zwracania ostatniego wyrażenia.

```
fn apply_discount(total: Money, pct: Percent) -> Money
  return total - total * pct / 100
```

### Typy z warunkami

W nawiasie za typem piszesz zwykły warunek. Sprawdzaną wartość oznaczasz **`α`** albo **własną nazwą** (zapis lambdy).

```
type Price    = Money(α > 0)
type Percent  = Int(α >= 0 && α <= 100)
type NonEmpty = List(len(α) > 0)

type Even  = Int(n => n % 2 == 0)
type Valid = Order(o => o.total > 0 && len(o.items) > 0)
```

Reguły:

- `α` zawsze oznacza sprawdzaną wartość i nigdy nie jest zmienną. Kompilator widzi go jak zwykły identyfikator, ale jego znaczenie wynika z miejsca użycia.
- Własną nazwę wybierasz, gdy warunek jest dłuższy, gdy nazwa coś wyjaśnia albo gdy wolisz pisać z klawiatury. Jeśli ta nazwa koliduje ze zmienną w zasięgu, kompilator zgłasza błąd. Nie zgadujemy.
- W warunkach używamy zwykłych operatorów i `&&`, w kolejności jak na osi liczbowej: `α >= 0 && α <= 100`.
- Nie ma zapisu łańcuchowego `0 <= α <= 100`, bo w C, Javie i JS znaczy on co innego.
- Nie ma zakresów `0..100`, bo w różnych językach inaczej traktują górną granicę.
- Nie ma predykatów typu `.positive?` czy `.between?`. Piszemy `α > 0`.
- Formatter może ujednolicić projekt do jednej formy (tylko `α`, tylko nazwy albo obie).
- W edytorze `α` można wpisać skrótem `\a`, tak jak robi to Lean.

Warunek można napisać bezpośrednio przy parametrze i może on odwoływać się do wcześniejszych parametrów:

```
fn slice(list: List, start: Int(α >= 0), end: Int(α > start)) -> List
```

### Zmienne: bez `let`, z `var`

Domyślnie każda nazwa jest niezmienna i nie ma słowa kluczowego. Tylko zmienne, które się zmieniają, oznacza się `var`.

```
pct = input as Percent or return InvalidDiscount
amount = apply_discount(total, pct)

var total = 0
for item in items
  total = total + item.price
```

Reguły:

- `nazwa = wartość` tworzy nową, niezmienną nazwę.
- `var nazwa = wartość` tworzy zmienną, którą później można nadpisać przez `nazwa = ...`.
- Nadpisanie nazwy bez `var` to błąd: „total jest niezmienne, użyj var”.
- Nie ma przesłaniania nazw. Nie można ponownie użyć nazwy, która jest już w zasięgu, np. parametru. Trzeba wybrać nową, opisową nazwę (`clean = input.trim()`).
- Nieużywana nazwa to błąd, więc literówki (`totl = total + 1`) wychodzą przy kompilacji.

Najczęstszy przypadek, czyli stała, nie ma żadnego słowa kluczowego. `var` pojawia się rzadko i od razu mówi recenzentowi, że ta wartość się zmienia.

### Zamiana wartości: `as ... or`

Zwykła wartość staje się typem z warunkiem przez `as`. Jeśli warunek nie jest spełniony, wykonuje się to, co stoi po `or`. Czyta się jak zdanie: „zamień na Percent albo zwróć InvalidDiscount”.

```
pct = input as Percent or return InvalidDiscount
```

Po `or` może stać:

- `return` z błędem, jak wyżej,
- wartość domyślna tego samego typu: `pct = input as Percent or 0`,
- blok z wcięciem, gdy trzeba zrobić coś więcej. Blok musi kończyć się `return`:

```
pct = input as Percent or
  log("zły rabat: " + input)
  return InvalidDiscount
```

`or` jest wolne, bo w warunkach logicznych używamy `&&` i `||`.

Zamiast `as ... or` można też użyć zwykłego `if` z `is`:

```
if input is not Percent
  return InvalidDiscount
pct = input as Percent
```

Kompilator nie przepuści wartości, dla której nie da się udowodnić warunku. Samo `Percent(input)` bez sprawdzenia się nie skompiluje („nie można udowodnić, że input >= 0 && input <= 100”).

### Efekty

Linia `effects` pod sygnaturą mówi, co funkcja robi poza liczeniem. Funkcja bez `effects` jest czysta.

```
fn get_user(id: UserId) -> User
  effects Db.read

fn send_receipt(email: Email, receipt: Receipt)
  effects Net
```

Reguła jest jedna: jeśli wywołujesz funkcję z efektem, musisz ten efekt zadeklarować u siebie. Agent nie przemyci wywołania sieci w funkcji, która miała tylko czytać z bazy.

### Błędy

Możliwe błędy są częścią typu wyniku i zapisuje się je przez `|`.

```
type PaymentError = CardDeclined | NoFunds | Timeout

fn charge(card: Card, amount: Price) -> Receipt | PaymentError
  effects Net
```

- `try` przekazuje błąd wyżej: jeśli wynik jest błędem, funkcja kończy się i go zwraca.
- `match` obsługuje błąd na miejscu. Kompilator pilnuje, żeby żaden przypadek nie został pominięty. Po dodaniu nowego wariantu każdy niepełny `match` przestaje się kompilować.

```
match charge(user.card, amount)
  Receipt r    => return r
  CardDeclined => return Declined
  NoFunds      => return Declined
  Timeout      => return retry_later()
```

## Poza zakresem (na razie)

Te pomysły padły, ale nie są jeszcze rozpisane. Szczegóły w [przemyslenia.md](przemyslenia.md).

- `spec` z `requires` / `ensures` i generowaniem testów property-based
- zapytania o program (`query callers(charge) where effects contains Net`)
- pochodzenie kodu (`@origin(agent: ..., reviewed: false)`) i polityki wdrożeń
- procesy z supervisorem i obserwowalnym stanem, w stylu Erlanga
- model pamięci i kompilacja (LLVM / WASM, własność jak w Ruście)
