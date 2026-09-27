# Efekt decrypt z ttfx w Sowie

Program odtwarza efekt `decrypt` z [ttfx](https://github.com/omacom/ttfx), czyli z przepisanego na Rusta terminaltexteffects. Wynik ma być co do bajta taki sam jak z

```
ttfx --seed N --frame-rate 0 --ignore-terminal-dimensions decrypt
```

przy tym samym `N` w zmiennej `SEED`. Porównanie i pomiary są w [porownanie.md](porownanie.md).

## Model

Ekran to lista wierszy `Line`, a wiersz to lista komórek `Glyph` z gotowym tekstem z ostatniej klatki. Każdy znak z wejścia ma listę klatek `Frame`: tekst komórki z kolorem ANSI i liczbę taktów. Klatka animacji to jeden takt wszystkich aktywnych komórek, a potem cały ekran jako tekst. Wiersz, w którym żadna komórka nie zaczyna nowej klatki, zachowuje tekst z poprzedniej klatki, bo tekst komórki zmienia się tylko na początku klatki sceny.

W ttfx znak ma sceny połączone zdarzeniami: gdy scena się kończy, silnik od razu włącza następną. Tutaj sceny są z góry złączone przez `{chain}` w jedną listę klatek. Wynik jest ten sam, a pętla animacji nie potrzebuje zdarzeń.

## Wejście

Tekst dzieli się na wiersze po `\n`. Tab to spacje do najbliższej kolumny podzielnej przez 4. Spacje na końcu wierszy i puste wiersze na końcu znikają, jak w ttfx:

```sowa
read_canvas("x  \n\n\n") == [["x"]]
```

Pusty tekst albo same białe znaki: program wypisuje `NO INPUT.` i kończy się kodem 1, tak jak ttfx. Tekstu z `\r` poza końcem wiersza `\r\n` ani z sekwencjami ANSI program nie obsługuje i kończy się kodem 2. ttfx je interpretuje (kolory z wejścia, powrót karetki), ale to osobny kawałek pracy.

## Sceny

Każdy znak ma dwie sceny:

1. **Pisanie**: bloki `▉ ▓ ▒ ░` po 2 takty i jeden znak szyfru na 1 takt, każdy w losowym z trzech zielonych kolorów.
2. **Szyfrowanie**: 80 znaków szyfru po 2 takty, potem od 1 do 15 znaków po 3–5 albo 35–59 taktów, a na koniec właściwy znak w 11 kolorach od `ffffff` do `eda000` po 5 taktów. Wszystkie znaki szyfru jednego znaku mają ten sam kolor.

Najpierw znaki pojawiają się po dwa: w każdej klatce z szansą 76 na 101. Gdy wszystkie skończą pisanie, szyfrowanie rusza dla wszystkich naraz i trwa, aż ostatni znak skończy scenę.

## Losowanie

Liczby losuje uprawnienie `Random`: xoshiro256++ z ziarnem rozwiniętym przez SplitMix64, ten sam generator co w ttfx. Dlatego przy tym samym ziarnie obie implementacje dają te same liczby, jeśli losują w tej samej kolejności:

1. sceny pisania wszystkich znaków, w kolejności pisania,
2. sceny szyfrowania wszystkich znaków, w tej samej kolejności,
3. w każdej klatce pisania, póki zostały znaki: czy pojawiają się nowe.

`random.int(a, b)` to `randint(a, b)` z Pythona, z obiema granicami włącznie. `randrange(35, 60)` z ttfx to więc `random.int(35, 59)`.

## Wyjście

Przed pierwszą klatką program ukrywa kursor, wypisuje tyle wierszy spacji, ile ma ekran, i zapamiętuje pozycję kursora (`ESC 7`). Każda klatka wraca do tej pozycji, cofa kursor o wysokość ekranu i wypisuje wszystkie wiersze. Na końcu kursor wraca na ekran. Pusta komórka to spacja, a znak to `ESC[38;2;R;G;Bm`, sam znak i `ESC[0m`.
