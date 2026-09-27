# Gramatyka

Składnia Sowy w jednym miejscu: znaki i wcięcia, deklaracje, instrukcje, wyrażenia i ich pierwszeństwo. Opis odpowiada temu, co dziś przyjmuje kompilator (`compiler/src/lexer.rs` i `parser.rs`). Znaczenie konstrukcji, czyli uprawnienia, warunki typów i dokumentacja, opisuje [zalozenia.md](zalozenia.md).

Zapis:

- `"fn"` to znak albo słowo wpisane dosłownie;
- `[ x ]` oznacza, że `x` jest opcjonalne;
- `{ x }` oznacza, że `x` powtarza się zero lub więcej razy;
- `a | b` oznacza jedną z możliwości;
- `NL`, `INDENT` i `DEDENT` to koniec linii, wejście w blok z większym wcięciem i wyjście z niego.

## Znaki

- **Wcięcia** robi się spacjami. Tabulator to błąd. Blok to linie z większym wcięciem niż linia nad nim. Linia z mniejszym wcięciem musi wrócić dokładnie do wcięcia któregoś z otwartych bloków.
- **Komentarz** to `//` do końca linii. Pusta linia i linia z samym komentarzem nic nie znaczą.
- **Nazwa** zaczyna się literą albo `_`, a dalej ma litery, cyfry i `_`. Litery mogą być spoza ASCII, np. `α`.
- **Liczby:** `42` to `Int`, a `12.50` to liczba dziesiętna (np. `Money`). Nie ma zapisu `1e3`, `0x10` ani `1_000`.
- **Tekst** to `"..."` w jednej linii, ze znakami specjalnymi `\n`, `\t`, `\r`, `\"` i `\\`.
- **Html:** `html"<b>{name}</b>"` w jednej linii albo `html"` na końcu linii, a pod nim linie aż do linii, która po wcięciu zaczyna się od `"`. W `{...}` stoi wyrażenie. Wspólne wcięcie linii jest usuwane.
- **Słowa kluczowe:** `fn`, `type`, `return`, `match`, `if`, `else`, `for`, `while`, `in`, `var`, `try`, `as`, `or`, `is`, `not`, `with`, `true`, `false`. Nie mogą być nazwami.
- **Słowa na początku linii:** `desc`, `doc` i `why` zamieniają resztę linii w tekst dosłowny. Samo `desc` bierze linie z większym wcięciem pod spodem. `example` i `property` mają znaczenie tylko w ciele funkcji.

### Kiedy nowa linia nic nie znaczy

- **W nawiasach** `( )`, `[ ]` i `{ }` nowa linia i wcięcie nic nie znaczą. Wyjątek to linia zakończona `=>`: pod nią stoi blok lambdy, który kończy się linią z mniejszym wcięciem, np. `)`.
- **Linia zaczynająca się od `|`, `||`, `->`, `==`, `!=` albo `&&`** jest ciągiem poprzedniej. Tak łamie się długą unię, sygnaturę albo warunek.
- **Inne operatory nie przedłużają linii.** Linia zaczynająca się od `+`, `or` albo `with` to błąd. Wyrażenie, które się nie mieści, bierze się w nawiasy.

## Plik

```
plik        = { dyrektywa NL } { typ | funkcja }
dyrektywa   = "desc" tekst | "desc" NL INDENT linie DEDENT | "doc" tekst | "why" tekst
```

Dyrektywy przed pierwszą deklaracją to nagłówek pliku.

## Typy

```
typ         = "type" nazwa "=" typ_wyr NL [ INDENT { dyrektywa NL } DEDENT ]
            | "type" nazwa NL [ INDENT { dyrektywa NL } { nazwa ":" typ_wyr NL } DEDENT ]

typ_wyr     = typ_człon { "|" typ_człon }
typ_człon   = nazwa [ "<" typ_wyr { "," typ_wyr } ">" ] [ "(" reszta ")" ]
reszta      = nazwa ":" typ_wyr { "," nazwa ":" typ_wyr }     // pola wariantu
            | nazwa "=>" wyr                                   // warunek z własną nazwą
            | wyr                                              // warunek z α
```

Pierwsza postać to alias, zawężenie albo unia (`type Percent = Int(α >= 0 && α <= 100)`, `type Route = Home | Show(number: String)`). Druga to rekord z polami w bloku. Po `=` może stać tylko typ w jednej linii, łamany przed `|`.

## Funkcje

```
funkcja     = "fn" nazwa "(" [ param { "," param } [ "," ] ] ")" [ "->" typ_wyr ] NL
              [ INDENT { dyrektywa NL | przykład | własność | instr } DEDENT ]
param       = nazwa ":" typ_wyr
przykład    = "example" wyr NL | "example" blok
własność    = "property" wyr NL
```

`desc`, `doc` i `why` muszą stać przed pierwszą instrukcją, a `example` i `property` mogą stać w dowolnym miejscu. Które elementy wolno pisać w `src/`, a które w `impl/`, sprawdza kompilator (zob. [zalozenia.md](zalozenia.md#specyfikacja-i-kod)).

## Instrukcje

```
blok        = NL INDENT { instr } DEDENT
instr       = "return" [ wyr ] NL
            | "var" nazwa "=" wyr NL
            | nazwa "=" wyr NL
            | "if" wyr blok [ "else" ( blok | instr_if ) ]
            | "for" nazwa "in" wyr blok
            | "while" wyr blok
            | "match" wyr { "," wyr } NL INDENT { gałąź } DEDENT
            | wyr NL
gałąź       = wzorzec { "," wzorzec } "=>" ( instr | blok )
wzorzec     = "_" | tekst | nazwa | Nazwa [ nazwa ] | "[" [ wzorzec { "," wzorzec } ] "]"
```

- `nazwa = wyr` tworzy nową nazwę albo, gdy nazwę utworzono przez `var`, nadpisuje ją.
- W `else if` drugie `if` jest zwykłą instrukcją, więc łańcuch może mieć dowolną długość.
- Wzorzec zaczynający się wielką literą to typ albo wariant, z opcjonalną nazwą całej wartości (`ShowInvoice r`). Mała litera wiąże dowolną wartość, a `_` pasuje do wszystkiego bez nazwy. Nie ma wzorców liczbowych ani rozkładania na pola.
- Instrukcja, której wyrażenie kończy się blokiem (lambda z blokiem, `or` z blokiem), nie ma już osobnego końca linii.

## Wyrażenia

Od najniższego pierwszeństwa do najwyższego:

| Poziom | Zapis | Łączność |
|---|---|---|
| lambda | `x => wyr`, `x => blok` | – |
| lub | `a \|\| b` | lewostronna |
| i | `a && b` | lewostronna |
| zaprzeczenie | `not a` | prawostronna |
| porównanie | `a == b`, `!=`, `<`, `<=`, `>`, `>=`, `a is T`, `a is not T` | brak |
| zamiana i kopia | `a as T`, `a as T or …`, `a with pole: wyr, …` | lewostronna |
| dodawanie | `a + b`, `a - b` | lewostronna |
| mnożenie | `a * b`, `a / b`, `a % b` | lewostronna |
| jednoargumentowe | `-a`, `try a` | prawostronna |
| przyrostkowe | `f(…)`, `a.pole`, `a.metoda(…)`, `a.metoda<T>(…)` | lewostronna |
| proste | liczba, tekst, html, `true`, `false`, nazwa, `[a, b]`, `(wyr)` | – |

```
wyr         = nazwa "=>" ( wyr | blok ) | lub
lub         = i { "||" i }
i           = nie { "&&" nie }
nie         = "not" nie | porówn
porówn      = zamiana [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) zamiana | "is" [ "not" ] typ_wyr ]
zamiana     = suma { "as" typ_wyr [ "or" alt ] | "with" pole { "," pole } }
alt         = "return" [ wyr ] | blok | suma
pole        = nazwa ":" wyr
suma        = iloczyn { ( "+" | "-" ) iloczyn }
iloczyn     = jedno { ( "*" | "/" | "%" ) jedno }
jedno       = "-" jedno | "try" jedno | przyr
przyr       = proste { "(" argumenty ")" | "." nazwa [ "<" typ_wyr { "," typ_wyr } ">" ] [ "(" argumenty ")" ] }
argumenty   = [ arg { "," arg } [ "," ] ]
arg         = nazwa ":" wyr | wyr
proste      = liczba | tekst | html | "true" | "false" | nazwa | "[" [ wyr { "," wyr } [ "," ] ] "]" | "(" wyr ")"
```

Reguły, które łatwo przeoczyć:

- **Porównania się nie łączą.** `a < b < c` to błąd składni. Pisze się `a < b && b < c` (zob. [zalozenia.md](zalozenia.md#typy-z-warunkami)).
- **`not` obejmuje całe porównanie:** `not a == b` znaczy `not (a == b)`.
- **Wartość pola w `with` sięga do przecinka.** `p with ok: p.n >= 0` znaczy `p with ok: (p.n >= 0)`. Przecinek kończy wartość tylko wtedy, gdy po nim stoi `nazwa:`, a w przeciwnym razie należy do otaczającego nawiasu. `with` z polami w kolejnych liniach działa tylko w nawiasach, bo poza nimi nowa linia kończy instrukcję.
- **Wartość po `or` kończy się na dodawaniu.** `x as T or a + 1` działa, ale `x as Flag or b > 0` znaczy `(x as Flag or b) > 0` (zob. propozycję niżej).
- **`try` wiąże mocniej niż działania:** `try f(x) + 1` znaczy `(try f(x)) + 1`.
- **Wywołać można tylko funkcję po nazwie albo metodę.** Lambdy w zmiennej nie wywołuje się przez `f(x)`. Lambda ma dokładnie jeden parametr.
- **Argumenty mogą mieć nazwy:** `Line(name: "A", quantity: 1)`. Rekord i wariant z polami tworzy się tylko z nazwami pól.

## Propozycje do decyzji

- **PROPOZYCJA: wartość po `or` do końca wyrażenia.** Dziś wartość domyślna po `or` kończy się na `+` i `-`, tak jak do niedawna wartość pola w `with`. `y = x as Flag or b > 0` przechodzi `sowa check`, a w czasie działania kończy się błędem „nie da się porównać true i 0”. Propozycja: po `or` stoi całe wyrażenie, tak jak po `return`. Wtedy `or` zamiany wiązałoby słabiej niż `||` i `&&`, co da się przeczytać tylko w jeden sposób, bo `or` nie jest operatorem logicznym (zob. [zalozenia.md](zalozenia.md#zamiana-wartości-as--or)).
- **PROPOZYCJA: ciąg linii także po `+`, `-` i `or`.** Dziś długie wyrażenie arytmetyczne łamie się tylko w nawiasach. Lista operatorów, od których może zaczynać się ciąg linii, mogłaby objąć też `+`, `-`, `*`, `/`, `or` i `with`.
