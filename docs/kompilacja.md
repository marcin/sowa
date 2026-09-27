# Kompilacja

Jak dziś Sowa zamienia się w działający program, ile to trwa i jakie są inne drogi. Wszystkie pomiary zrobiono na macOS (Apple Silicon) na przykładach [fakturownia_web](../examples/fakturownia_web/) i [ttfx_decrypt](../examples/ttfx_decrypt/).

## Dwie drogi dziś

```
.sowa ─ lexer → parser → check ─┬─ codegen_rs.rs → .sowa/app_rs.rs → rustc (LLVM) → .sowa/app_rs
                                └─ codegen.rs    → .sowa/app.js   → Bun (--bun)
```

| | Rust (domyślnie) | JS dla Buna (testowo) |
|---|---|---|
| polecenie | `sowa build`, `run`, `test` | to samo z `--bun` |
| kompilacja | generowanie Rusta i rustc: 3–6 s | tylko generowanie JS, ok. 10 ms |
| program | szybki: 50 ms, 14 MB (ręcznie pisany ttfx: 56 ms, 28 MB) | wolny: ttfx_decrypt 2,48 s, 406 MB |
| do czego | praca, wydanie, pomiary | szybka pętla testów i porównanie; na razie bez nowych funkcji |

Obie drogi dają to samo: te same wyniki testów, te same komunikaty i te same wylosowane przypadki property. W ttfx_decrypt wyjście z obu dróg jest identyczne z ttfx bajt w bajt.

Rust jest kompilowany z `rustc -O -C codegen-units=1`, a z opcją `--panic-abort` dochodzi `-C panic=abort`. Szczegóły są w [compiler/README.md](../compiler/README.md). Szybkość programu zależy od kodu, który generuje Sowa: typy Rusta, przenoszenie ostatniego odczytu i zmiana w miejscu. Opis jest w [porownanie.md z ttfx_decrypt](../examples/ttfx_decrypt/docs/porownanie.md#skąd-wynik).

## Pętla „zmiana → test”

Tabela podaje czas od wpisania polecenia do wyniku, po zmianie w kodzie. Pomiar objął całe polecenie `sowa` (kompilator zbudowany z `--release`), z medianą z 3 uruchomień.

| Polecenie | fakturownia_web | ttfx_decrypt |
|---|---|---|
| `sowa check` | 0,01 s | 0,00 s |
| `sowa test --bun` | 0,08 s | 0,02 s |
| `sowa test`, rustc 1.98 (obecnie) | 6,4 s | 3,3 s |
| `sowa test`, nocny rustc, LLVM z `codegen-units=16` | 4,8 s | 2,4 s |
| `sowa test`, nocny rustc, Cranelift | 1,35 s | 0,88 s |
| `sowa test`, nocny rustc, Cranelift z `codegen-units=16` | 1,25 s | 0,80 s |
| `sowa test` bez zmian w kodzie (kompilacja pominięta) | 0,04 s | 0,01 s |

Domyślny jest Rust, bo tylko on daje szybki program i to on dostaje nowe funkcje. Pętla „zmiana → test” trwa w nim jednak 3–6 s zamiast 0,08 s w Bunie. Skracają ją Cranelift i `codegen-units=16` (niżej), a gdy liczy się każda sekunda, `--bun` nadal działa. Przy długich testach Rust wygrywa także w pętli. Przy 10 000 przypadków na property w fakturownia_web Bun potrzebował 3,48 s, a Rust 1,97 s bez kompilacji. Ten pomiar zrobiono jeszcze przed typami w kompilacji, gdy każda wartość w Ruście była dynamicznym `V`, a nowego nie było.

## Ustawienia rustc i Cranelift

Ten sam wygenerowany kod skompilowano sam rustc (nocny 1.101, 2026-09-26) z różnymi ustawieniami. Czas kompilacji to mediana z 3 uruchomień, a czas programu to mediana z 11 uruchomień. Wyjście ttfx_decrypt było identyczne z ttfx we wszystkich 18 przypadkach i przy każdym ustawieniu. Testy fakturownia_web przechodziły 33 z 33.

| Ustawienie | Kompilacja ttfx_decrypt | Kompilacja fakturownia_web (testy) | ttfx_decrypt, 60 wierszy | ttfx_decrypt, 180 wierszy | Testy fakturownia_web | Plik ttfx_decrypt |
|---|---|---|---|---|---|---|
| LLVM `-O`, `codegen-units=1` (obecnie) | 2,28 s | 5,63 s | 51 ms | 223 ms | 25 ms | 740 KB |
| LLVM `-O`, `codegen-units=16` | 1,97 s | 4,64 s | 52 ms | 228 ms | – | 800 KB |
| LLVM `-C opt-level=1` | 1,69 s | 3,81 s | 60 ms | 271 ms | – | 842 KB |
| LLVM `-C opt-level=0` | 0,51 s | 0,84 s | 455 ms | 2,45 s | 43 ms | 1664 KB |
| Cranelift `-C opt-level=0` | 0,46 s | 0,74 s | 511 ms | 2,70 s | 45 ms | 1883 KB |
| **Cranelift `-O`** | **0,66 s** | **1,01 s** | **254 ms** | **1,29 s** | **36 ms** | 1636 KB |

Punkty odniesienia: ttfx liczy 60 wierszy w 56 ms, a 180 wierszy w 218 ms. Sowa → Bun liczy 60 wierszy w 2,48 s.

Wnioski:

- **Cranelift z `-O`** kompiluje 3,5–5,5 razy szybciej niż LLVM z `-O`. Program jest ok. 5 razy wolniejszy niż z LLVM, ale nadal ok. 10 razy szybszy niż w Bunie. Pętla w fakturownia_web spada z 6,4 s do 1,3 s.
- **Bez optymalizacji Cranelift nie pomaga.** LLVM `-O0` kompiluje prawie tak samo szybko jak Cranelift `-O0`. Przy `-O0` czas zajmuje front rustc, czyli sprawdzanie typów i borrow checker, a nie generowanie kodu. Ten front zostaje przy każdym ustawieniu, więc pętla przez Rusta nie zejdzie poniżej ok. 0,5–0,8 s.
- **`codegen-units=1`** wydłuża kompilację o 15–20%, a przyspiesza program o 1–2%. Przy wydaniu to się opłaca, przy testach nie.
- **Wada Cranelift:** działa tylko z nocnym rustc i komponentem `rustc-codegen-cranelift-preview`. Stabilny Rust z Homebrew go nie ma.

## Inne możliwe cele

Tych dróg nie mierzono. Oceny pochodzą z opisów samych projektów.

| Cel | Program | Kompilacja | Nakład | Co daje Sowie |
|---|---|---|---|---|
| **C** (tcc w pracy, clang na wydanie) | jak Rust (clang też używa LLVM) | tcc: dziesiątki ms | drugi generator i runtime w C; pamięć i granice tablic sprawdza sam generator | jedna droga do pracy i do wydania; tak kompilują Koka i Lean, z tym samym modelem pamięci co Sowa (liczenie referencji i zmiana w miejscu) |
| **WebAssembly** (wasmtime, przeglądarka, Bun) | ok. 1,2–2 razy wolniej niż natywnie | jak Rust | mały: obecny Rust z `--target wasm32-wasip1` | piaskownica dla [kodu użytkowników](przemyslenia.md#kod-użytkowników); uprawnienia Sowy stają się importami WASI |
| **QBE** | ok. 70% szybkości LLVM | bardzo szybka | średni | mały backend bez zależności; nie ma Windowsa |
| **LLVM IR bezpośrednio** | jak teraz | oszczędza tylko front rustc (ok. 0,5 s) | największy | w praktyce nic ponad Rusta |
| **Zig** (kod Zig albo `zig cc`) | jak C | średnia | jak C | łatwa kompilacja pod inne systemy |
| **Go** | ok. 1,5–2 razy wolniej | bardzo szybka | średni | ma garbage collector, więc ginie zmiana w miejscu, na której stoi szybkość backendu Rust |
| **własny asembler** | najwyższa, np. SIMD | natychmiastowa | ogromny, osobno dla każdego procesora | tylko dla wąskich pętli, nie jako backend języka |
| `bun build --compile` | jak Bun | szybka | zero | jeden plik do rozprowadzenia, ale to nadal Bun, a nie kod natywny |

## Propozycje do decyzji

Poniższe zmiany dotyczą kompilatora, a nie języka:

1. **`sowa test` z `codegen-units=16`.** Kompilacja jest o 15–20% krótsza, a test nie odczuje 1–2% wolniejszego programu. `build` i `run` zostają z `codegen-units=1`.
2. **Cranelift w `sowa test`, gdy jest dostępny.** Na przykład opcja `--fast` albo automatyczne sprawdzenie, czy rustc ma komponent Cranelift. Pętla spada do ok. 1 s. Bez nocnego rustc wszystko działa jak dziś.
3. **Pomiar WebAssembly.** Najmniejszy nakład, bo obecny kod w Ruście prawdopodobnie skompiluje się do `wasm32-wasip1` prawie bez zmian. Pomiar pokaże, ile kosztuje piaskownica.
4. **Backend C na później.** Jeśli ma powstać jedna droga zamiast Buna i Rusta, najmocniejszym kandydatem jest C z tcc w pracy i clang na wydanie. To największa z tych zmian, więc najpierw warto zrobić punkty 2 i 3.

## Jak powtórzyć

Nocny rustc z Cranelift da się zainstalować w osobnym katalogu, bez zmian w systemie:

```sh
D=/tmp/rust-nightly
curl -sSfL -o /tmp/rustup-init https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init
chmod +x /tmp/rustup-init
RUSTUP_HOME=$D/home CARGO_HOME=$D/cargo /tmp/rustup-init -y --no-modify-path \
  --profile minimal --default-toolchain nightly -c rustc-codegen-cranelift-preview

# nakładka, przez którą sowa woła rustc z Cranelift
printf '#!/bin/sh\nRUSTUP_HOME=%s/home exec %s/cargo/bin/rustc "$@" -Zcodegen-backend=cranelift\n' $D $D > $D/rustc-cl
chmod +x $D/rustc-cl

rm -f examples/fakturownia_web/.sowa/test_rs
time SOWA_RUSTC=$D/rustc-cl compiler/target/release/sowa test examples/fakturownia_web
```

Przy kilku flagach `-C` rustc bierze ostatnią. Nakładka może więc nadpisać ustawienia Sowy, np. dopisać `-C codegen-units=16`.
