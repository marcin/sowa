# Sowa w VS Code

Kolorowanie składni plików `.sowa` i bloków ```` ```sowa ```` w plikach `.md`. Zwijanie po wcięciach, komentarze `//` przez Cmd+/.

## Instalacja

```
cd editors/vscode
npx @vscode/vsce package --skip-license -o sowa.vsix
code --install-extension sowa.vsix
```

Potem przeładuj okno: Cmd+Shift+P, „Developer: Reload Window”. Po zmianie gramatyki trzeba podbić `version` w `package.json` i zainstalować ponownie.

## Co jest kolorowane

| Element | Przykład |
|---|---|
| słowa kluczowe | `fn`, `type`, `return`, `match`, `if`, `for`, `try`, `var` |
| słowa-operatory | `as`, `or`, `is`, `not`, `with` |
| dokumentacja | `desc` z tekstem (także blok z wcięciem), `doc` i `why` ze ścieżką jako odnośnikiem, `example`, `property` |
| odwołania w `desc` | `{Percent}` |
| uprawnienia | `Db`, `DbRead`, `Clock`, `Mailer`, `Http`, ... w parametrach |
| warunek w typie | `α` |
| typy, wywołania, pola | `Money`, `read_nip(...)`, `name: ...` |

## GitHub

GitHub koloruje kod przez Linguist, który dodaje język dopiero, gdy używa go co najmniej 200 repozytoriów. Do tego czasu `.gitattributes` w katalogu głównym każe traktować `.sowa` jak Rusta. Bloki ```` ```sowa ```` w `.md` na GitHubie zostają bez kolorów.
