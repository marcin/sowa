# 002. Numeracja bez luk

Dotyczy: {issue_invoice}, {save_with_next_number}

## Decyzja

Numer faktury nadajemy w tej samej transakcji, w której ją zapisujemy. Jeśli zapis się nie uda, numer nie jest zużyty.

## Dlaczego

- Numery faktur mają iść po kolei. Luka w numeracji zawsze budzi pytanie, co się stało z brakującą fakturą, i trzeba to wyjaśniać księgowej.
- Wcześniejszy pomysł, czyli rezerwowanie numeru przy otwarciu formularza, zostawiał luki, gdy użytkownik zamknął formularz bez zapisu.

## Konsekwencje

- Dwie faktury wystawiane równocześnie czekają na siebie przy nadawaniu numeru. Przy naszym ruchu to nie jest problem.
- Wystawionej faktury nie usuwamy. Błędy poprawia się fakturą korygującą (jeszcze nie ma jej w kodzie).
