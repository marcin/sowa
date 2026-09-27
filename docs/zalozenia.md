# Założenia

## Zasada nadrzędna

Agent pisze, człowiek zatwierdza. Człowiek zatwierdza specyfikację: typy, sygnatury z uprawnieniami, opisy i przykłady. Robi to raz, gdy całość już działa, a kodu nie czyta: ciała funkcji leżą osobno, w `impl/`, i pilnują ich kompilator, testy i uprawnienia (zob. [Specyfikacja i kod](#specyfikacja-i-kod) i [Zatwierdzanie](#zatwierdzanie)). Z tego wynika reszta:

1. **Jak najmniej wiedzy do przeczytania.** Kod ma być zrozumiały dla kogoś, kto zna tylko podstawy: operatory, `if`, `return`, wywołania funkcji. Każda rzecz, którą trzeba sprawdzić w dokumentacji, to koszt.
2. **Dłuższe i oczywiste wygrywa z krótkim i sprytnym.** Agentowi nie przeszkadza pisanie więcej, a recenzentowi przeszkadza zgadywanie.
3. **Rygorystyczne granice, swobodne wnętrze.** Sygnatury, warunki na typach, uprawnienia i błędy są jawne i sprawdzane. To one są recenzowane, i tylko one. Ciało funkcji może być żmudne, bo kompilator pilnuje zgodności z sygnaturą.
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

#### Warunek wyniku

Warunek można też napisać przy typie wyniku. `α` oznacza wtedy wynik, a warunek może odwoływać się do parametrów:

```
fn apply_discount(total: Money, pct: Percent) -> Money(α <= total)

fn totals(lines: List<Line>(len(α) > 0), discount: Percent) -> Totals(α.gross == α.net + α.vat)

fn issue_invoice(form: InvoiceForm, db: Db, clock: Clock) -> Invoice(α.status == Issued) | IssueError
```

Przy wyniku z błędami warunek dotyczy tylko wariantu, przy którym stoi. Nie ma osobnego słowa `ensures`: warunek wyniku zapisuje się tak samo jak warunek parametru.

Kompilator próbuje udowodnić warunek wyniku przy każdym `return`. Jeśli nie umie, np. przy `total - total * pct / 100`, wstawia sprawdzenie w runtime i sprawdza ten warunek testami na danych generowanych z typów. `sowa review` pokazuje, które warunki są udowodnione, a które tylko sprawdzane. Niespełniony warunek wyniku to błąd programu, a nie wariant w typie wyniku, bo oznacza błąd w kodzie, a nie w danych.

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

### Uprawnienia

Funkcja może zrobić coś poza liczeniem tylko przez **uprawnienie**, które dostała w parametrze. Uprawnienie to wartość jednego z wbudowanych typów, np. `Db`, `Clock` albo `Mailer`:

```
fn issue_invoice(form: InvoiceForm, db: Db, clock: Clock) -> Invoice | IssueError

fn send_invoice(invoice: Invoice, mail: Mailer) -> Sent | SendError
```

Operacje wywołuje się na uprawnieniu: `clock.today()`, `mail.send(to, subject, pdf)`, `db.transaction(...)`. Nie ma globalnych funkcji w rodzaju `current_date()` czy `http_post(url, ...)`, więc bez uprawnienia nie ma czym wysłać e-maila ani odczytać daty.

Reguły:

- **Funkcja bez uprawnień w parametrach jest czysta.** Nie łączy się z siecią, nie czyta zegara ani bazy. Widać to w sygnaturze, bez osobnego słowa kluczowego. Wyjątek to parametr, który sam jest funkcją (zob. [Funkcje przyjmujące funkcje](#funkcje-przyjmujące-funkcje)).
- **Uprawnienia nie da się utworzyć w kodzie.** Nie ma konstruktora `Mailer("smtp.gdzies.com")`. Wszystkie uprawnienia dostaje `main` od runtime, według `[resources]` w `sowa.toml` (zob. niżej), i przekazuje je dalej w parametrach.
- **Uprawnienie przechodzi tylko przez parametry.** Nie można go zapisać w polu rekordu, zwrócić z funkcji ani trzymać w zmiennej globalnej.
- **Uprawnienie można zawęzić, ale nie poszerzyć.** `Db` można przekazać tam, gdzie funkcja chce `DbRead`, ale nie odwrotnie.
- **Uprawnienia stoją na końcu listy parametrów.** Pilnuje tego formatter, żeby było je widać w jednym miejscu.

Kto wywołuje funkcję z uprawnieniem, musi to uprawnienie mieć. Agent nie przemyci wysyłki e-maila w funkcji, która miała tylko czytać z bazy, bo nie ma czym jej wykonać:

```
fn notify(id: UserId, receipt: Receipt, db: DbRead, mail: Mailer)
  user = get_user(id, db)
  send_receipt(user.email, receipt, mail)
```

Wbudowane typy uprawnień:

| Typ | Na co pozwala | Zasób w `sowa.toml` |
|---|---|---|
| `Db` | czytać i zapisywać w bazie | adres bazy |
| `DbRead` | tylko czytać z bazy | ten sam co `Db` |
| `Clock` | odczytać bieżącą datę i czas | – |
| `Random` | losować | – |
| `Log` | pisać do logów | – |
| `Mailer` | wysyłać e-maile przez jeden serwer | adres serwera |
| `Http` | wysyłać zapytania pod jeden adres | adres bazowy, np. `https://api.bank.pl` |
| `Files` | czytać i zapisywać pliki w jednym katalogu | katalog |

Własnych typów uprawnień na razie nie ma.

#### Zasoby: `[resources]` w `sowa.toml`

Uprawnienia, które program w ogóle dostaje, wymienia `sowa.toml`:

```toml
[resources]
db    = { type = "Db", url = "postgres://localhost/invoices" }
clock = { type = "Clock" }
mail  = { type = "Mailer", server = "smtp.firma.pl:587" }
```

```
fn main(db: Db, clock: Clock, mail: Mailer)
```

- Runtime przekazuje zasoby do `main` według nazw parametrów. Parametr bez zasobu o tej nazwie albo zasób innego typu to błąd kompilacji.
- Skoro uprawnienia nie da się utworzyć w kodzie, każdy `Mailer` w programie to `smtp.firma.pl:587`. `mail.send(...)` nie przyjmuje adresu serwera. Człowiek zatwierdza raz: „faktury wolno wysyłać przez smtp.firma.pl”.
- `sowa run` przekazuje te same adresy do runtime jako jedyne dozwolone połączenia. Przy kompilacji do TypeScriptu i uruchamianiu w Deno: `--allow-net=smtp.firma.pl:587,localhost:5432`. To druga linia obrony, na wypadek błędu w kompilatorze.
- Nowy zasób to zmiana w `sowa.toml`, a ten plik jest pod CODEOWNERS.

Nie ma osobnej listy „który plik może mieć jakie uprawnienia”. Pokazują to sygnatury w `src/`: jeśli żadna funkcja w `src/invoice.sowa` nie ma uprawnień, cały moduł jest czysty, razem z funkcjami pomocniczymi w `impl/`, bo nie mają skąd ich dostać.

#### Funkcje przyjmujące funkcje

Lambda może użyć uprawnienia z otoczenia:

```
fn save_all(invoices: List<Invoice>, db: Db)
  invoices.each(i => save_invoice(i, db))
```

`each` nie ma w sygnaturze żadnego uprawnienia. Może zrobić tylko to, co przekazana funkcja, a tę wywołujący mógł zbudować tylko z uprawnień, które sam ma. Pełna reguła brzmi więc tak: funkcja, która nie ma w parametrach ani uprawnień, ani funkcji, jest czysta. Funkcja z parametrem-funkcją może zrobić to, co ta funkcja, i nic więcej.

Dzięki temu sygnatura `map` czy `each` zostaje prosta. Przy efektach w sygnaturze (`effects Net`) trzeba by efektów generycznych, jak wiersze efektów w Koce.

### Specyfikacja i kod

Szczegóły, zagrożenia i porównanie z innymi językami: [specyfikacja.md](specyfikacja.md).

Człowiek zatwierdza specyfikację, a kodu nie czyta. Specyfikacja leży w `src/`, a ciała funkcji w `impl/`:

```toml
[project]
src  = "src"
impl = "impl"
```

Plik w `src/` to specyfikacja modułu, bez ciał funkcji. Plik w `impl/` powtarza linię `fn`, a pod nią ma kod:

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

- Sygnatura w `impl/` musi być identyczna jak w `src/`, razem z uprawnieniami. Zmiana sygnatury wymaga więc zmiany w `src/`, a ta zgody człowieka.
- `impl/` może mieć własne funkcje pomocnicze i typy. Są prywatne: spoza modułu widać tylko `src/`. Funkcja pomocnicza dostaje uprawnienie tylko od funkcji z `src/`, która je ma.
- `example` i `property` w `src/` to specyfikacja, a w `impl/` własne testy agenta.
- Wywołania bibliotek spoza Sowy mogą stać tylko w `src/`.
- `impl/` nie ma właściciela w CODEOWNERS i jest zwinięty w PR (`impl/** linguist-generated=true` w `.gitattributes`). `sowa check` sprawdza, czy CODEOWNERS obejmuje `src/`, `docs/`, `sowa.toml` i `docs.lock`.

Kod wybranego modułu można też czytać: wystarczy dopisać plik z `impl/` do CODEOWNERS i wyłączyć mu zwijanie w `.gitattributes` (`impl/numbering.sowa -linguist-generated`). `sowa.toml` nic o tym nie wie.

### Zatwierdzanie

Uzasadnienie i porównanie z innymi narzędziami: [zatwierdzanie.md](zatwierdzanie.md).

#### Zatwierdzenie na końcu

1. **Agent buduje.** W jednym PR zmienia `src/`, `impl/` i `docs/` tyle razy, ile trzeba. Specyfikacja może się zmieniać razem z kodem. Nikt niczego jeszcze nie zatwierdza.
2. **Całość działa.** Testy przechodzą, a człowiek może program uruchomić albo obejrzeć podgląd. Agent oznacza PR jako gotowy.
3. **Człowiek zatwierdza raz.** CI wkleja do PR wynik `sowa review --base main`, czyli zmiany w znaczeniu względem `main`, a nie historię poprawek w PR. Osłabienia i usunięte testy są na górze (zob. niżej). Człowiek zatwierdza PR.
4. **Po zatwierdzeniu specyfikacja stoi.** Agent może jeszcze poprawiać `impl/`, np. po uwagach z CI. `sowa check --ci` porównuje commit, na którym człowiek zatwierdził PR, z bieżącym. Jeśli od tego czasu zmienił się którykolwiek plik z właścicielem w CODEOWNERS, check nie przechodzi:

```
błąd: specyfikacja zmieniła się po zatwierdzeniu (zatwierdzone na 3f2a91c)
        src/issuing.sowa
      potrzebne ponowne zatwierdzenie PR
```

Człowiek nie traci czasu na specyfikację, która w trakcie pracy i tak się zmieni. Zatwierdza wersję końcową, a jednocześnie widzi, że działa.

Warunki: `sowa check --ci` jest wymaganym checkiem w ochronie `main`, a „Dismiss stale approvals” jest wyłączone, żeby poprawki w `impl/` nie kasowały zatwierdzenia. Agent ma osobne konto bez prawa zatwierdzania i scalania. Szczegóły w [zatwierdzanie.md](zatwierdzanie.md#kiedy-codeowners-nie-wystarcza).

Zatwierdzenie jest jedno na PR i obejmuje całą specyfikację. Nie ma zatwierdzania funkcji po kolei ani plików z zatwierdzeniami poza `docs.lock`.

#### `sowa review`

Polecenie porównuje specyfikację z gałęzią bazową i pokazuje zmiany w znaczeniu, a nie w tekście:

```
$ sowa review --base main

Specyfikacja (src/, docs/, sowa.toml): 5 zmian

  [1] uprawnienie     send_reminder  src/sending.sowa:24   nowa funkcja z Mailer (mail: smtp.firma.pl:587)
  [2] osłabienie      Percent        src/types.sowa:5      α <= 100  →  α <= 1000
                                                           dopuszcza np. 101
  [3] usunięty test   totals         src/invoice.sowa:50   property totals(lines, discount).net <= totals(lines, 0).net
  [4] rozszerzenie    read_nip       src/types.sowa:24     parametr: String(len(α) <= 20)  →  String
  [5] zwykłe          line_net       src/invoice.sowa:38   nowy example

Opisy do przejrzenia (docs.lock): uzytkownik/faktury.md#rabat
```

Kategorie mają stałą kolejność i nie wymagają konfiguracji:

| Kategoria | Co | Dlaczego tak wysoko |
|---|---|---|
| uprawnienie | funkcja dostaje nowe uprawnienie; nowy albo zmieniony zasób w `[resources]` | kod może zrobić coś, czego wcześniej nie mógł |
| osłabienie | luźniejszy warunek w definicji typu albo w wyniku; nowy wariant błędu w wyniku | gwarancje, na których polega reszta programu, są słabsze |
| usunięty test | usunięty albo zmieniony `example`, `property` lub blok `sowa` w `.md` | agent mógł dopasować test do kodu |
| rozszerzenie | luźniejszy warunek na parametrze | funkcja przyjmuje więcej danych; to widać, ale niczego nie psuje |
| zwykłe | reszta: nowy przykład, nowa czysta funkcja, zmiana `desc` | |

Osłabienie rozpoznaje solver: sprawdza, czy stary warunek wynika z nowego. Jeśli nie wynika, pokazuje kontrprzykład, np. „dopuszcza 101”. Gdy solver nie umie tego rozstrzygnąć, np. przy arytmetyce nieliniowej, zmiana trafia do osłabień z dopiskiem „nie udało się porównać”. Lepiej pokazać za dużo, niż przepuścić osłabienie.

Na końcu `sowa review` pyta o opisy, które mogły się zdezaktualizować. Potwierdzenie zapisuje wiersz w `docs.lock` (punkt 5 w [Co sprawdza kompilator](#co-sprawdza-kompilator)).

#### Bez PR albo na jednym koncie

Kto pracuje sam z agentem na jednym koncie, może zatwierdzać podpisanym commitem:

```toml
[review]
approvers = ["reviewer@intum.com"]
sign      = true
```

`sowa review --approve` robi pusty commit podpisany kluczem SSH, a `sowa check --ci` sprawdza, czy podpis należy do osoby z `approvers` i czy od tego commitu nie zmieniła się specyfikacja. Klucz musi wymagać potwierdzenia przy każdym użyciu (klucz sprzętowy, `ssh-add -c`), bo odblokowanego klucza w `ssh-agent` agent też może użyć. To opcja, a nie domyślne zachowanie, bo wymaga konfiguracji.

#### Drugi agent jako recenzent

Opcjonalnie drugi agent, na osobnym koncie i najlepiej z innym modelem, może zatwierdzać PR-y, w których `sowa review` pokazuje tylko rozszerzenia i zwykłe zmiany:

```toml
[review]
agent = "sowa-reviewer"   # konto agenta-recenzenta
```

`sowa check --ci` uznaje zatwierdzenie tego konta tylko wtedy, gdy w zmianie nie ma nowego uprawnienia, osłabienia, usuniętego testu ani zmiany w `docs.lock`. Wtedy potrzebny jest człowiek. Granicę wyznaczają kategorie z `sowa review`, a nie ocena agenta. Uzasadnienie i ryzyka: [zatwierdzanie.md](zatwierdzanie.md#drugi-agent-jako-recenzent).

### Błędy

Możliwe błędy są częścią typu wyniku i zapisuje się je przez `|`.

```
type PaymentError = CardDeclined | NoFunds | Timeout

fn charge(card: Card, amount: Price, bank: Http) -> Receipt | PaymentError
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

### Dokumentacja: `desc`, `doc`, `why`, `example`, `property`

Dokumentacja i założenia są częścią języka, a nie komentarzami. Kompilator je zna i sprawdza.

Każde założenie powinno trafić na najwyższy poziom, na jaki się da:

| Poziom | Przykład | Zapis |
|---|---|---|
| sprawdzane | „rabat jest od 0 do 100”, „rabat nie zwiększa kwoty”, „nie łączy się z siecią” | typ z warunkiem, warunek wyniku, uprawnienia w parametrach |
| testowane | „rabat 20% od 100 daje 80”, „rabat 0 nie zmienia kwoty” | `example`, `property` |
| opisowe | co robi funkcja, instrukcja dla użytkownika, dlaczego tak | `desc`, `doc`, `why` |

Każde słowo ma jedną formę, więc po samym słowie widać, czy to tekst, czy odnośnik:

| Słowo | Co zawiera | Forma |
|---|---|---|
| `desc` | co robi funkcja albo typ, krótko | tekst bez cudzysłowu: w tej samej linii albo w bloku z wcięciem |
| `doc` | dokumentacja dla użytkownika | ścieżka do pliku `.md`, opcjonalnie z `#sekcją` |
| `why` | dlaczego tak: założenie, wymóg biznesowy albo decyzja | ścieżka do pliku `.md`, opcjonalnie z `#sekcją` |
| `example` | przykład, który uruchamia się jako test i pojawia się w dokumentacji | wyrażenie |
| `property` | warunek, który ma być prawdziwy dla dowolnych danych (zob. niżej) | wyrażenie z nazwami parametrów |

Linie stoją pod sygnaturą w stałej kolejności: `desc`, `doc`, `why`, `example`, `property`. Każdą z nich można powtórzyć, np. dwie linie `why`, gdy funkcja wynika z dwóch decyzji. `desc`, `doc` i `why` działają też pod definicją typu.

Funkcja ma trzy grupy oddzielone pustą linią, żeby się nie zlewały. Kolejności i odstępów pilnuje formatter:

1. opis: `desc`, `doc`, `why`,
2. przykłady: `example`, `property`,
3. kod (w `impl/`).

Przykłady są osobno, bo to kod, a nie tekst. Czyta się je inaczej niż opis, a przy kilku przykładach opis ginąłby w grupie.

```
fn issue_invoice(form: InvoiceForm, db: Db, clock: Clock) -> Invoice(α.status == Issued) | IssueError
  doc uzytkownik/faktury.md#wystawianie-faktury
  why decyzje/002-numeracja-bez-luk.md
  why decyzje/003-wysylka-osobno.md

  buyer = try read_buyer(form)
  ...
```

```
fn apply_discount(total: Money, pct: Percent) -> Money(α <= total)
  desc Odejmuje rabat procentowy od kwoty.
  doc rabaty.md#naliczanie-rabatu
  why decyzje/rabat-od-brutto.md

  example apply_discount(100, 20) == 80
  property apply_discount(total, 0) == total

  return total - total * pct / 100
```

Przykłady w tym dokumencie mają ciało pod dokumentacją, żeby było widać całą funkcję. W projekcie ciało leży w `impl/`.

`property` to warunek, który ma być prawdziwy dla dowolnych danych. Nazwy parametrów funkcji oznaczają w nim dowolną wartość typu tego parametru. Dane generuje kompilator z typów z warunkami, np. dla `Percent` wartości brzegowe 0, 1, 99 i 100 oraz losowe. `example` sprawdza jeden przypadek, a `property` całą klasę przypadków, więc trudniej dopasować do niego kod. W `property` można użyć tylko nazw parametrów funkcji, pod którą stoi, i stałych.

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
examples      = 3   # example i property razem na funkcję; resztę przenieś do bloków sowa w .md
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
why zalozenia.md#obliczenia-i-uprawnienia

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
5. **Wykrywanie nieaktualnego opisu.** Tekstu nie da się sprawdzić, ale da się wykryć, że mógł się zdezaktualizować. Kompilator zapamiętuje hash sygnatury z warunkami i uprawnieniami z chwili, gdy ktoś zatwierdził opis. Gdy się zmienią, zgłasza ostrzeżenie:

   ```
   ostrzeżenie: rabaty.md#naliczanie-rabatu nie był przeglądany
                od zmiany apply_discount (zmieniła się sygnatura)
   ```

   Hashe trzymamy w osobnym pliku `docs.lock`, żeby nie zaśmiecać kodu. Zatwierdzenie opisu aktualizuje ten plik. To łączy się z zasadą nadrzędną: agent zmienia kod, a człowiek potwierdza, że założenie nadal obowiązuje.

Jedna sekcja `.md` może opisywać kilka funkcji. Ostrzeżenie pojawia się wtedy, gdy zmieni się którakolwiek z nich. Sekcja jest powiązana z symbolem na dwa sposoby: przez `doc` albo `why` w kodzie oraz przez `{Symbol}` w jej tekście. Dla nagłówka pliku hash obejmuje sygnatury wszystkich definicji w pliku.

## Poza zakresem (na razie)

Te pomysły padły, ale nie są jeszcze rozpisane. Szczegóły w [przemyslenia.md](przemyslenia.md).

- zapytania o program (`query callers(charge) where takes Http`)
- pochodzenie kodu (`@origin(agent: ..., reviewed: false)`) i polityki wdrożeń
- procesy z supervisorem i obserwowalnym stanem, w stylu Erlanga
- model pamięci i kompilacja (LLVM / WASM, własność jak w Ruście)
