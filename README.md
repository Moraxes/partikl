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

## Raporty
`reports/` to artefakty generowane narzędziami z `main` i nadpisywane przy każdym
uruchomieniu (wyjątek od zasady niepoprawiania), nie edytować ręcznie:
- `perf-report.md`: etapy optymalizacji, `node tools/bench-report/report.js` z
  `.data/perf-stages.json` (manifest i przebiegi zostają w `.data/`, poza gitem).
