# 002. Ponowna wysyłka do KSeF jest bezpieczna

Dotyczy: {send_to_ksef}, {ksef_fake}

## Decyzja

Aplikacja zakłada, że ksef.pl dla tego samego numeru faktury zwraca zawsze ten sam numer z KSeF i nie tworzy drugiej faktury. Dlatego po błędzie sieci wystarczy wysłać fakturę jeszcze raz.

## Dlaczego

- Przy przekroczeniu czasu nie wiadomo, czy ksef.pl przyjął fakturę. Bez tego założenia trzeba by przed ponowną wysyłką pytać ksef.pl o status, a to osobne API i osobny stan.
- Dwa kliknięcia naraz mogą wysłać tę samą fakturę dwa razy. Przy tym założeniu obie wysyłki dostaną ten sam numer z KSeF.

## Jak tego pilnować

- Założenie jest zapisane w atrapie {ksef_fake}: ta sama faktura daje zawsze ten sam numer `KSEF-<numer faktury>`. Atrapa to czysta funkcja, więc inaczej się nie da.
- Kod atrapy człowiek czyta (`impl/ksef_fake.sowa` jest w CODEOWNERS). Jeśli prawdziwy ksef.pl działa inaczej, najpierw zmienia się atrapę i tę decyzję, a dopiero potem kod.

## Konsekwencje

- Atrapa jako czysta funkcja nie pamięta poprzednich zapytań, więc nie opisze serwera, który za drugim razem odpowiada inaczej. Taki serwer wymagałby atrapy ze stanem, której Sowa jeszcze nie ma.
