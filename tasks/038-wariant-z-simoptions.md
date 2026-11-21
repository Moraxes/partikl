---
id: 38
title: Wariant z SimOptions
summary: Pipeline'y kluczowane przez SimOptions; GpuParams zostaje licznikiem kroków (StepClock)
status: done
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 36
created: 2026-11-21T12:39:02
worked: 2026-11-21T15:44:05
edited: 2026-11-21T15:44:05
links:
- kind: commit
  target: eead1cd
  at: 2026-11-21T15:44:05
  note: 'refactor(gpu): key force pipelines by SimOptions'
---
Cel: zamiast (u8, bool), GpuParams.overlap i sync_options kluczować warianty sił bezpośrednio przez SimOptions (Hash, ExtractResource). Z GpuParams zostaje licznik kroków. Wynik: SimOptions (Hash) wyciągane do świata renderowania wprost i kluczem cache'u wariantów; GpuParams zostało przy deterministic i step_count (nie nazwane StepClock, bo tryb deterministyczny to nie zegar). Usunięte sync_options, Variant, stałe SPRING/RESOLVE/ADAPTIVE_RANGE i repr(u8). Nowy test: zmiana opcji w trakcie działa od następnego kroku.
