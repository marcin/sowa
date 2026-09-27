# Porównanie z ttfx

Porównanie obejmuje trzy programy, które dostają to samo wejście i to samo ziarno:

- **ttfx**: oryginał z gałęzi `master`, commit `702b630` („Bump to 0.4.0”), zbudowany przez `cargo build --release` z `lto = true`. Na Macu z procesorem ARM działa silnik w Ruście. Silnik w asemblerze x86-64 z [PR #35](https://github.com/omacom/ttfx/pull/35) buduje się tylko na Linuksie x86-64, więc tu się nie włączył (`TTFX_ASM_SHOW_TIER=1` wypisuje „this build has no assembly engine”). Według `plans/asm-x86.md` z ttfx asm liczy `decrypt` 19–23× szybciej niż silnik w Ruście. Tych liczb tu nie mierzono;
- **Sowa → Rust**: ten przykład skompilowany przez `sowa build --rust` (rustc `-O`) do `.sowa/app_rs`;
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

Kod wyjścia też jest ten sam we wszystkich przypadkach.

## Wyniki pomiarów

Mierzone przez `/usr/bin/time -l` na macOS (Apple Silicon). Wyjście szło do `/dev/null`, bez odtwarzania w tempie 60 klatek/s. Każdy program uruchamiano 5 razy, a tabela podaje medianę. CPU to suma czasu użytkownika i systemu, a RAM to maksymalna pamięć rezydentna.

| Wejście | Wyjście | Klatki | ttfx | Sowa → Rust | Sowa → Bun |
|---|---|---|---|---|---|
| mały: 2 wiersze, 3 znaki | 33 KB | 445 | 0,00 s · 3 MB | 0,00 s · 2 MB | 0,02 s CPU 0,03 s · 59 MB |
| średni: 26 wierszy, 819 znaków | 26 MB | 1225 | 0,03 s · 22 MB | 0,84 s · 41 MB | 2,36 s CPU 2,96 s · 284 MB |
| duży: 60 wierszy, 1481 znaków | 44 MB | 1664 | 0,05 s · 28 MB | 1,10 s · 68 MB | 2,38 s CPU 2,98 s · 410 MB |

Przy ttfx i Sowa → Rust CPU jest równe czasowi.

Na dużym wejściu, względem ttfx:

| Program | Czas | RAM |
|---|---|---|
| ttfx (Rust pisany ręcznie) | 1× | 1× |
| Sowa → Rust | ok. 22× wolniej | 2,4× więcej |
| Sowa → Bun | ok. 48× wolniej | 15× więcej |

Gdy animacja leci w terminalu w 60 klatkach/s, duże wejście trwa ok. 28 s (1664 klatki). Wszystkie trzy programy liczą ją więc szybciej, niż ją widać. Różnica ma znaczenie przy dłuższych tekstach, w CI i jako miara tego, ile kosztuje sam język.

## Skąd różnica

Oba programy robią to samo i losują w tej samej kolejności, a więc różni je tylko sposób wykonania. ttfx i Sowa → Rust to kod maszynowy z tego samego kompilatora (rustc z LLVM). Różnica nie wynika więc z języka docelowego, tylko z kodu, który generuje Sowa.

**ttfx** trzyma komórki w tablicy i zmienia je w miejscu. Wiersze wypisuje do jednego bufora bajtów.

**Sowa → Rust** (profil z `sample` na macOS). Każda wartość to dynamiczne `V`. Rekord to lista pól wyszukiwanych po nazwie, a `with` buduje nowy rekord. W każdej klatce powstaje więc nowa lista wszystkich komórek. Najwięcej czasu idzie na:

- zwalnianie wartości (`drop` na `V` i `Rc<Vec<V>>`);
- `clone`;
- szukanie pól po nazwie (`field`, `memcmp`);
- `malloc` i `free`.

**Sowa → Bun** (profil z `bun --cpu-prof`). Tu najwięcej czasu idzie na:

- `join` (ok. 25%);
- `$mk`, czyli budowę rekordu przy `with` (ok. 12%). Sprawdza on warunki typu na każdym polu, także na polach, których `with` nie zmienił;
- `async`: runtime czeka na każde wywołanie funkcji, nawet takiej bez uprawnień;
- kopię listy przy każdym przekazaniu jej jako parametru z warunkiem typu.

Te koszty nie zależą od tego przykładu, więc poprawki w runtime przyspieszyłyby każdy program Sowy. Ich listę zawiera sekcja „Co dalej”. Kod przykładu obchodzi część z nich: w gorącej pętli używa lambd zamiast funkcji z parametrem listy, a klatki bierze przez `at()`.

## Co dalej

Pomysły, bez zmian w języku i od najtańszego:

1. `with` sprawdza warunki tylko zmienionych pól. Reszta była już sprawdzona.
2. Bun: funkcje bez uprawnień i bez wywołań z uprawnieniami generowane jako zwykłe (bez `async`). Kompilator wie to z sygnatur.
3. Rust: pola rekordu po indeksie ustalonym w kompilacji zamiast po nazwie. Rekord o znanym typie jako `struct` zamiast listy par.
4. Rust: gdy lista ma jedną referencję (`Rc::strong_count == 1`), `map` może zmieniać ją w miejscu. Tak robią Koka, Lean i Roc. Znika wtedy większość `malloc`/`free` i `drop`.

Punkt 4 zbliżyłby Sowę → Rust do ttfx bez zmiany kodu przykładu. Tego nie sprawdzono.

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

# zgodność
$TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /tmp/ttfx.out
SEED=1 .sowa/app_rs < /tmp/large.txt | cmp - /tmp/ttfx.out && echo Rust: identyczne
SEED=1 bun .sowa/app.js < /tmp/large.txt | cmp - /tmp/ttfx.out && echo Bun: identyczne

# czas i pamięć
/usr/bin/time -l $TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /dev/null
SEED=1 /usr/bin/time -l .sowa/app_rs < /tmp/large.txt > /dev/null
SEED=1 /usr/bin/time -l bun .sowa/app.js < /tmp/large.txt > /dev/null
```

`runtime.js` się zmienia, więc duże wejście z czasem będzie inne niż w tabeli. Zgodność bajt w bajt powinna się utrzymać dla każdego tekstu bez `\r` poza `\r\n` i bez sekwencji ANSI (zob. [zalozenia.md](zalozenia.md#wejście)).
