---
id: 22
title: Przełączniki opcji symulacji
summary: 'SimOptions z flag CLI, klawisze w trakcie (nakładanie: 3 tryby cyklicznie); warianty jako osobne systemy z run_if'
status: todo
priority: soon
branch: main
created: 2026-11-20T23:12:14
edited: 2026-11-20T23:20:42
---
Cel: opcjonalne zachowania symulacji w zasobie `SimOptions` (boolean dla uśredniania sił 024, enum `Overlap { Spring, Resolve, AdaptiveRange }` dla nakładania 025/026, jeden klawisz przełącza cyklicznie), ustawiane flagami CLI i przełączane klawiszami w trakcie. Następny krok: zasób, flagi, klawisze w `ui`, schemat z run_if.

## Kształt
- Osobny system per wariant z `run_if` na `SimOptions` (np. `compute_forces::<Averaged>` i `compute_forces::<Plain>`, `resolve_overlaps` tylko przy włączonej opcji). Wariant widać w schemacie i w profilu.
- Stan, który musi przetrwać przełączenie (`ForceData`: bufory, `last_work`), w zasobie zamiast `Local`, żeby warianty go współdzieliły.
- Bench: domyślnie wszystko wyłączone; nazwy systemów w `SYSTEM_NAMES` bez zmian dla wariantu domyślnego.
- Testy: każdy wariant osobno; przełączenie w trakcie nie gubi cząstek ani stanu siatki.
