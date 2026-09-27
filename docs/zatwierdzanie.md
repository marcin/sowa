# Zatwierdzanie: agent pisze, człowiek zatwierdza

Zasada nadrzędna Sowy brzmi: agent pisze, człowiek zatwierdza. Ten dokument opisuje, jak „zatwierdza” działa w praktyce. Składnia i reguły są w [zalozenia.md](zalozenia.md#zatwierdzanie), a tu jest całość w jednym miejscu, z uzasadnieniem.

## Problem

Agent potrafi napisać dużo kodu szybko. Człowiek nie przeczyta każdej linii, więc musi wiedzieć, **które zmiany wymagają jego uwagi**. Zwykły diff tego nie mówi: nowa funkcja wysyłająca dane na zewnątrz wygląda w nim tak samo jak zmiana nazwy zmiennej.

Druga trudność: agent ma terminal. Każde zabezpieczenie, które da się „kliknąć” poleceniem albo edycją pliku, agent może obejść. Nie musi to być złośliwość. Wystarczy, że chce, żeby build przeszedł.

Trzecia: czas człowieka. Zatwierdzanie specyfikacji przed kodem zmusza do czytania pomysłu, który w trakcie implementacji i tak się zmieni. Zatwierdzanie po kodzie grozi tym, że człowiek przepuści specyfikację, którą agent dopasował do kodu.

## Co wymaga zatwierdzenia

| Co | Dlaczego człowiek | Gdzie |
|---|---|---|
| specyfikacja: typy, warunki, sygnatury z uprawnieniami, `example`, `property` | kompilator sprawdzi, że kod jej się trzyma, ale nie wie, czy to dobra specyfikacja | `src/` |
| dokumentacja i decyzje | tekstu nie da się sprawdzić | `docs/` |
| opis funkcji, której sygnatura się zmieniła | tekst mógł się zdezaktualizować | `docs.lock` |
| zasoby i limity | adres serwera pocztowego albo bazy to decyzja człowieka | `sowa.toml` |
| opcjonalnie: kod wybranych modułów | gdy poprawności nie da się wyrazić w specyfikacji | pliki z `impl/` dopisane do CODEOWNERS |

Wszystko inne, czyli to, że kod trzyma się typów, uprawnień i przykładów, sprawdza kompilator. Człowiek nie musi.

## Zatwierdzenie na końcu

**1. Agent buduje.** W jednym PR zmienia `src/`, `impl/` i `docs/`, tyle razy, ile trzeba. Specyfikacja zmienia się razem z kodem. PR jest szkicem i niczego się w nim jeszcze nie zatwierdza. CI przy każdym pushu odświeża komentarz z `sowa review`, więc człowiek może zajrzeć wcześniej, ale nie musi.

**2. Całość działa.** Testy przechodzą, człowiek może program uruchomić albo obejrzeć podgląd. Agent oznacza PR jako gotowy.

**3. Człowiek zatwierdza raz.** Czyta wynik `sowa review --base main` (zob. niżej), w razie potrzeby zagląda do plików w `src/` i zatwierdza PR. Lista pokazuje zmianę netto względem `main`, a nie historię poprawek w PR. Jeśli agent w trakcie pracy poluzował warunek albo usunął test, to jest na górze listy, z kontrprzykładem. Jeśli osłabienie jest niepożądane, człowiek je odrzuca, a agent poprawia kod, a nie specyfikację.

**4. Po zatwierdzeniu specyfikacja stoi.** Agent może jeszcze poprawiać `impl/`. `sowa check --ci` pobiera z API GitHuba ostatnie zatwierdzenie PR przez właściciela z CODEOWNERS i commit, na którym je złożył. Potem sprawdza, czy od tego commitu zmienił się którykolwiek plik z właścicielem:

```
błąd: specyfikacja zmieniła się po zatwierdzeniu (zatwierdzone na 3f2a91c)
        src/issuing.sowa
      potrzebne ponowne zatwierdzenie PR
```

Przy ponownym zatwierdzeniu `sowa review --base 3f2a91c` pokazuje tylko to, co zmieniło się od poprzedniego.

Ustawienia w ochronie `main`:

- `sowa check --ci` jest wymaganym checkiem,
- wymagane jest review od właściciela z CODEOWNERS,
- „Dismiss stale approvals when new commits are pushed” jest **wyłączone**. Gdyby było włączone, każda poprawka w `impl/` kasowałaby zatwierdzenie, a człowiek musiałby zatwierdzać kod, którego nie czyta. Zmianę specyfikacji po zatwierdzeniu i tak wyłapie check z kroku 4.

Check musi się uruchamiać także po każdym zatwierdzeniu (`pull_request_review`), a checkout potrzebuje pełnej historii, żeby zatwierdzony commit był w repozytorium. Przykładowy workflow jest w [compiler/README.md](../compiler/README.md#przegląd-i-zatwierdzanie).

Dlaczego nie odwrotnie, czyli specyfikacja zatwierdzana przed kodem: człowiek czytałby pomysł, który nie wiadomo, czy zadziała. W trakcie implementacji prawie zawsze wychodzi coś, co zmienia specyfikację, więc zatwierdzałby dwa razy, a za pierwszym razem na próżno. Zatwierdzanie gotowej, działającej zmiany jest tańsze, a pułapkę dopasowania specyfikacji do kodu rozbraja porównanie z `main`.

## `sowa review`: zmiany w znaczeniu

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

Kolejność jest stała: uprawnienie, osłabienie, usunięty test, rozszerzenie, zwykłe (tabela w [zalozenia.md](zalozenia.md#sowa-review)). Nie trzeba niczego ustawiać: nowe uprawnienie jest zawsze na górze, bo zawsze znaczy „kod może zrobić coś, czego wcześniej nie mógł”.

**Osłabienie sprawdza solver.** Dla każdego zmienionego warunku `sowa review` pyta, czy stary warunek wynika z nowego:

- wynika: warunek jest taki sam albo ostrzejszy,
- nie wynika: warunek jest luźniejszy, a solver podaje kontrprzykład, np. „dopuszcza 101”,
- solver nie wie, np. przy arytmetyce nieliniowej: zmiana trafia do osłabień z dopiskiem „nie udało się porównać”.

Kierunek zależy od miejsca. Luźniejszy warunek w definicji typu albo w wyniku to osłabienie, bo reszta programu na nim polega. Luźniejszy warunek na parametrze to rozszerzenie: funkcja przyjmuje więcej danych, co widać, ale nic nie psuje. Ostrzejszy warunek na parametrze wyłapie kompilator u każdego wywołującego.

## Opisy: `docs.lock`

Tekstu nie da się sprawdzić, ale da się wykryć, że mógł się zdezaktualizować. `docs.lock` trzyma hash sygnatury z warunkami i uprawnieniami z chwili, gdy człowiek potwierdził opis. Po zmianie sygnatury `sowa check` wskazuje akapity do przejrzenia, a `sowa review` na końcu pyta o każdy. Szczegóły w [zalozenia.md](zalozenia.md#co-sprawdza-kompilator).

To jedyny plik z zatwierdzeniami w projekcie. Leży pod CODEOWNERS, więc agent może przygotować wpis, ale PR bez zgody właściciela nie wejdzie.

## Dlaczego tak

- **Gwarancję daje review, a nie plik.** Review PR już istnieje w każdym zespole i agent nie zatwierdzi sam swojego PR, o ile działa na własnym koncie (zob. [Kiedy CODEOWNERS nie wystarcza](#kiedy-codeowners-nie-wystarcza)). Sowa nie wymyśla nowego mechanizmu uprawnień, tylko mówi recenzentowi, na co patrzeć, i pilnuje, żeby po zatwierdzeniu specyfikacja się nie zmieniła.
- **Zatwierdza się znaczenie, nie kod.** Człowiek zatwierdza „ta funkcja może wysyłać e-maile przez smtp.firma.pl”, a nie każdą poprawkę w jej środku.
- **Jedno zatwierdzenie na PR.** Nie ma zatwierdzania funkcji po kolei, więc nie ma zmęczenia zatwierdzaniem, w którym po tygodniu klika się bez czytania.
- **Polityka też jest chroniona.** Agent nie doda zasobu w `sowa.toml`, bo ten plik też wymaga zgody właściciela.
- **Proces jest sprawdzany.** `sowa check` sprawdza, czy CODEOWNERS obejmuje `src/`, `docs/`, `sowa.toml` i `docs.lock`. Bez tego wszystkie powyższe zabezpieczenia byłyby tylko umową.

## Kiedy CODEOWNERS nie wystarcza

CODEOWNERS chroni tylko wtedy, gdy agent i człowiek to **dwie różne tożsamości**. W praktyce agent często działa na koncie człowieka: ten sam `gh auth`, ten sam token, ta sama tożsamość w gicie. Wtedy:

- autor PR i właściciel z CODEOWNERS to ta sama osoba, więc wymóg zgody właściciela albo nie da się spełnić, albo omija go uprawnienie admina,
- agent z tokenem admina może scalić PR z pominięciem reguł albo wypchnąć zmianę prosto na `main`.

CODEOWNERS działa, gdy spełnione są wszystkie warunki:

1. agent ma **osobne konto** (konto bota, GitHub App albo osobny token) bez prawa zatwierdzania i scalania,
2. gałąź `main` ma ochronę: wymagane review od właściciela z CODEOWNERS, wymagany `sowa check --ci` i **brak wyjątku dla adminów**,
3. token człowieka nie jest dostępny w środowisku agenta.

Jeśli choć jeden warunek nie jest spełniony, a zwłaszcza gdy ktoś pracuje sam z agentem na jednym koncie, zostaje podpis.

## Praca bez PR albo na jednym koncie

Kto pracuje sam z agentem, może zatwierdzać specyfikację podpisanym commitem:

```toml
[review]
approvers = ["reviewer@intum.com"]
sign      = true
```

`sowa review --approve` robi pusty commit podpisany kluczem SSH, tak jak podpisuje się commity w gicie. `sowa check --ci` sprawdza, czy ostatni taki commit ma podpis osoby z `approvers` i czy od niego nie zmienił się żaden plik specyfikacji. To ten sam check co w kroku 4 [Zatwierdzenia na końcu](#zatwierdzenie-na-końcu), tylko zatwierdzeniem jest commit, a nie review w PR.

Hasło do klucza nie wystarcza. Jeśli klucz jest odblokowany w `ssh-agent`, agent działający w tym samym terminalu też może nim podpisać. Klucz musi wymagać **potwierdzenia przy każdym użyciu**:

- klucz sprzętowy z dotknięciem (`ed25519-sk`, np. YubiKey),
- `ssh-add -c`, które przy każdym podpisie pyta w okienku systemowym,
- menedżer haseł, który pyta o zgodę przy każdym użyciu klucza (np. agent SSH w 1Password).

Wtedy agent może uruchomić `sowa review --approve`, ale podpisu bez człowieka nie złoży.

Klucze do sprawdzenia podpisu nie mogą leżeć w repozytorium, bo agent dopisałby tam swój. Wpis `"@login"` w `approvers` bierze klucze do podpisu (Signing keys) z konta na GitHubie, a adres e-mail wiersze z pliku `allowed_signers` spoza repozytorium, wskazanego przez `SOWA_ALLOWED_SIGNERS` albo `git config gpg.ssh.allowedSignersFile`. Odwołanie osoby to usunięcie jej z `approvers` (zmiana w `sowa.toml`, więc też wymaga zatwierdzenia) albo usunięcie klucza z konta. Jak to działa w kompilatorze, opisuje [compiler/README.md](../compiler/README.md#przegląd-i-zatwierdzanie).

## Drugi agent jako recenzent

Pomysł: jeden agent pisze kod, drugi, „pewniejszy”, go zatwierdza. Człowiek miałby wtedy mniej pracy. Sprawdzi się to jako filtr przed człowiekiem, ale nie zamiast niego:

- **Błędy się nakładają.** Dwa modele, zwłaszcza z tej samej rodziny, mają podobne ślepe plamy. Czego nie zauważył autor, często nie zauważy recenzent. Modele oceniają też łagodniej teksty podobne do własnych.
- **Autor może przekonać recenzenta.** Opis PR, komentarze w kodzie i `desc` to tekst, który czyta recenzent. Agent piszący, nawet bez złych zamiarów, uzasadni w nim osłabienie tak, że brzmi rozsądnie. W skrajnym przypadku to wstrzyknięcie poleceń.
- **Recenzent nie wie, czego chciał człowiek.** Zna tylko treść zadania. Nie oceni, czy rabat do 1000% to pomyłka, czy nowa oferta.

Dlatego agent-recenzent w Sowie nie decyduje o tym, co może zatwierdzić. Decydują kategorie z `sowa review`, czyli kompilator:

| Kategoria w `sowa review` | Kto zatwierdza |
|---|---|
| uprawnienie, osłabienie, usunięty test, zmiana w `docs.lock` | tylko człowiek; agent-recenzent pisze uwagi w PR |
| rozszerzenie, zwykłe | agent-recenzent albo człowiek |

Warunki, żeby to działało:

1. **Osobne konto** agenta-recenzenta (`[review] agent` w `sowa.toml`), z prawem zatwierdzania PR, ale bez prawa zapisu do repozytorium. Konto jest w zespole z CODEOWNERS, więc jego zatwierdzenie spełnia wymóg GitHuba, a `sowa check --ci` pilnuje reszty.
2. **Inny model niż u autora**, najlepiej od innego dostawcy. Błędy nakładają się wtedy rzadziej.
3. **Recenzent dostaje treść zadania, wynik `sowa review` i diff `src/`**, a nie opis PR, historię rozmowy ani uzasadnienia autora. Ocenia, czy specyfikacja pasuje do zadania, a nie czy autor ją dobrze obronił.
4. **`sowa check --ci` sprawdza kategorie**, a nie ufa recenzentowi: zatwierdzenie konta z `[review] agent` przy zmianie z kategorii „tylko człowiek” się nie liczy.

Zysk jest największy przy małych, częstych zmianach: nowa czysta funkcja, dodatkowy `example`, poprawka `desc`. Tam człowiek i tak klikałby „zatwierdź” bez czytania. Przy zmianach, które coś otwierają albo luzują, recenzent skraca pracę człowieka, bo wskazuje, na co patrzeć, ale jej nie zastępuje.

## Jak to robią inni

- **Design by contract** (Eiffel, SPARK, TLA+): kontrakt jest częścią kodu i jest sprawdzany, ale żadne narzędzie nie pokazuje recenzentowi, że kontrakt w zmianie jest słabszy niż przed nią.
- **[cargo-vet](https://github.com/mozilla/cargo-vet)** (Mozilla): audyty zależności zapisane w pliku z informacją, kto i co sprawdził. Dotyczy cudzych bibliotek, a nie własnego kodu.
- **Narzędzia do zmian w API** ([buf breaking](https://buf.build/docs/breaking/), [cargo-semver-checks](https://github.com/obi1kenobi/cargo-semver-checks)): wykrywają zmiany w interfejsie, które psują wywołujących. `sowa review` robi coś podobnego dla warunków i uprawnień, z solverem zamiast listy reguł.
- **[CODEOWNERS](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)** (GitHub, GitLab): wymagana zgoda właściciela dla wybranych ścieżek. Sowa z tego korzysta, a nie zastępuje.
- **[Boruna](https://github.com/escapeboy/boruna)**: łańcuchy dowodowe z hashami dla wykonań, zob. [porownanie.md](porownanie.md).

## Otwarte pytania

- GitLab i inne platformy: jak `sowa check --ci` ma odczytać, na którym commicie człowiek zatwierdził zmianę?
- Duży PR ze specyfikacją: czy `sowa review` ma grupować zmiany po module, gdy jest ich kilkadziesiąt?
- Duża zmiana, przy której warto zapytać o kierunek wcześniej: wystarczy komentarz w szkicu PR z bieżącym `sowa review`, czy potrzebne jest coś więcej?
- Co z konfliktami w `docs.lock`, gdy dwa PR zatwierdzają różne opisy? Może jeden wiersz na symbol i sortowanie, żeby merge był prosty.
- Agent-recenzent: czy „usunięty test” zastąpiony mocniejszym `property` też musi iść do człowieka? Solver mógłby sprawdzić, że nowy test obejmuje stary.
