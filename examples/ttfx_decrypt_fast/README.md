# Przykładowy projekt: ttfx_decrypt_fast

Efekt `decrypt` z [ttfx](https://github.com/omacom/ttfx) przepisany na Sowę. ttfx to przepisany na Rusta [terminaltexteffects](https://github.com/ChrisBuilds/terminaltexteffects). Tekst z wejścia pojawia się w terminalu jako zielony szyfr, który „odszyfrowuje się” znak po znaku.

Przy tym samym ziarnie program wypisuje co do bajta to samo co

```
ttfx --seed N --frame-rate 0 --ignore-terminal-dimensions decrypt
```

To wariant przykładu [ttfx_decrypt](../ttfx_decrypt/), który zostaje bez zmian. Sceny, losowanie i wyjście są te same. Inaczej działa tylko pętla animacji. Ekran to lista wierszy `Line`, a każdy wiersz pamięta swój gotowy tekst. W każdej klatce `advance` pomija wiersze, które skończyły scenę albo jeszcze się nie zaczęły. Tekst wiersza powstaje od nowa tylko wtedy, gdy któraś komórka zaczyna nową klatkę sceny. W ttfx_decrypt każda klatka przechodzi wszystkie komórki i składa cały ekran od nowa.

Wynik w Ruście na wejściu z 60 wierszy: 41 ms, gdy ttfx liczy 55 ms, a ttfx_decrypt 50 ms. Na wejściu z 180 wierszy: 130 ms, gdy ttfx liczy 213 ms, a ttfx_decrypt 222 ms. W Bunie wersja fast jest wolniejsza od ttfx_decrypt (3,2 s zamiast 2,4 s), bo runtime Buna sprawdza przy każdym nowym wierszu całą listę jego scen. Szczegóły są w [docs/porownanie.md](docs/porownanie.md).

## Uruchomienie

Potrzebny jest Rust (cargo) i [Bun](https://bun.sh). Z katalogu głównego repozytorium:

```sh
cargo build --manifest-path compiler/Cargo.toml
compiler/target/debug/sowa check examples/ttfx_decrypt_fast
compiler/target/debug/sowa test examples/ttfx_decrypt_fast
compiler/target/debug/sowa test --rust examples/ttfx_decrypt_fast
```

Animacja w terminalu:

```sh
printf 'Witaj w Sowie\nttfx decrypt\n' \
  | SEED=1 compiler/target/debug/sowa run --rust examples/ttfx_decrypt_fast \
  | bun examples/ttfx_decrypt_fast/play.js
```

Bez `--rust` program działa w Bunie, a wynik jest ten sam. `SEED` wybiera przebieg: bez tej zmiennej ziarno pochodzi z zegara. Tekst można też podać z pliku, np. `< README.md`.

Program wypisuje wszystkie klatki od razu, bez pauz, tak jak ttfx z `--frame-rate 0`. `play.js` dzieli wyjście na klatki i pokazuje je w 60 klatkach/s. Tempo podaje argument, np. `play.js 30`. Skrypt jest w JS, bo Sowa nie ma jeszcze pauzy (zob. [Propozycje](#propozycje-do-decyzji)).

## Struktura

```
ttfx_decrypt_fast/
  sowa.toml            [resources]: Random z ziarnem z SEED, Terminal; [resources.test]: Random z ziarnem 1
  src/
    types.sowa         Rgb, Frame (tekst komórki i liczba taktów), Glyph (komórka ekranu), Line (wiersz z gotowym tekstem)
    canvas.sowa        read_canvas: tekst → prostokąt komórek; number_cells, make_lines, typed_symbols
    scenes.sowa        sceny pisania i szyfrowania, gradient, chain, step, advance (takt wiersza)
    main.sowa          main(random: Random, terminal: Terminal)
  impl/                ciała funkcji z src/
  docs/
    zalozenia.md       model, wejście, sceny, kolejność losowania, format wyjścia
    porownanie.md      zgodność bajt w bajt i pomiary: ttfx, ttfx_decrypt, ttfx_decrypt_fast
  play.js              odtwarzacz 60 klatek/s (poza Sową)
```

Człowiek czyta `sowa.toml`, `src/` i `docs/`. Uprawnienia widać w jednym miejscu: tylko `main` dostaje `Terminal`, a funkcje losujące sceny dostają `Random`. Żadna funkcja nie ma dostępu do plików, sieci ani zegara.

Jak ten model odwzorowuje ttfx (sceny złączone z góry zamiast zdarzeń) i w jakiej kolejności program losuje, opisuje [docs/zalozenia.md](docs/zalozenia.md). Od tej kolejności zależy zgodność bajt w bajt.

## Czego nie obsługuje

- Pozostałych efektów ttfx i opcji `decrypt` (kolory, czas trwania scen). Wartości są wpisane na stałe, jak domyślne w ttfx.
- Wejścia z sekwencjami ANSI albo z `\r` poza `\r\n`. Program kończy się wtedy kodem 2, a ttfx je interpretuje.
- Dopasowania do rozmiaru terminala. Program zachowuje się jak ttfx z `--ignore-terminal-dimensions`.

## Propozycje do decyzji

Propozycje z [ttfx_decrypt](../ttfx_decrypt/README.md#propozycje-do-decyzji) dotyczą też tego przykładu.

Przy pracy nad przykładem wyszedł błąd w pierwszeństwie `with`, już rozstrzygnięty i poprawiony. Wartość pola w `with` kończyła się na `+` i `-`, więc `l with busy: l.first >= 0` znaczyło `(l with busy: l.first) >= 0`. Checker tego nie zgłaszał, a program kończył się w czasie działania błędem `Line.busy: 0 nie jest Bool`. Teraz wartość pola to całe wyrażenie do przecinka, jak argument w `Line(busy: ...)`. Checker sprawdza też typ wartości pola w `with` i w konstruktorze rekordu, gdy da się go ustalić ze stałej, porównania, parametru z typem, jego pola albo wyniku funkcji.
