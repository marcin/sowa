# 003. Wysyłka e-mailem jest osobnym krokiem

Dotyczy: {issue_invoice}, {send_invoice}

## Decyzja

{issue_invoice} nie wysyła e-maila. Wysyłka to osobna funkcja {send_invoice}, wywoływana po zapisaniu faktury.

## Dlaczego

- Wystawienie faktury to zapis w bazie (`db: Db, clock: Clock`). Wysyłka to poczta (`mail: Mailer`: tylko serwer z `[resources]` w `sowa.toml`). Gdyby były razem, awaria serwera pocztowego cofałaby wystawienie faktury albo zostawiała ją w niejasnym stanie.
- Sygnatura {issue_invoice} bez `Mailer` i bez `Http` daje recenzentowi gwarancję: wystawienie faktury niczego nie wysyła na zewnątrz. Kompilator pilnuje, żeby tak zostało, bo bez tego parametru funkcja nie ma czym wysłać.

## Konsekwencje

- Trzeba osobno obsłużyć ponowną wysyłkę, gdy e-mail nie dojdzie.
- Kto doda wysyłkę do {issue_invoice}, musi dopisać `mail: Mailer` do jego sygnatury. `sowa review` pokaże to jako nowe uprawnienie, na samej górze listy, a `docs.lock` zgłosi, że ta decyzja wymaga ponownego przejrzenia.
