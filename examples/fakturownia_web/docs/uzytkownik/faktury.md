# Faktury

Instrukcja dla użytkownika. Odnośniki w nawiasach klamrowych, np. {Nip}, sprawdza kompilator Sowy. Bloki kodu `sowa` uruchamiają się jako testy na zasobach z `[resources.test]`.

## Wystawianie faktury

Na stronie „Nowa faktura” ({post_invoice}) podaj:

- nazwę nabywcy,
- NIP nabywcy ({Nip}); możesz wpisać go z kreskami lub spacjami, np. `123-456-32-18`,
- adres e-mail nabywcy,
- co najmniej jedną pozycję: nazwę, ilość, cenę netto i stawkę VAT.

Po kliknięciu „Wystaw fakturę” faktura dostaje numer, np. `FV/2026/0001` ({InvoiceNumber}), i trafia na listę faktur. Numery idą po kolei w ramach roku i nie mają luk. Wystawionej faktury nie da się usunąć.

Jeśli dane są błędne, faktura nie zostanie wystawiona, a numer nie zostanie zużyty. Komunikat wskaże, co poprawić, np. „Pozycja 2: nieprawidłowa ilość.”

```sowa
form = "buyer_name=Firma&buyer_nip=123-456-32-18&buyer_email=biuro@firma.pl&lines[0].name=Usługa&lines[0].quantity=0&lines[0].unit_net=100&lines[0].vat=23"
response = handle(Request(method: Post, path: "/invoices", body: form), db, clock, ksef)
response is BadRequest
list_invoices(db) == []
```

## Stawki VAT

Każda pozycja ma jedną stawkę ({VatRate}): 23%, 8%, 5% albo 0%.

## Sumy

Na fakturze widzisz sumę netto, VAT i brutto ({totals}). VAT liczy się od sumy netto w każdej stawce, a nie od każdej pozycji osobno, więc może się różnić o grosz od sumy VAT-ów z pozycji.

## Wysyłka do KSeF

Wystawioną fakturę wysyłasz do KSeF przyciskiem „Wyślij do KSeF” na jej stronie ({post_ksef}). Po wysyłce strona pokazuje numer z KSeF i czas wysyłki ({InvoiceStatus}).

- Wysyłka to osobny krok. Jeśli KSeF nie odpowiada albo odrzuci fakturę, faktura nadal jest wystawiona i można wysłać ją ponownie.
- Wysłanej faktury nie wyślesz drugi raz. Ponowne kliknięcie wraca na jej stronę.

Cały przepływ, od formularza do numeru z KSeF:

```sowa
form = "buyer_name=Firma&buyer_nip=123-456-32-18&buyer_email=biuro@firma.pl&lines[0].name=Usługa&lines[0].quantity=1&lines[0].unit_net=100&lines[0].vat=23"
created = handle(Request(method: Post, path: "/invoices", body: form), db, clock, ksef)
created == Redirect(to: "/invoices/2026/0001")
sent = handle(Request(method: Post, path: "/invoices/2026/0001/ksef", body: ""), db, clock, ksef)
sent == Redirect(to: "/invoices/2026/0001")
invoice = try find_invoice("FV/2026/0001", db)
invoice.status == SentToKsef(reference: "KSEF-FV/2026/0001", sent_at: clock.now())
again = handle(Request(method: Post, path: "/invoices/2026/0001/ksef", body: ""), db, clock, ksef)
again == Redirect(to: "/invoices/2026/0001")
```
