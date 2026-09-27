# Porównanie z ttfx

Porównanie obejmuje programy, które dostają to samo wejście i to samo ziarno:

- **ttfx**: oryginał z gałęzi `master`, commit `702b630` („Bump to 0.4.0”), zbudowany przez `cargo build --release` z `lto = true`. Na Macu z procesorem ARM działa silnik w Ruście. Silnik w asemblerze x86-64 z [PR #35](https://github.com/omacom/ttfx/pull/35) buduje się tylko na Linuksie x86-64, więc tu się nie włączył (`TTFX_ASM_SHOW_TIER=1` wypisuje „this build has no assembly engine”). Według `plans/asm-x86.md` z ttfx asm liczy `decrypt` 19–23× szybciej niż silnik w Ruście. Tych liczb tu nie mierzono;
- **Sowa → Rust**: ten przykład skompilowany przez `sowa build --rust` (rustc `-O`) do `.sowa/app_rs`;
- **Sowa → Rust, c00432d**: to samo, ale kompilatorem z commitu `c00432d`, w którym każda wartość była dynamicznym `V`. Podany jako punkt odniesienia;
- **Sowa → Bun**: ten przykład skompilowany przez `sowa build` do `.sowa/app.js` i uruchomiony w Bunie 1.3.4.

## Zgodność bajt w bajt

Na każdym wejściu i każdym ziarnie wyjście obu wersji Sowy porównywano `cmp` z

```
ttfx --seed N --frame-rate 0 --ignore-terminal-dimensions decrypt
```

| Wejście | Co sprawdza | Ziarna | Rust | Bun |
|---|---|---|---|---|
| `ab\n c` | najprostszy przypadek | 1, 7, 42 | identyczne | identyczne |
| taby, polskie znaki, puste wiersze w środku i na końcu, spacje na końcu | `read_canvas` | 1, 7, 42 | identyczne | identyczne |
| to samo z `\r\n` | końce wierszy z Windows | 1, 7, 42 | identyczne | identyczne |
| same spacje i `\n` | `NO INPUT.` i kod 1 | 1, 7, 42 | identyczne | identyczne |
| 1000 bajtów tekstu, 26 wierszy | 26 MB wyjścia | 1, 7, 42 | identyczne | identyczne |
| 60 wierszy kodu po 80 kolumn | 44 MB wyjścia | 1, 7, 42 | identyczne | identyczne |
| to samo 4 razy, 240 wierszy | 420 MB wyjścia | 1 | identyczne | nie sprawdzano |

Kod wyjścia też jest ten sam we wszystkich przypadkach.

## Wyniki pomiarów

Pomiary zrobiono na macOS (Apple Silicon). Wyjście szło do `/dev/null`, bez odtwarzania w tempie 60 klatek/s. Każdy program uruchamiano 5 razy, a tabela podaje medianę. Czas to czas od startu do końca procesu, mierzony w milisekundach. CPU to suma czasu użytkownika i systemu, a RAM to maksymalna pamięć rezydentna, obie z `wait4`.

| Wejście | Wyjście | Klatki | ttfx | Sowa → Rust | Sowa → Rust, c00432d | Sowa → Bun |
|---|---|---|---|---|---|---|
| mały: 2 wiersze, 3 znaki | 33 KB | 445 | 2,7 ms · 3 MB | 2,0 ms · 2 MB | 4,5 ms · 2 MB | 25 ms, CPU 46 ms · 59 MB |
| średni: 26 wierszy, 819 znaków | 26 MB | 1225 | 38 ms · 22 MB | 38 ms · 9 MB | 0,88 s · 48 MB | 2,45 s, CPU 3,06 s · 282 MB |
| duży: 60 wierszy, 1481 znaków | 44 MB | 1664 | 59 ms · 28 MB | 53 ms · 14 MB | 1,16 s · 70 MB | 2,48 s, CPU 3,03 s · 406 MB |
| bardzo duży: duży 4 razy, 5924 znaki | 420 MB | 4702 | 327 ms · 99 MB | 367 ms · 52 MB | nie mierzono | nie mierzono |

Przy ttfx i Sowa → Rust CPU jest równe czasowi z dokładnością do 2 ms.

Obecnie `sowa build --rust` dodaje do `-O` flagi `-C codegen-units=1 -C panic=abort`. Z nimi duże wejście trwa ok. 50 ms zamiast 53 ms, a wejście z 180 wierszy ok. 3% krócej. Tabela pochodzi z pomiarów z samym `-O`.

Względem ttfx:

| Program | Czas, duży | RAM, duży | Czas, bardzo duży | RAM, bardzo duży |
|---|---|---|---|---|
| ttfx (Rust pisany ręcznie) | 1× | 1× | 1× | 1× |
| Sowa → Rust | 0,9× (ok. 10% szybciej) | 0,5× | 1,12× (ok. 12% wolniej) | 0,5× |
| Sowa → Rust, c00432d | ok. 20× wolniej | 2,5× więcej | – | – |
| Sowa → Bun | ok. 42× wolniej | 14× więcej | – | – |

Gdy animacja leci w terminalu w 60 klatkach/s, duże wejście trwa ok. 28 s (1664 klatki). Wszystkie programy liczą ją więc szybciej, niż ją widać. Różnica ma znaczenie przy dłuższych tekstach, w CI i jako miara tego, ile kosztuje sam język.

## Skąd wynik

Oba programy robią to samo i losują w tej samej kolejności, a więc różni je tylko sposób wykonania. ttfx i Sowa → Rust to kod maszynowy z tego samego kompilatora (rustc z LLVM). O wyniku decyduje więc kod, który generuje Sowa.

**ttfx** trzyma znaki w tablicy i zmienia je w miejscu. W każdej klatce aktualizuje tylko aktywne znaki, a wiersze wypisuje do jednego bufora bajtów.

**Sowa → Rust, c00432d.** Każda wartość była dynamicznym `V`. Rekord był listą pól wyszukiwanych po nazwie, a `with` budował nowy rekord. W każdej klatce powstawała więc nowa lista wszystkich komórek. Czas szedł na `drop`, `clone`, szukanie pól po nazwie oraz `malloc` i `free`.

**Sowa → Rust** (`compiler/src/codegen_rs.rs`, `moves.rs`, `runtime.rs`). Program w Sowie się nie zmienił. Zmienił się generator:

- typy znane w kompilacji mają typy Rusta: `Int` to `i64`, `String` to `Rc<str>`, `List<T>` to `Rc<Vec<T>>`, a rekord to `struct`. Dynamiczne `V` zostaje tylko dla typów bez odpowiednika (`Money`, daty, warianty, unie);
- ostatni odczyt zmiennej przenosi wartość zamiast ją klonować (`moves.rs`). Lista i rekord mają wtedy jedną referencję i można je zmienić w miejscu. Tak robią Koka, Lean i Roc;
- `map` z wynikiem tego samego typu zmienia tablicę w miejscu. Gdy lambda zwraca swój parametr bez zmian (`return g`), element zostaje w tablicy bez kopiowania. W tym przykładzie większość komórek w klatce się nie zmienia, więc to daje najwięcej;
- `with` na rekordzie z jedną referencją zmienia pola w miejscu. Warunek typu jest sprawdzany tylko na zmienionych polach, a pole tekstowe jest przypisywane tylko wtedy, gdy wskazuje inny tekst;
- parametr, który funkcja tylko czyta, przychodzi przez referencję, a `frames = at(lista, i)` nie kopiuje elementu;
- `terminal.write(a + b)`, `join` i `to_string` piszą od razu do bufora wyjścia, bez tekstów pośrednich. Krótkie teksty są kopiowane bez wywołania `memcpy`, a liczby bez `format!`;
- lambdy są wklejane w pętlę (`#[inline(always)]`), a obsługa błędów (`at` poza listą) jest poza gorącą ścieżką.

Na bardzo dużym wejściu Sowa → Rust jest ok. 12% wolniejsza. Według profilu (`sample`) ok. 40% czasu idzie na rysowanie klatki, a reszta na `map` po komórkach. ttfx odwiedza w `update` tylko aktywne znaki, a program Sowy w każdej klatce przechodzi wszystkie komórki. Czy to jest przyczyna różnicy, nie sprawdzono.

**Sowa → Bun** (profil z `bun --cpu-prof`). Tu najwięcej czasu idzie na:

- `join` (ok. 25%);
- `$mk`, czyli budowę rekordu przy `with` (ok. 12%). Sprawdza on warunki typu na każdym polu, także na polach, których `with` nie zmienił;
- `async`: runtime czeka na każde wywołanie funkcji, nawet takiej bez uprawnień;
- kopię listy przy każdym przekazaniu jej jako parametru z warunkiem typu.

Te koszty nie zależą od tego przykładu, więc poprawki w runtime przyspieszyłyby każdy program Sowy. Ich listę zawiera sekcja „Co dalej”. Kod przykładu obchodzi część z nich: w gorącej pętli używa lambd zamiast funkcji z parametrem listy, a klatki bierze przez `at()`.

## Co dalej

Pomysły bez zmian w języku:

1. Bun: `with` sprawdza warunki tylko zmienionych pól, tak jak w Ruście.
2. Bun: funkcje bez uprawnień i bez wywołań z uprawnieniami generowane jako zwykłe (bez `async`). Kompilator wie to z sygnatur.
3. Bun: przenoszenie ostatniego odczytu i `map` w miejscu, jak w Ruście.
4. Rust: sprawdzić, skąd różnica na bardzo dużym wejściu.

## Jak powtórzyć

Z katalogu głównego repozytorium:

```sh
cargo build --manifest-path compiler/Cargo.toml
compiler/target/debug/sowa build examples/ttfx_decrypt          # .sowa/app.js
compiler/target/debug/sowa build --rust examples/ttfx_decrypt   # .sowa/app_rs

git clone https://github.com/omacom/ttfx /tmp/ttfx
cargo build --release --manifest-path /tmp/ttfx/Cargo.toml
TTFX=/tmp/ttfx/target/release/ttfx

cd examples/ttfx_decrypt
head -60 ../../compiler/src/runtime.js | cut -c1-80 > /tmp/large.txt
cat /tmp/large.txt /tmp/large.txt /tmp/large.txt /tmp/large.txt > /tmp/big.txt

# zgodność
$TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /tmp/ttfx.out
SEED=1 .sowa/app_rs < /tmp/large.txt | cmp - /tmp/ttfx.out && echo Rust: identyczne
SEED=1 bun .sowa/app.js < /tmp/large.txt | cmp - /tmp/ttfx.out && echo Bun: identyczne

# czas i pamięć
/usr/bin/time -l $TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /dev/null
SEED=1 /usr/bin/time -l .sowa/app_rs < /tmp/large.txt > /dev/null
SEED=1 /usr/bin/time -l bun .sowa/app.js < /tmp/large.txt > /dev/null
```

`/usr/bin/time` podaje czas z dokładnością do 0,01 s, więc do tabeli czas mierzono w milisekundach i brano medianę z 5 uruchomień. Wersję z `c00432d` buduje się tak samo w osobnym katalogu (`git worktree add /tmp/sowa-c00432d c00432d`).

`runtime.js` się zmienia, więc duże wejście z czasem będzie inne niż w tabeli. Zgodność bajt w bajt powinna się utrzymać dla każdego tekstu bez `\r` poza `\r\n` i bez sekwencji ANSI (zob. [zalozenia.md](zalozenia.md#wejście)).
