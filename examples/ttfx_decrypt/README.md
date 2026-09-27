# Przykładowy projekt: ttfx_decrypt

Efekt `decrypt` z [ttfx](https://github.com/omacom/ttfx) przepisany na Sowę. ttfx to przepisany na Rusta [terminaltexteffects](https://github.com/ChrisBuilds/terminaltexteffects). Tekst z wejścia pojawia się w terminalu jako zielony szyfr, który „odszyfrowuje się” znak po znaku.

Przy tym samym ziarnie program wypisuje co do bajta to samo co

```
ttfx --seed N --frame-rate 0 --ignore-terminal-dimensions decrypt
```

Dzięki temu da się porównać szybkość samego języka, bo oba programy robią dokładnie tę samą pracę. Wynik: Sowa → Rust jest ok. 22× wolniejsza od ttfx, czyli od Rusta pisanego ręcznie, a Sowa → Bun ok. 48× wolniejsza. Szczegóły i sposób powtórzenia pomiaru są w [docs/porownanie.md](docs/porownanie.md).

## Uruchomienie

Potrzebny jest Rust (cargo) i [Bun](https://bun.sh). Z katalogu głównego repozytorium:

```sh
cargo build --manifest-path compiler/Cargo.toml
compiler/target/debug/sowa check examples/ttfx_decrypt
compiler/target/debug/sowa test examples/ttfx_decrypt
compiler/target/debug/sowa test --rust examples/ttfx_decrypt
```

Animacja w terminalu:

```sh
printf 'Witaj w Sowie\nttfx decrypt\n' \
  | SEED=1 compiler/target/debug/sowa run --rust examples/ttfx_decrypt \
  | bun examples/ttfx_decrypt/play.js
```

Bez `--rust` program działa w Bunie, a wynik jest ten sam. `SEED` wybiera przebieg: bez tej zmiennej ziarno pochodzi z zegara. Tekst można też podać z pliku, np. `< README.md`.

Program wypisuje wszystkie klatki od razu, bez pauz, tak jak ttfx z `--frame-rate 0`. `play.js` dzieli wyjście na klatki i pokazuje je w 60 klatkach/s. Tempo podaje argument, np. `play.js 30`. Skrypt jest w JS, bo Sowa nie ma jeszcze pauzy (zob. [Propozycje](#propozycje-do-decyzji)).

## Struktura

```
ttfx_decrypt/
  sowa.toml            [resources]: Random z ziarnem z SEED, Terminal; [resources.test]: Random z ziarnem 1
  src/
    types.sowa         Rgb, Frame (tekst komórki i liczba taktów), Glyph (komórka ekranu)
    canvas.sowa        read_canvas: tekst → prostokąt komórek; number_cells, typed_symbols
    scenes.sowa        sceny pisania i szyfrowania, gradient, chain, step
    main.sowa          main(random: Random, terminal: Terminal)
  impl/                ciała funkcji z src/
  docs/
    zalozenia.md       model, wejście, sceny, kolejność losowania, format wyjścia
    porownanie.md      zgodność bajt w bajt i pomiary: ttfx, Sowa → Rust, Sowa → Bun
  play.js              odtwarzacz 60 klatek/s (poza Sową)
```

Człowiek czyta `sowa.toml`, `src/` i `docs/`. Uprawnienia widać w jednym miejscu: tylko `main` dostaje `Terminal`, a funkcje losujące sceny dostają `Random`. Żadna funkcja nie ma dostępu do plików, sieci ani zegara.

Jak ten model odwzorowuje ttfx (sceny złączone z góry zamiast zdarzeń) i w jakiej kolejności program losuje, opisuje [docs/zalozenia.md](docs/zalozenia.md). Od tej kolejności zależy zgodność bajt w bajt.

## Czego nie obsługuje

- Pozostałych efektów ttfx i opcji `decrypt` (kolory, czas trwania scen). Wartości są wpisane na stałe, jak domyślne w ttfx.
- Wejścia z sekwencjami ANSI albo z `\r` poza `\r\n`. Program kończy się wtedy kodem 2, a ttfx je interpretuje.
- Dopasowania do rozmiaru terminala. Program zachowuje się jak ttfx z `--ignore-terminal-dimensions`.

## Propozycje do decyzji

Przykład wymagał rzeczy, których Sowa dotąd nie miała. Kompilator je obsługuje, ale to propozycje: o ich przyjęciu decyduje człowiek. Wpisane są też do [otwartych pytań](../../docs/przemyslenia.md#otwarte-pytania).

| Co | Po co | Gdzie |
|---|---|---|
| `while warunek` z blokiem | pętle, w których liczba kroków nie jest znana z góry: pętla animacji, zakresy znaków szyfru | parser, oba backendy |
| uprawnienie `Random` z `int(min, max)` i `choice(lista)`; w `sowa.toml` `seed` albo `seed_env` | losowanie w tej samej kolejności co ttfx; testy na stałym ziarnie | `[resources]`, runtime |
| uprawnienie `Terminal` z `read()`, `write(text)`, `exit(code)` | całe stdin naraz, stdout z buforem, kod wyjścia | `[resources]`, runtime |
| funkcje `at(lista, i)`, `join(lista, sep)`, `split(text, sep)`, `chars(text)`, `char(kod)` | budowa klatek z tekstu i komórek | wbudowane |
| `sowa build --rust`, `sowa run --rust` | program, nie tylko testy, w backendzie Rust | kompilator |

Otwarte przy tym pytania:

- Czy `Random` bez ziarna ma losować z zegara, czy wymagać `seed` albo `seed_env`?
- Czy `Terminal.exit` ma zostać, czy kod wyjścia ma wynikać z wyniku `main`?
- Pauza (`clock.sleep(ms)`?), żeby tempo animacji dało się ustawić w Sowie bez `play.js`.
