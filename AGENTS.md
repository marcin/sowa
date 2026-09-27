# Sowa

Projekt języka programowania na erę AI (na razie tylko specyfikacja i przykłady, bez kompilatora).

Przed pracą przeczytaj:
- docs/zalozenia.md – zasady i ustalona składnia (źródło prawdy)
- docs/przemyslenia.md – uzasadnienia, odrzucone warianty, otwarte pytania
- docs/zatwierdzanie.md – jak człowiek zatwierdza zmiany agenta (sowa review, *.lock, CODEOWNERS)
- docs/tryby.md – tryb spec (człowiek zatwierdza specyfikację, kod w impl/) i tryb code
- docs/porownanie.md – porównanie z innymi językami
- docs/ocena.md – ocena projektu i proponowany następny krok (prototyp checkera)
- examples/*.sowa – przykłady; muszą być zgodne z docs/zalozenia.md

Zasady przy zmianach:
- Kryterium nadrzędne: jak najmniej wiedzy potrzebnej człowiekowi do przeczytania kodu.
- Nie wprowadzaj nowej składni w przykładach bez dopisania jej do zalozenia.md albo do otwartych pytań.
- Decyzje odrzucone wraz z powodem zapisuj w przemyslenia.md.
- Dokumentacja jest po polsku.

W projektach w Sowie (np. examples/invoices):
- Nie uruchamiaj `sowa review` i nie edytuj plików `*.lock`. Zatwierdza człowiek.
- W trybie spec (`[review] mode = "spec"`) kod w `impl/` możesz zmieniać swobodnie, poza plikami z `[review] read`. Zmiany w `src/` i `docs/` to zmiany specyfikacji: wymagają zgody człowieka, więc wypisz je w podsumowaniu pracy. Nie osłabiaj warunków w typach i nie zmieniaj ani nie usuwaj przykładów w `src/`, żeby przeszły testy. Własne testy dopisuj w `impl/`.
- Nie zmieniaj sekcji `[effects]`, `[review]` ani `[limits]` w `sowa.toml`. Jeśli uważasz, że trzeba, zaproponuj zmianę człowiekowi.
- Gdy `sowa check` zgłasza oczekujące zatwierdzenia albo nieprzejrzane opisy, wypisz je człowiekowi w podsumowaniu pracy.
