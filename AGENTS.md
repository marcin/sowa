# Sowa

Projekt języka programowania na erę AI (na razie tylko specyfikacja i przykłady, bez kompilatora).

Przed pracą przeczytaj:
- docs/zalozenia.md – zasady i ustalona składnia (źródło prawdy)
- docs/przemyslenia.md – uzasadnienia, odrzucone warianty, otwarte pytania
- docs/zatwierdzanie.md – jak człowiek zatwierdza zmiany agenta (zatwierdzenie na końcu, sowa review, docs.lock, CODEOWNERS)
- docs/specyfikacja.md – specyfikacja w src/, kod w impl/, uprawnienia jako parametry
- docs/porownanie.md – porównanie z innymi językami
- docs/ocena.md – ocena projektu i proponowany następny krok (prototyp checkera)
- examples/*.sowa – przykłady; muszą być zgodne z docs/zalozenia.md

Zasady przy zmianach:
- Kryterium nadrzędne: jak najmniej wiedzy potrzebnej człowiekowi do przeczytania kodu.
- Nie wprowadzaj nowej składni w przykładach bez dopisania jej do zalozenia.md albo do otwartych pytań.
- Decyzje odrzucone wraz z powodem zapisuj w przemyslenia.md.
- Dokumentacja jest po polsku.

W projektach w Sowie (np. examples/invoices):
- Pracuj w jednym PR: zmieniaj `src/`, `impl/` i `docs/`, aż testy przejdą. Człowiek zatwierdza PR raz, gdy całość działa.
- Po zatwierdzeniu PR zmieniaj już tylko `impl/`. Zmiana w `src/`, `docs/`, `sowa.toml` albo `docs.lock` unieważnia zatwierdzenie (`sowa check --ci`).
- Nie osłabiaj warunków w typach i w wynikach i nie zmieniaj ani nie usuwaj `example` i `property` w `src/`, żeby przeszły testy. Popraw kod. Własne testy dopisuj w `impl/`.
- Gdy osłabienie specyfikacji jest naprawdę potrzebne, napisz w opisie PR dlaczego. `sowa review` i tak pokaże je na górze.
- Nie edytuj `docs.lock` i nie uruchamiaj `sowa review --approve`. Zatwierdza człowiek.
- Nie zmieniaj `[resources]` ani `[limits]` w `sowa.toml`. Jeśli uważasz, że trzeba, zaproponuj zmianę człowiekowi.
- Gdy `sowa check` zgłasza nieprzejrzane opisy, wypisz je w opisie PR.
