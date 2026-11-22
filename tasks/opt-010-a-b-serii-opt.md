---
id: opt-010
title: A/B serii opt
summary: A/B całej serii opt względem e99ea18 (haiku)
status: done
priority: later
branch: main
part_of: opt-001
blocked_by:
- opt-002
- opt-003
- opt-005
- opt-006
- opt-007
- opt-008
created: 2026-11-22T01:12:31
worked: 2026-11-22T13:08:42
links:
- kind: commit
  target: 5a19c34
  at: 2026-11-22T13:05:09
  note: 'chore(bench): e99ea18 with 074 steady-state warm-up (A/B baseline only)'
- kind: commit
  target: a37cb88
  at: 2026-11-22T13:08:28
  note: 'data(bench): opt-010 A/B series (quick x5 vs e99ea18+074, 500k/1M)'
- kind: data
  target: a37cb88
  at: 2026-11-22T13:08:35
  note: 'A/B serii opt: quick x5 vs e99ea18+074 (5a19c34), HEAD 8e32d8b; 500k/1M gpu i render'
---
Model: haiku (uruchomienie skryptów i zapis liczb; bez zmian w kodzie).

Cel: podsumowanie serii opt: naprzemienne A/B względem stanu sprzed serii i zapis wyników.

Kroki:
1. Baza: commit e99ea18 (przed serią; to ostatni commit z naprawą 049). Uruchom `AB_BASE=e99ea18 tools/bench-ab/ab.sh` (zob. tools/bench-ab). Domyślnie bench gpu --quick.
2. Dodatkowo duże n: cargo bench --bench gpu -- --sizes 500000,1000000 oraz cargo bench --bench render -- --modes loop --sizes 500000,1000000 na bazie i na HEAD (bazę przez git worktree w katalogu poza repo; nie zmieniać bieżącego worktree).
3. Wyniki per liczba cząstek i scena (uniform/clusters/dense) wpisać do tego zadania w tabeli (baza / HEAD / zmiana %), z podziałem na przebiegi (gpu_*_ns). Przebiegi do .artifacts/bench, commit w .artifacts, tq link.
4. Jeśli któryś pomiar jest gorszy niż baza o więcej niż 5%: nie naprawiać, zgłosić z danymi użytkownikowi.
5. Zaktualizuj w 054 jedną linią, że seria opt została domknięta (tq edit nie jest potrzebne: dopisz plik zadania lub użyj tq worked 054).
