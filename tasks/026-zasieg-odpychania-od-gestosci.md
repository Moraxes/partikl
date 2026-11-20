---
id: 26
title: Zasięg odpychania od gęstości
summary: 'Trzeci tryb nakładania: zasięg odpychania pary rośnie z wygładzoną gęstością, sztywność stała'
status: done
priority: soon
branch: main
blocked_by:
- 22
relates:
- 23
- 25
created: 2026-11-20T23:20:35
worked: 2026-11-20T23:47:19
edited: 2026-11-20T23:47:19
links:
- kind: commit
  target: e3011d7
  at: 2026-11-20T23:47:19
  note: 'feat: density-dependent repulsion range as an overlap mode'
- kind: commit
  target: e3011d7
  at: 2026-11-20T23:47:19
---
Wynik: `--overlap adaptive-range` / klawisz O (trzeci tryb). Zasięg pary `max(r_i, r_j)`, `r = 10·√(N̄/64)` w [10, 20], `N̄` w komponencie `Crowding` (wymagany przez `Acceleration`), α = 0,02. Gwiazdy same się rozpychają: bez uśredniania chaos prawie znika, skupiska najluźniejsze ze wszystkich trybów.

## Sonda n=100000, `spec-2026-11-20-21-48-03.ron`, krok 1500
| wariant | prędkość med / p99 / max [j/s] | >1000 j/s | najpełniejsza komórka |
|---|---|---|---|
| sprężyna | 682 / 4107 / 5747 | 38,4% | 2407 |
| zmienny zasięg | 348 / 1168 / 2155 | 2,7% | 247 |
| zmienny zasięg + uśrednianie | 125 / 253 / 450 | 0% | 1082 |

Bez zmian α, N0 i r_max (wartości z planu). Zapis do ECS przez wspólne mapowanie kolejność ECS → siatka (`slots`).

