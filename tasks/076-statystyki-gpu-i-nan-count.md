---
id: 76
title: Statystyki GPU i nan_count
status: done
priority: soon
branch: main
part_of: 28
created: 2026-11-22T17:42:58
worked: 2026-11-22T17:54:02
links:
- kind: commit
  target: b688741
  at: 2026-11-22T17:54:02
  note: 'feat(gpu): optional state stats pass with nan_count'
---
Cel: opcjonalny przebieg reduce_stats po siłach (M1c planu M3): nan_count (nie-skończone pos/last), max prędkość, energia kinetyczna; odczyt asynchroniczny jak timestamp queries; domyślnie wyłączony (zero kosztu). Test: wstrzyknięty NaN daje nan_count 1. Interfejs (panel, pauza) na razie poza zakresem. Następny krok: pass + bufor + readback + test w harnessie.
