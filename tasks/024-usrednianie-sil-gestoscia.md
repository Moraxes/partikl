---
id: 24
title: Uśrednianie sił gęstością
summary: 'Siła razy min(1, N0/N): ograniczona sztywność i przyciąganie w skupiskach'
status: doing
priority: soon
branch: main
blocked_by:
- 22
created: 2026-11-20T23:12:22
worked: 2026-11-20T23:39:27
edited: 2026-11-20T23:39:27
links:
- kind: commit
  target: 5e5331f
  at: 2026-11-20T23:39:27
  note: 'feat: optional force averaging by neighbour count'
- kind: commit
  target: 5e5331f
  at: 2026-11-20T23:39:27
---
Wynik: `--average-forces` / klawisz N, `AVERAGE_NEIGHBOURS = 64`. Chaos przy n=100000 znika przy każdym sprawdzonym N0; skupiska dalej gęstnieją (najpełniejsza komórka rośnie), ale bez wyrzutów.

## Sonda n=100000, `spec-2026-11-20-21-48-03.ron`, krok 1500
| wariant | prędkość med / p99 / max [j/s] | >1000 j/s | najpełniejsza komórka |
|---|---|---|---|
| bez uśredniania (po 027) | 682 / 4107 / 5747 | 38,4% | 2407 |
| N0 = 32 | 54 / 204 / 385 | 0% | 1660 |
| N0 = 64 | 74 / 183 / 595 | 0% | 3534 |
| N0 = 128 | 99 / 227 / 545 | 0% | 1876 |

N0 = 64: z ograniczenia sztywności (`N0 · REPULSION_STRENGTH`) stabilne do ok. 400; środek sprawdzonych wartości. Przy n=9000 macierz 23 ma ok. 12 sąsiadów w zasięgu, więc tam bez zmian; macierze gęstsze zmieniają dynamikę skupisk.

