# 003. Wysyłka e-mailem jest osobnym krokiem

Dotyczy: {issue_invoice}, {send_invoice}

## Decyzja

{issue_invoice} nie wysyła e-maila. Wysyłka to osobna funkcja {send_invoice}, wywoływana po zapisaniu faktury.

## Dlaczego

- Wystawienie faktury to zapis w bazie (`effects Db.read, Db.write, Clock`). Wysyłka to sieć (`effects Net(mail)`: tylko serwer pocztowy z `sowa.toml`). Gdyby były razem, awaria serwera pocztowego cofałaby wystawienie faktury albo zostawiała ją w niejasnym stanie.
- Sygnatura {issue_invoice} bez `Net` daje recenzentowi gwarancję: wystawienie faktury niczego nie wysyła na zewnątrz. Kompilator pilnuje, żeby tak zostało.

## Konsekwencje

- Trzeba osobno obsłużyć ponowną wysyłkę, gdy e-mail nie dojdzie.
- Kto doda wysyłkę do {issue_invoice}, musi dopisać `Net` do jego sygnatury. Taka zmiana od razu rzuca się w oczy przy recenzji, a `docs.lock` zgłosi, że ta decyzja wymaga ponownego przejrzenia.
