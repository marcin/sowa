# Założenia

## Zasada nadrzędna

Agent pisze, człowiek zatwierdza. Domyślnie człowiek zatwierdza granice i specyfikację: typy, sygnatury, efekty, opisy i przykłady. Kodu nie czyta: ciała funkcji leżą osobno, w `impl/`, i pilnują ich kompilator, testy i uprawnienia w runtime. Kod można też czytać, w wybranych modułach albo w całym projekcie (zob. [Tryby](#tryby-spec-i-code)). Z tego wynika reszta:

1. **Jak najmniej wiedzy do przeczytania.** Kod ma być zrozumiały dla kogoś, kto zna tylko podstawy: operatory, `if`, `return`, wywołania funkcji. Każda rzecz, którą trzeba sprawdzić w dokumentacji, to koszt.
2. **Dłuższe i oczywiste wygrywa z krótkim i sprytnym.** Agentowi nie przeszkadza pisanie więcej, a recenzentowi przeszkadza zgadywanie.
3. **Rygorystyczne granice, swobodne wnętrze.** Sygnatury, warunki na typach, efekty i błędy są jawne i sprawdzane. To one są recenzowane, a w trybie `spec` tylko one. Ciało funkcji może być żmudne, bo kompilator pilnuje zgodności z sygnaturą.
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

Linia `effects` pod sygnaturą mówi, co funkcja robi poza liczeniem. Funkcja bez `effects` jest czysta. Od kodu oddziela ją pusta linia (zob. [Dokumentacja](#dokumentacja-desc-doc-why-example)).

```
fn get_user(id: UserId) -> User
  effects Db.read

fn send_receipt(email: Email, receipt: Receipt)
  effects Net
```

Reguła jest jedna: jeśli wywołujesz funkcję z efektem, musisz ten efekt zadeklarować u siebie. Agent nie przemyci wywołania sieci w funkcji, która miała tylko czytać z bazy.

#### Ograniczenia w `sowa.toml`

`effects` przy funkcji mówi, co funkcja robi. Sekcja `[effects]` w `sowa.toml` mówi, na co projekt w ogóle pozwala. Sprawdza ją `sowa check`:

```toml
[effects]
allowed = ["Db.read", "Db.write", "Clock", "Net"]

[effects.files]
"src/issuing.sowa" = ["Db.read", "Db.write", "Clock"]
"src/sending.sowa" = ["Net"]

[effects.rules.Net]
why     = true
approve = true
```

| Klucz | Co znaczy |
|---|---|
| `allowed` | efekty, które mogą wystąpić w projekcie; każdy inny to błąd |
| `files` | w których plikach wolno użyć których efektów; plik, którego nie ma na liście, musi być czysty |
| `rules.<efekt>.why` | funkcja z tym efektem musi mieć `why` |
| `rules.<efekt>.approve = true` | funkcja, która dostaje ten efekt, czeka na zatwierdzenie przez człowieka (zob. niżej); zatwierdzenie obejmuje sygnaturę i efekty; w trybie `spec` taka zmiana stoi na górze listy w `sowa review` |
| `rules.<efekt>.approve = "body"` | jak wyżej, ale ponowne zatwierdzenie także po każdej zmianie ciała funkcji |

```
błąd: src/invoice.sowa: totals ma efekt Db.read, a w sowa.toml ten plik ma być czysty
błąd: efekt Random nie jest dozwolony w projekcie (sowa.toml, [effects] allowed)
błąd: send_invoice ma efekt Net, więc musi mieć why (sowa.toml, [effects.rules.Net])
ostrzeżenie: notify_buyer dostała efekt Net i czeka na zatwierdzenie (sowa review)
```

Bez sekcji `[effects]` projekt nie ma ograniczeń poza regułą z sygnatur.

To przesuwa założenie z poziomu „opisowe” na „sprawdzane”. Zdanie „sieć tylko przy wysyłce” w `.md` może się zdezaktualizować, a wpis w `sowa.toml` nie. `approve` łączy się z zasadą nadrzędną: agent może dopisać funkcję, która zapisuje do bazy albo łączy się z siecią, ale nie wejdzie ona bez zgody człowieka.

#### Zatwierdzanie: `sowa review`

Uzasadnienie i porównanie z innymi narzędziami: [zatwierdzanie.md](zatwierdzanie.md).

Zatwierdzanie działa w trzech krokach. Tak samo zatwierdza się opisy z `docs.lock` (punkt 5 w [Co sprawdza kompilator](#co-sprawdza-kompilator)).

**1. `sowa check` pokazuje, co czeka.** Lokalnie to ostrzeżenie, żeby nie blokować pracy. W CI (`sowa check --ci`) to błąd.

```
ostrzeżenie: notify_buyer (src/sending.sowa:21) dostała efekt Net i czeka na zatwierdzenie
             uruchom: sowa review
```

**2. Człowiek przegląda przez `sowa review`.** Polecenie jest interaktywne i pokazuje po kolei wszystko, co czeka, razem z tym, czego potrzeba do decyzji: sygnaturę, efekty, `desc`, treść sekcji z `why` i miejsca wywołania.

```
$ sowa review

[1/1] notify_buyer  src/sending.sowa:21  (nowa funkcja)

  fn notify_buyer(invoice: Invoice) -> Sent | SendError
    effects Net                                  ← wymaga zatwierdzenia (sowa.toml)

    desc Wysyła nabywcy przypomnienie o płatności.
    why decyzje/004-przypomnienia.md             → pokazać? [enter]

  wywoływana z: remind_unpaid (src/payments.sowa:40)

  Zatwierdzić Net dla notify_buyer? [t]ak / [n]ie / [p]omiń
```

Po zatwierdzeniu w `effects.lock` pojawia się wiersz z hashem, osobą (z `git config user.email`) i datą:

```
notify_buyer  src/sending.sowa  Net  7e21c4  reviewer@intum.com  2026-09-27
```

**3. Zatwierdzenie egzekwuje review kodu.** Agent też ma terminal, więc może uruchomić `sowa review` albo dopisać wiersz do pliku `.lock`. Sam plik niczego nie gwarantuje. Gwarancję daje to, że pliki `*.lock` i `sowa.toml` należą do właściciela w CODEOWNERS:

```
# .github/CODEOWNERS
sowa.toml   @marcin
*.lock      @marcin
```

- Agent może przygotować wpis, ale PR nie wejdzie bez zgody właściciela. Warunek: agent ma osobne konto bez prawa zatwierdzania i scalania, a ochrona `main` nie ma wyjątku dla adminów. Szczegóły w [zatwierdzanie.md](zatwierdzanie.md#kiedy-codeowners-nie-wystarcza).
- Diff pliku `.lock` jest listą kontrolną dla recenzenta: widać w nim dokładnie, co się zatwierdza („notify_buyer: Net”).
- Agent nie poluzuje też polityki w `sowa.toml` (`allowed`, `files`, `rules`) bez zgody człowieka.
- W AGENTS.md projektu jest reguła, że agent nie uruchamia `sowa review` i nie edytuje plików `*.lock`, a gdy `sowa check` zgłasza oczekujące zatwierdzenia, przekazuje je człowiekowi.

Bez PR albo gdy agent działa na koncie człowieka, można włączyć podpisywanie wpisów kluczem SSH, tak jak podpisuje się commity w gicie:

```toml
[review]
approvers = ["reviewer@intum.com"]
sign      = true
```

`sowa review` podpisuje wtedy każdy wpis, a `sowa check --ci` sprawdza podpis na liście `approvers`. Klucz musi wymagać potwierdzenia przy każdym użyciu (klucz sprzętowy, `ssh-add -c`), bo odblokowanego klucza w `ssh-agent` agent też może użyć. To opcja, a nie domyślne zachowanie, bo wymaga konfiguracji.

#### Mapa efektów modułu: `sowa effects`

Czasem nie chcesz zatwierdzać funkcji po kolei, tylko wiedzieć, gdzie w module powstają jakie efekty. `sowa effects` pokazuje to bez wchodzenia do funkcji. Raport jest pogrupowany po efektach, a nie po funkcjach:

```
$ sowa effects src/numbering.sowa

src/numbering.sowa   dozwolone: Db.read, Db.write (sowa.toml)

  Db.read   last_invoice_seq   ← save_with_next_number
  Db.write  save_invoice       ← save_with_next_number

  czyste: format_number
```

Przy efekcie stoi funkcja, w której on **powstaje**, czyli która sama wywołuje coś z tym efektem spoza modułu: z biblioteki albo z innego pliku. Po `przez` stoi wywołanie, z którego efekt przychodzi, np. `przez current_date()`, a po `←` funkcje z modułu, które ją wywołują i tylko przekazują efekt dalej. Dzięki temu lista zostaje krótka nawet w dużym module. Raport działa zawsze, bez konfiguracji, bo kompilator i tak liczy graf wywołań.

Żeby mapa modułu wymagała zatwierdzenia, wpisz moduł w `[review.files]`:

```toml
[review.files]
"src/numbering.sowa" = "effects"
```

- Zatwierdza się mapę modułu, a nie funkcje. `effects.lock` ma po jednym wierszu na efekt w module: efekt, funkcje, w których powstaje, hash, kto i kiedy.
- Hash obejmuje tylko pary (efekt, funkcja, w której powstaje). Zmiana w ciele funkcji, nowa funkcja czysta albo nowa funkcja, która tylko przekazuje efekt, nie wymagają ponownego zatwierdzenia.
- Nowy efekt w module albo nowe miejsce, w którym efekt powstaje, czeka na zatwierdzenie:

```
ostrzeżenie: src/numbering.sowa: zmieniła się mapa efektów i czeka na zatwierdzenie
               + Db.write  reset_numbering
             uruchom: sowa review
```

`[effects.files]` mówi, **jakie** efekty wolno mieć w pliku, a `[review.files]` każe zatwierdzać, **gdzie** w nim powstają. Gdy funkcja podlega też regule `approve` z `[effects.rules]`, `sowa review` pokazuje ją raz, a jedno zatwierdzenie zapisuje oba wiersze.

### Tryby: `spec` i `code`

Szczegóły, zagrożenia i porównanie z innymi językami: [tryby.md](tryby.md).

Tryb mówi, co człowiek czyta. Ustawia się go w `sowa.toml`:

```toml
[project]
src  = "src"
impl = "impl"

[review]
mode = "spec"                    # domyślnie; albo "code"
read = ["impl/numbering.sowa"]   # kod, który człowiek i tak czyta
```

| | `spec` (domyślny) | `code` |
|---|---|---|
| co czyta człowiek | `src/` i `docs/`: typy, sygnatury, efekty, opisy, przykłady | całe funkcje |
| gdzie są ciała funkcji | w `impl/`, w pliku o tej samej nazwie | w `src/`, pod dokumentacją |
| co chroni CODEOWNERS | `src/`, `docs/`, `sowa.toml`, `*.lock`, pliki z `read` | `sowa.toml`, `*.lock` |

W trybie `spec` plik w `src/` to specyfikacja modułu, bez ciał funkcji. Plik w `impl/` powtarza linię `fn` i `effects`, a pod nimi ma kod:

```
// src/types.sowa
fn read_nip(input: String) -> Nip | InvalidNip
  desc Zamienia NIP wpisany przez użytkownika (z kreskami lub spacjami) na Nip.

  example read_nip("123-456-32-18") == "1234563218"
  example read_nip("123") == InvalidNip

// impl/types.sowa
fn read_nip(input: String) -> Nip | InvalidNip
  digits = remove(remove(input, "-"), " ")
  return digits as Nip or return InvalidNip
```

- Sygnatura i `effects` w `impl/` muszą być identyczne jak w `src/`. Zmiana sygnatury wymaga więc zmiany w `src/`, a ta zgody człowieka.
- `impl/` może mieć własne funkcje pomocnicze i typy. Są prywatne: spoza modułu widać tylko `src/`.
- `example` w `src/` to specyfikacja, a w `impl/` to własne testy agenta.
- Wywołania bibliotek spoza Sowy mogą stać tylko w `src/`.
- `impl/` nie ma właściciela w CODEOWNERS i jest zwinięty w PR (`impl/** linguist-generated=true` w `.gitattributes`). `sowa check` sprawdza, czy CODEOWNERS obejmuje `src/`, `docs/` i pliki z `read`.

W trybie `code` nie ma `impl/`: funkcja ma ciało pod dokumentacją, jak w przykładach w tym dokumencie. Wtedy `approve`, mapa efektów modułu i `effects.lock` wskazują człowiekowi, które funkcje czytać.

#### Efekty z zasobem

Efekt można przypiąć do zasobu nazwanego w `sowa.toml`. Wtedy agent nie wyśle danych gdzie indziej, nawet w kodzie, którego nikt nie czyta:

```
fn send_invoice(invoice: Invoice) -> Sent | SendError
  effects Net(mail)
```

```toml
[effects.resources]
mail = "smtp.firma.pl:587"
```

Funkcje sieciowe przyjmują zasób zamiast adresu: `mail_send(mail, to, subject, pdf)`. Adres jako tekst, np. `http_post("https://gdzies.com", ...)`, się nie skompiluje. `sowa run` przekazuje zasoby do runtime jako jedyne dozwolone połączenia.

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

### Dokumentacja: `desc`, `doc`, `why`, `example`

Dokumentacja i założenia są częścią języka, a nie komentarzami. Kompilator je zna i sprawdza.

Każde założenie powinno trafić na najwyższy poziom, na jaki się da:

| Poziom | Przykład | Zapis |
|---|---|---|
| sprawdzane | „rabat jest od 0 do 100”, „nie łączy się z siecią” | typ z warunkiem, `effects` |
| testowane | „rabat 20% od 100 daje 80” | `example` |
| opisowe | co robi funkcja, instrukcja dla użytkownika, dlaczego tak | `desc`, `doc`, `why` |

Każde słowo ma jedną formę, więc po samym słowie widać, czy to tekst, czy odnośnik:

| Słowo | Co zawiera | Forma |
|---|---|---|
| `desc` | co robi funkcja albo typ, krótko | tekst bez cudzysłowu: w tej samej linii albo w bloku z wcięciem |
| `doc` | dokumentacja dla użytkownika | ścieżka do pliku `.md`, opcjonalnie z `#sekcją` |
| `why` | dlaczego tak: założenie, wymóg biznesowy albo decyzja | ścieżka do pliku `.md`, opcjonalnie z `#sekcją` |
| `example` | przykład, który uruchamia się jako test i pojawia się w dokumentacji | wyrażenie |

Linie stoją pod sygnaturą w stałej kolejności: `effects`, `desc`, `doc`, `why`, `example`. Każdą z nich można powtórzyć, np. dwie linie `why`, gdy funkcja wynika z dwóch decyzji. `desc`, `doc` i `why` działają też pod definicją typu.

Funkcja ma cztery grupy oddzielone pustą linią, żeby się nie zlewały. Kolejności i odstępów pilnuje formatter:

1. `effects`,
2. opis: `desc`, `doc`, `why`,
3. przykłady: `example`,
4. kod.

Przykłady są osobno, bo to kod, a nie tekst. Czyta się je inaczej niż opis, a przy kilku przykładach opis ginąłby w grupie.

```
fn issue_invoice(form: InvoiceForm) -> Invoice | IssueError
  effects Db.read, Db.write, Clock

  doc uzytkownik/faktury.md#wystawianie-faktury
  why decyzje/002-numeracja-bez-luk.md
  why decyzje/003-wysylka-osobno.md

  buyer = try read_buyer(form)
  ...
```

```
fn apply_discount(total: Money, pct: Percent) -> Money
  desc Odejmuje rabat procentowy od kwoty.
  doc rabaty.md#naliczanie-rabatu
  why decyzje/rabat-od-brutto.md

  example apply_discount(100, 20) == 80
  example apply_discount(100, 0) == 100

  return total - total * pct / 100
```

Brakującą grupę się pomija, bez podwójnych pustych linii. W typie z polami opis stoi na górze, a pola pod nią, po pustej linii.

Przed `fn` zawsze stoi pusta linia, żeby `desc` poprzedniej definicji nie zlewał się z sygnaturą funkcji. Typy mogą stać jeden pod drugim, także z opisem:

```
type Percent  = Int(α >= 0 && α <= 100)
  desc Rabat procentowy, od 0 do 100 włącznie.
type Quantity = Int(α > 0)
  desc Liczba sztuk na pozycji.

fn apply_discount(total: Money, pct: Percent) -> Money
```

`desc` na kilka linii działa tak samo jak blok po `or`: samo słowo, a pod nim tekst z wcięciem. Blok kończy się tam, gdzie kończy się wcięcie.

```
fn read_discount(input: Int) -> Percent | DiscountError
  desc
    Zamienia rabat wpisany przez użytkownika na Percent.
    Wartości spoza zakresu nie są przycinane do 0 albo 100,
    tylko zwracają InvalidDiscount.
  why decyzje/bez-przycinania-rabatu.md
```

Reguły:

- **Tekst `desc` jest dosłowny.** Nie ma cudzysłowów ani znaków ucieczki. `"`, `#` i `//` w środku są zwykłym tekstem. Jedyny wyjątek to odnośnik `{Symbol}`, który działa tak samo jak w `.md` (zob. niżej).
- **`desc` jest krótki, reszta trafia do `.md`.** Kilka linii przy kodzie jest w porządku, a dłuższy tekst blokuje limit z `sowa.toml`. Instrukcja dla użytkownika to `doc`, a uzasadnienie to `why`.
- **Ścieżki liczą się od katalogu z dokumentacją** podanego w `sowa.toml` (`docs = "docs"`), więc nie powtarzamy `docs/` w każdej linii. Obowiązuje najbliższy `sowa.toml` w górę drzewa katalogów.
- **Sekcja** (`#nagłówek`) ma nazwę w stylu GitHuba: małe litery, spacje jako `-`, polskie litery zostają, np. `#płatność`.

#### Limity

Tekst i przykłady przy kodzie mają limity. Po przekroczeniu `sowa check` zgłasza ostrzeżenie z prośbą o przeniesienie do `.md`. Pilnuje tego `sowa check`, a nie formatter, bo formatter tylko układa kod i niczego nie zgłasza.

Wartości domyślne są w języku, a projekt może je zmienić w `sowa.toml`:

```toml
[limits]
desc_lines    = 3   # linii w desc; dłuższy opis przenieś do .md i wskaż przez doc
examples      = 3   # przykładów na funkcję; resztę przenieś do bloków sowa w .md
example_lines = 5   # linii w jednym przykładzie
```

```
ostrzeżenie: src/invoice.sowa:44 totals ma 4 przykłady (limit 3 z sowa.toml)
             przenieś część do bloku ```sowa w pliku .md, np. decyzje/001-vat-od-sumy-w-stawce.md
```

Przykład przeniesiony do bloku ` ```sowa ` w `.md` dalej jest testem. Często pasuje tam lepiej: przykład, który udowadnia decyzję, leży przy jej opisie.

Agent, który dostanie takie ostrzeżenie, mógłby po prostu usunąć przykład zamiast go przenieść. Dlatego `sowa check` pokazuje przy każdej funkcji wszystkie jej testy, z kodu i z `.md` razem:

```
totals: 2 testy (1 w kodzie, 1 w decyzje/001-vat-od-sumy-w-stawce.md)
```

Blok ` ```sowa ` liczy się jako test każdej funkcji, którą wywołuje. Spadek liczby testów widać w wyniku i w diffie.

#### Nagłówek pliku

`desc`, `doc` i `why` na samej górze pliku, bez wcięcia i przed pierwszą definicją, opisują cały plik. Tu trafiają odnośniki do ogólnych założeń, które dotyczą wielu funkcji naraz:

```
desc Faktura, pozycje i sumy.
why zalozenia.md#kwoty
why zalozenia.md#obliczenia-i-efekty

type Line
  ...
```

- Nagłówek zastępuje komentarz na początku pliku. Komentarz może się zdezaktualizować po cichu, a odnośnik w nagłówku kompilator sprawdza tak jak każdy inny.
- `desc`, `doc` albo `why` bez wcięcia w środku pliku to błąd, bo nie wiadomo, czego dotyczy.

#### Co sprawdza kompilator

1. **Plik istnieje.** Brak pliku po `doc` albo `why` to błąd kompilacji.
2. **Sekcja istnieje.** `#naliczanie-rabatu` musi odpowiadać nagłówkowi w pliku. Zmiana nagłówka w `.md` psuje build, a nie zostawia martwego linku.
3. **Odnośniki `{Symbol}`.** W `.md` i w `desc` można pisać `{apply_discount}` albo `{Percent}`.
   - Kompilator sprawdza, czy taki symbol istnieje. Zmiana nazwy w kodzie psuje build, a nie zostawia martwego odnośnika.
   - Typ z warunkiem renderuje się jako opis warunku, np. `{Percent}` → „od 0 do 100”, więc tekst dla użytkownika nie rozjedzie się z walidacją.
   - Funkcja renderuje się jako nazwa z linkiem do sygnatury.
   - Sekcja `.md`, w której stoi `{Symbol}`, jest powiązana z tym symbolem w `docs.lock` (punkt 5), nawet jeśli żaden `doc` ani `why` na nią nie wskazuje.
4. **Przykłady w `.md` są testami.** Blok kodu oznaczony `sowa` w pliku `.md` kompiluje się i uruchamia tak jak `example`.
5. **Wykrywanie nieaktualnego opisu.** Tekstu nie da się sprawdzić, ale da się wykryć, że mógł się zdezaktualizować. Kompilator zapamiętuje hash sygnatury, warunków i efektów z chwili, gdy ktoś zatwierdził opis. Gdy się zmienią, zgłasza ostrzeżenie:

   ```
   ostrzeżenie: rabaty.md#naliczanie-rabatu nie był przeglądany
                od zmiany apply_discount (zmieniła się sygnatura)
   ```

   Hashe trzymamy w osobnym pliku `docs.lock`, żeby nie zaśmiecać kodu. Zatwierdzenie opisu aktualizuje ten plik. To łączy się z zasadą nadrzędną: agent zmienia kod, a człowiek potwierdza, że założenie nadal obowiązuje.

Jedna sekcja `.md` może opisywać kilka funkcji. Ostrzeżenie pojawia się wtedy, gdy zmieni się którakolwiek z nich. Sekcja jest powiązana z symbolem na dwa sposoby: przez `doc` albo `why` w kodzie oraz przez `{Symbol}` w jej tekście. Dla nagłówka pliku hash obejmuje sygnatury wszystkich definicji w pliku.

## Poza zakresem (na razie)

Te pomysły padły, ale nie są jeszcze rozpisane. Szczegóły w [przemyslenia.md](przemyslenia.md).

- `spec` z `requires` / `ensures` i generowaniem testów property-based
- zapytania o program (`query callers(charge) where effects contains Net`)
- pochodzenie kodu (`@origin(agent: ..., reviewed: false)`) i polityki wdrożeń
- procesy z supervisorem i obserwowalnym stanem, w stylu Erlanga
- model pamięci i kompilacja (LLVM / WASM, własność jak w Ruście)
