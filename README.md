# Dziennik pracy partikl

Gałąź `worklog` bez wspólnej historii z `main`, podpięta jako worktree w `.worklog/`
(`git worktree add .worklog worklog`). Wyniki eksperymentów, pomiary, sprawdzone i odrzucone
warianty, raporty. Historia zmian w kodzie zostaje w `main`.

- Jeden wpis na plik: `yyyy-mm-dd-hh-mm-<tytuł po myślnikach>.md`.
- Nagłówek: `commit:` rewizja `main`, na której zrobiono pomiar (z `-dirty`, jeśli na
  niezacommitowanych zmianach, i wtedy opis zmian w treści); opcjonalnie `data:` pliki wyników
  z `.data/bench-results/`.
- Wpisów się nie poprawia. Nowy wynik albo korekta to nowy wpis, który wskazuje stary.
- Commit na tej gałęzi: `docs: add <tytuł>`.
