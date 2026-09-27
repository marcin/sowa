# Porównanie z ttfx

Porównanie obejmuje programy, które dostają to samo wejście i to samo ziarno:

- **ttfx**: oryginał z gałęzi `master`, commit `702b630`, zbudowany przez `cargo build --release` z `lto = true`. Na Macu z procesorem ARM działa silnik w Ruście, bo silnik w asemblerze x86-64 buduje się tylko na Linuksie (zob. [porownanie.md z ttfx_decrypt](../../ttfx_decrypt/docs/porownanie.md));
- **ttfx_decrypt**: [pierwsza wersja przykładu](../../ttfx_decrypt/), w której każda klatka przechodzi wszystkie komórki i składa cały ekran;
- **ttfx_decrypt_fast**: ten przykład, w którym klatka przelicza tylko wiersze w trakcie sceny, a tekst składa tylko z wierszy, które się zmieniły.

Obie wersje Sowy zbudowano tym samym kompilatorem przez `sowa build --rust` (rustc `-O -C codegen-units=1`) i `sowa build` (Bun 1.3.4).

## Zgodność bajt w bajt

Wyjście porównywano `cmp` z

```
ttfx --seed N --frame-rate 0 --ignore-terminal-dimensions decrypt
```

na tych samych 6 wejściach co w [ttfx_decrypt](../../ttfx_decrypt/docs/porownanie.md#zgodność-bajt-w-bajt) (od `ab\n c` do 60 wierszy kodu) i ziarnach 1, 7 i 42. W Ruście i w Bunie wyjście i kod wyjścia są identyczne w 18 z 18 przypadków. Wejście z 180 wierszy przy ziarnie 1 też daje identyczne wyjście w Ruście.

## Wyniki pomiarów

Pomiary zrobiono na macOS (Apple Silicon). Wyjście szło do `/dev/null`. Programy uruchamiano na przemian po 15 razy, a tabela podaje najkrótszy czas, bo na maszynie działały też inne procesy.

| Wejście | Wyjście | ttfx | ttfx_decrypt → Rust | ttfx_decrypt_fast → Rust |
|---|---|---|---|---|
| 60 wierszy kodu po 80 kolumn | 44 MB | 55 ms | 50 ms | **41 ms** |
| to samo 3 razy, 180 wierszy | 257 MB | 213 ms | 222 ms | **130 ms** |

Względem ttfx wersja fast jest ok. 1,3 raza szybsza na 60 wierszach i ok. 1,6 raza szybsza na 180 wierszach. Im więcej wierszy, tym większy zysk, bo w danej klatce większość wierszy albo już skończyła scenę, albo jeszcze jej nie zaczęła.

W Bunie wersja fast jest wolniejsza: 3,24 s zamiast 2,40 s na 60 wierszach (najkrótszy z 5 czasów). Każdy nowy `Line` przechodzi przez `$mk`, który sprawdza warunki typu na wszystkich polach, także na liście `scenes` z tysiącami klatek. To znany koszt runtime Buna (zob. [porownanie.md z ttfx_decrypt](../../ttfx_decrypt/docs/porownanie.md#co-dalej), punkty 1 i 3). Z tego samego powodu sceny są polem wiersza, a nie parametrem `advance`: lista w parametrze jest w Bunie sprawdzana i kopiowana przy każdym wywołaniu, więc pierwsza wersja z parametrem `scenes` liczyła duże wejście ponad 5 minut.

## Skąd wynik

Tekst komórki zmienia się tylko w takcie, w którym zaczyna ona nową klatkę sceny (`ticks == 0` przed `step`). W pozostałych taktach rośnie tylko licznik. Stąd trzy oszczędności w `advance`:

1. Wiersz bez komórek w trakcie sceny (`busy` fałszywe) albo taki, którego pierwszy znak nie został jeszcze napisany (`first >= typed`), zostaje bez zmian. Nie ma wtedy ani `map`, ani `join`.
2. Wiersz, w którym żadna komórka nie zaczyna nowej klatki, zachowuje stary tekst. `map` liczy tylko takty.
3. Cała klatka to `join` gotowych tekstów wierszy, bez przechodzenia po komórkach.

Druga część zysku pochodzi z dwóch zmian w generatorze Rusta (`moves.rs`, `codegen_rs.rs`). Obie działają w każdym programie, a nie tylko w tym przykładzie:

- **Ostatni odczyt zmiennej wyjmuje pole.** `advance` czyta `line.cells` jako ostatni odczyt `line`. Wcześniej generator czytał pole zawsze przez `clone()`, więc lista miała dwie referencje i `map` musiał ją skopiować. Teraz lista ma jedną referencję i `map` zmienia ją w miejscu. W pierwszej wersji `advance` ta zmiana skróciła czas z 48 do 41 ms na 60 wierszach i ze 150 do 128 ms na 180 wierszach.
- **Element pętli `for` przez referencję.** Wcześniej `for g in cells` kopiował każdy element. Teraz element, którego ciało pętli nie zmienia ani nie przenosi, jest czytany przez referencję. Bez tego obecne `advance` liczy duże wejście w 50 ms zamiast 41 ms.

Na ttfx_decrypt żadna z tych zmian nie wpływa, bo nie czyta tam pól z listą ani nie ma `for` w gorącej pętli.

ttfx w każdej klatce aktualizuje tylko aktywne znaki, ale cały ekran składa od nowa z komórek. Wersja fast składa od nowa tylko zmienione wiersze. Dlatego jest szybsza, choć przechodzi komórki aktywnych wierszy, także te bez zmian.

## Jak powtórzyć

Z katalogu głównego repozytorium, z ttfx zbudowanym jak w [porownanie.md z ttfx_decrypt](../../ttfx_decrypt/docs/porownanie.md#jak-powtórzyć):

```sh
compiler/target/debug/sowa build --rust examples/ttfx_decrypt
compiler/target/debug/sowa build --rust examples/ttfx_decrypt_fast
head -60 compiler/src/runtime.js | cut -c1-80 > /tmp/large.txt

$TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /tmp/ttfx.out
SEED=1 examples/ttfx_decrypt_fast/.sowa/app_rs < /tmp/large.txt | cmp - /tmp/ttfx.out && echo identyczne

for i in 1 2 3 4 5; do
  /usr/bin/time $TTFX --seed 1 --frame-rate 0 --ignore-terminal-dimensions decrypt < /tmp/large.txt > /dev/null
  SEED=1 /usr/bin/time examples/ttfx_decrypt/.sowa/app_rs < /tmp/large.txt > /dev/null
  SEED=1 /usr/bin/time examples/ttfx_decrypt_fast/.sowa/app_rs < /tmp/large.txt > /dev/null
done
```

`runtime.js` się zmienia, więc duże wejście z czasem będzie inne niż w tabeli.
