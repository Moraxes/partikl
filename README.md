# Tracker partikl

Zadania projektu dla człowieka i agentów: `tasks/NNN-tytuł.md`, obsługiwane narzędziem `tq`
(projekt trqr w `~/repos/trqr`, opis formatu i poleceń w jego README). Gałąź `trqr`, worktree `.trqr/`,
lokalna. Artefakty (wyniki benchmarków, raporty) są na gałęzi `artifacts` w `.artifacts/` i
linkowane z zadań (`tq link`, `tq artifacts`).

## Zasady
- Tytuł krótki (2–5 słów), bo numer i tytuł mogą być nazwą gałęzi; szczegóły w `summary`.
- Treść od najważniejszego: pierwszy akapit to cel i „Następny krok” (albo wynik, gdy
  `done`), potem sekcje `##`. Wyniki eksperymentów idą do zadania, duże dane i raporty do
  `.artifacts/` z linkiem.
- Kolejność wynika z `blocked_by` i `priority` (`tq ready`, `tq next`), najwyżej 2 zadania
  w toku.
- Commity w `main` ze stopką `Task: 042` linkują się same (hook `post-commit`).
