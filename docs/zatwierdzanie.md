# Zatwierdzanie: agent pisze, człowiek zatwierdza

Zasada nadrzędna Sowy brzmi: agent pisze, człowiek zatwierdza. Ten dokument opisuje, jak „zatwierdza” działa w praktyce. Składnia i reguły są w [zalozenia.md](zalozenia.md#zatwierdzanie-sowa-review), a tu jest całość w jednym miejscu, z uzasadnieniem.

To, co tu opisane (`approve`, `effects.lock`, mapa efektów modułu), służy do czytania kodu: w trybie `code` i w plikach, które w trybie `spec` człowiek i tak czyta (`[review] read`). W domyślnym trybie `spec` zatwierdzenie to przede wszystkim review zmian w `src/` i `docs/`, a `approve` tylko przesuwa zmianę na górę listy w `sowa review`. Zob. [tryby.md](tryby.md).

## Problem

Agent potrafi napisać dużo kodu szybko. Człowiek nie przeczyta każdej linii, więc musi wiedzieć, **które zmiany wymagają jego uwagi**. Zwykły diff tego nie mówi: nowa funkcja wysyłająca dane na zewnątrz wygląda w nim tak samo jak zmiana nazwy zmiennej.

Druga trudność: agent ma terminal. Każde zabezpieczenie, które da się „kliknąć” poleceniem albo edycją pliku, agent może obejść. Nie musi to być złośliwość. Wystarczy, że chce, żeby build przeszedł.

## Co wymaga zatwierdzenia

Tylko rzeczy, których kompilator nie sprawdzi sam i które zmieniają znaczenie programu:

| Co | Dlaczego człowiek | Gdzie zapisane |
|---|---|---|
| funkcja dostaje efekt oznaczony `approve` (np. `Net`, `Db.write`) | kompilator wie, że funkcja łączy się z siecią, ale nie wie, czy powinna | `effects.lock` |
| w module z `[review.files]` pojawia się nowy efekt albo nowe miejsce, w którym efekt powstaje | człowiek chce znać mapę efektów modułu, ale nie przeglądać funkcji po kolei | `effects.lock` |
| zmienia się sygnatura, warunek albo efekt funkcji opisanej w `.md` | tekstu nie da się sprawdzić, da się tylko wykryć, że mógł się zdezaktualizować | `docs.lock` |
| zmienia się polityka projektu (`[effects]`, `[limits]`, `[review]`) | to reguły, które pilnują całej reszty | `sowa.toml` |

Wszystko inne, czyli typy z warunkami, efekty w sygnaturach, wyczerpujący `match` i przykłady, sprawdza kompilator. Człowiek nie musi.

## Jak to działa

**1. `sowa check` mówi, co czeka.** Lokalnie jest to ostrzeżenie, żeby nie blokować pracy, a w CI (`sowa check --ci`) błąd.

```
ostrzeżenie: notify_buyer (src/sending.sowa:21) dostała efekt Net i czeka na zatwierdzenie
ostrzeżenie: uzytkownik/faktury.md#rabat nie był przeglądany od zmiany line_net
             uruchom: sowa review
```

**2. Człowiek przegląda przez `sowa review`.** Polecenie pokazuje po kolei wszystko, co czeka, razem z kontekstem potrzebnym do decyzji: sygnaturę, efekty, `desc`, treść sekcji z `why` i miejsca wywołania. Zatwierdzenie zapisuje w pliku `.lock` hash, osobę i datę.

```
[1/2] notify_buyer  src/sending.sowa:21  (nowa funkcja)

  fn notify_buyer(invoice: Invoice) -> Sent | SendError
    effects Net                                  ← wymaga zatwierdzenia (sowa.toml)

    desc Wysyła nabywcy przypomnienie o płatności.
    why decyzje/004-przypomnienia.md             → pokazać? [enter]

  wywoływana z: remind_unpaid (src/payments.sowa:40)

  Zatwierdzić Net dla notify_buyer? [t]ak / [n]ie / [p]omiń
```

**3. Review PR egzekwuje, że zatwierdził człowiek.** Pliki `*.lock` i `sowa.toml` należą do właściciela w CODEOWNERS. Agent może przygotować wpis, ale PR nie wejdzie bez zgody człowieka. Działa to tylko wtedy, gdy agent ma osobne konto (zob. [Kiedy CODEOWNERS nie wystarcza](#kiedy-codeowners-nie-wystarcza)).

```
# .github/CODEOWNERS
sowa.toml   @marcin
*.lock      @marcin
```

## Dlaczego tak

- **Nie ufamy plikowi, ufamy review.** Sam `.lock` niczego nie gwarantuje, bo agent może go edytować. Review PR już istnieje w każdym zespole i agent nie zatwierdzi sam swojego PR, o ile działa na własnym koncie (zob. [Kiedy CODEOWNERS nie wystarcza](#kiedy-codeowners-nie-wystarcza)). Sowa nie wymyśla nowego mechanizmu uprawnień, tylko mówi recenzentowi, na co patrzeć.
- **Diff pliku `.lock` to lista kontrolna.** Zamiast przeglądać 800 linii kodu, recenzent widzi w `effects.lock` jeden nowy wiersz: „notify_buyer: Net”. Tego nie przeoczy.
- **Zatwierdza się znaczenie, nie kod.** Hash obejmuje sygnaturę i efekty, a nie ciało funkcji. „Ta funkcja może łączyć się z siecią” zatwierdza się raz, a nie po każdej poprawce w środku. Dla wrażliwych efektów jest ostrzejsza opcja `approve = "body"`.
- **Polityka też jest chroniona.** Agent nie poluzuje `allowed` w `sowa.toml`, żeby ominąć regułę, bo ten plik też wymaga zgody właściciela.
- **Agent wie, czego nie robić.** Reguła w AGENTS.md mówi, że agent nie uruchamia `sowa review`, nie edytuje plików `*.lock` ani polityki w `sowa.toml`, a oczekujące zatwierdzenia przekazuje człowiekowi. Reguła nie jest zabezpieczeniem, zabezpieczeniem jest CODEOWNERS. Dzięki regule uczciwy agent nie marnuje jednak czasu recenzenta.

## Zatwierdzanie modułu zamiast funkcji

`approve` na efekcie działa na poziomie funkcji: każda nowa funkcja z `Db.write` czeka osobno. W dużym module to dużo zatwierdzeń, a człowiek często chce wiedzieć tylko jedno: gdzie w module dzieje się coś poza liczeniem.

Do tego służy mapa efektów modułu. `sowa effects` pokazuje ją zawsze:

```
$ sowa effects src/issuing.sowa

src/issuing.sowa   dozwolone: Db.read, Db.write, Clock (sowa.toml)

  Clock     issue_invoice   przez current_date()
  Db.read   issue_invoice   przez save_with_next_number (src/numbering.sowa)
  Db.write  issue_invoice   przez save_with_next_number (src/numbering.sowa)

  czyste: read_buyer
```

Moduł wpisany w `[review.files]` w `sowa.toml` wymaga zatwierdzenia tej mapy:

```toml
[review.files]
"src/issuing.sowa" = "effects"
```

`sowa review` pokazuje wtedy diff mapy, a nie funkcje:

```
[1/1] src/numbering.sowa  (mapa efektów)

    Db.read   last_invoice_seq
    Db.write  save_invoice
  + Db.write  reset_numbering        (nowa funkcja)

  Zatwierdzić mapę efektów? [t]ak / [n]ie / [f]unkcje / [p]omiń
```

Treść funkcji jest pod `[f]`, ale nie trzeba do niej zaglądać. W `effects.lock` zapisuje się jeden wiersz na efekt w module:

```
src/numbering.sowa   Db.write   save_invoice, reset_numbering   91c3d7  reviewer@intum.com     2026-09-27
```

Ponownego zatwierdzenia wymaga tylko nowy efekt albo nowe miejsce, w którym efekt powstaje. Zmiana w ciele funkcji, nowa funkcja czysta albo nowa funkcja, która tylko woła `save_invoice`, nic nie zmieniają. Mapę widać też w PR: diff pliku `effects.lock` to dokładnie te wiersze.

To lżejsze niż `approve` na częstym efekcie. Zamiast zatwierdzać każdą funkcję z `Db.write` osobno, człowiek zatwierdza jedną listę na moduł.

## Kiedy CODEOWNERS nie wystarcza

CODEOWNERS chroni tylko wtedy, gdy agent i człowiek to **dwie różne tożsamości**. W praktyce agent często działa na koncie człowieka: ten sam `gh auth`, ten sam token, ta sama tożsamość w gicie. Wtedy:

- autor PR i właściciel z CODEOWNERS to ta sama osoba, więc wymóg zgody właściciela albo nie da się spełnić, albo omija go uprawnienie admina,
- agent z tokenem admina może scalić PR z pominięciem reguł albo wypchnąć zmianę prosto na `main`.

CODEOWNERS działa, gdy spełnione są wszystkie warunki:

1. agent ma **osobne konto** (konto bota, GitHub App albo osobny token) bez prawa zatwierdzania i scalania,
2. gałąź `main` ma ochronę: wymagane review od właściciela z CODEOWNERS i **brak wyjątku dla adminów**,
3. token człowieka nie jest dostępny w środowisku agenta.

Jeśli choć jeden warunek nie jest spełniony, a zwłaszcza gdy ktoś pracuje sam z agentem na jednym koncie, zostaje podpis.

## Praca bez PR albo na jednym koncie

Kto pracuje sam z agentem, może włączyć podpisywanie wpisów kluczem SSH, tak jak podpisuje się commity w gicie:

```toml
[review]
approvers = ["reviewer@intum.com"]
sign      = true
```

`sowa review` podpisuje każdy wpis, a `sowa check --ci` sprawdza podpis na liście `approvers`.

Hasło do klucza nie wystarcza. Jeśli klucz jest odblokowany w `ssh-agent`, agent działający w tym samym terminalu też może nim podpisać. Klucz musi wymagać **potwierdzenia przy każdym użyciu**:

- klucz sprzętowy z dotknięciem (`ed25519-sk`, np. YubiKey),
- `ssh-add -c`, które przy każdym podpisie pyta w okienku systemowym,
- menedżer haseł, który pyta o zgodę przy każdym użyciu klucza (np. agent SSH w 1Password).

Wtedy agent może uruchomić `sowa review`, ale podpisu bez człowieka nie złoży.

## Jak to robią inni

- **[cargo-vet](https://github.com/mozilla/cargo-vet)** (Mozilla): audyty zależności zapisane w pliku z informacją, kto i co sprawdził. Najbliższy odpowiednik, ale dotyczy cudzych bibliotek, a nie własnego kodu.
- **Pliki lock** (`Cargo.lock`, `package-lock.json`, `go.sum`): hash zapisany w repozytorium i sprawdzany przy buildzie. Od nich wzięliśmy format, ale ich nikt nie zatwierdza, tylko generują się same.
- **[CODEOWNERS](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners)** (GitHub, GitLab): wymagana zgoda właściciela dla wybranych ścieżek. Sowa z tego korzysta, a nie zastępuje.
- **Uprawnienia w [Deno](https://docs.deno.com/runtime/fundamentals/security/)** (`--allow-net`): ograniczenie efektów, ale w czasie uruchomienia i dla całego procesu, a nie funkcji.
- **[Boruna](https://github.com/escapeboy/boruna)**: łańcuchy dowodowe z hashami dla wykonań, zob. [porownanie.md](porownanie.md).

Nie znaleźliśmy języka, w którym kompilator sam wskazuje, **które zmiany w kodzie wymagają decyzji człowieka**, i wiąże to z review. Razem z wykrywaniem nieaktualnych opisów (`docs.lock`) może to być wyróżnik Sowy. To ważne, bo sama czytelna składnia jest słabą przewagą, zob. [ocena.md](ocena.md).

## Otwarte pytania

- Czy połączyć `effects.lock` i `docs.lock` w jeden plik zatwierdzeń?
- Format podpisu (SSH jak w gicie?) i jak dodawać albo odwoływać osoby z `approvers`.
- Co z konfliktami w plikach `.lock`, gdy dwa PR zatwierdzają różne rzeczy? Może jeden wiersz na symbol i sortowanie, żeby merge był prosty.
- Czy `sowa review` ma pokazywać diff ciała funkcji od ostatniego zatwierdzenia, nawet przy `approve = true`?
- Zmęczenie zatwierdzaniem: jeśli `approve` stoi na częstym efekcie (np. `Db.write`), ludzie zaczną zatwierdzać bez czytania. Może `sowa check` powinien ostrzegać, gdy zatwierdzeń jest za dużo, albo zalecać `approve` tylko dla rzadkich efektów, jak `Net`, a dla reszty mapę modułu z `[review.files]`?
- Mapa modułu: czy funkcje, które tylko przekazują efekt dalej (po `←`), też powinny wchodzić do hasha? Teraz nie wchodzą, żeby mapa się nie zmieniała przy każdej nowej funkcji, ale wtedy nie widać, że efekt ma nowe wejście.
- Mapa modułu: czy `[review.files]` powinno przyjmować wzorce ścieżek (`"src/db/*.sowa"`)?
