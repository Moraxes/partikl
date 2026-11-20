---
id: 24
title: Uśrednianie sił gęstością
summary: 'Siła razy min(1, N0/N): ograniczona sztywność i przyciąganie w skupiskach'
status: todo
priority: soon
branch: main
blocked_by:
- 22
created: 2026-11-20T23:12:22
worked: 2026-11-20T23:39:27
links:
- kind: commit
  target: 5e5331f
  at: 2026-11-20T23:39:27
  note: 'feat: optional force averaging by neighbour count'
---
Cel: suma sił cząstki razy `min(1, N0/N)` (N: sąsiedzi w zasięgu, N0 rzędu kilkudziesięciu), żeby siła i sztywność nie rosły z gęstością. Następny krok: licznik sąsiadów w jądrze (oba tryby, w parach licznik dla obu stron przez bufory), dobór N0 sondą.

## Pomiar
- Sonda n=100000, `spec-2026-11-20-21-48-03.ron`: odsetek cząstek ponad 1000 j/s i najpełniejsza komórka w czasie (przed: 16% po 3,75 s, 44% po 15 s z próbkowaniem).
- `--quick` z opcją wyłączoną: koszt bez zmian.
