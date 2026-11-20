---
id: 20
title: Rzadsze kroki symulacji
summary: 'Większy dt albo mniej kroków na sekundę (100 Hz → 50 Hz): o połowę mniej pracy; stabilność do sprawdzenia'
status: todo
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
---
Cel: mniej pracy na sekundę, skala czasu nie ma znaczenia dla wygaszacza. Następny krok: `DELTA_TIME` 0,01 → 0,02 (krok fizyki co 20 ms) i obejrzeć, czy ruch i skupiska wyglądają podobnie; alternatywnie zostawić dt i zwolnić symulację (mniej kroków na sekundę).

## Ryzyka
- Całkowanie Verleta z odpychaniem o sztywności 100 (albo większej po ograniczeniu gęstości) może się rozbiegać przy większym dt.
- Wpływa na wszystkie macierze inaczej; benchmark (czas kroku) tego nie pokaże, liczy się czas na sekundę animacji.
