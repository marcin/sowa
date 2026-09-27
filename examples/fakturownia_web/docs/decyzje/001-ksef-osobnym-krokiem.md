# 001. Wysyłka do KSeF jest osobnym krokiem

Dotyczy: {issue_invoice}, {send_to_ksef}, {post_invoice}, {post_ksef}

## Decyzja

{issue_invoice} tylko zapisuje fakturę. Do KSeF wysyła ją osobna funkcja {send_to_ksef}, uruchamiana przyciskiem na stronie faktury.

## Dlaczego

- Wystawienie to zapis w bazie (`db: Db, clock: Clock`), a wysyłka to sieć (`ksef: Http`). Gdyby były razem, awaria ksef.pl cofałaby wystawienie faktury albo zostawiała ją w niejasnym stanie, a numer mógłby przepaść.
- Sygnatury {issue_invoice} i {post_invoice} bez `Http` dają recenzentowi gwarancję, że wystawienie niczego nie wysyła na zewnątrz. Kompilator pilnuje, żeby tak zostało.
- Użytkownik może obejrzeć fakturę przed wysyłką.

## Konsekwencje

- Faktura ma dwa statusy: `Issued` i `SentToKsef` z numerem z KSeF. Wysłać można tylko fakturę w statusie `Issued`, co wynika z typu parametru {send_to_ksef}.
- Kto zechce wysyłać od razu przy wystawieniu, musi dopisać `ksef: Http` do sygnatury {issue_invoice}. `sowa review` pokaże to jako nowe uprawnienie, a `docs.lock` zgłosi tę decyzję do ponownego przejrzenia.
