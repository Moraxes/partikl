---
id: 61
title: Szablon liczników GPU
summary: Szablon Instruments z zestawem liczników wydajności, żeby xctrace zbierał ALU/pamięć/occupancy
status: todo
priority: later
branch: compute-shader-1
part_of: 54
created: 2026-11-21T18:09:50
---
Cel: liczniki wydajności GPU (limitery ALU i pamięci, occupancy) nie zbierają się z CLI (patrz 058). Użytkownik zapisuje z GUI Instruments szablon (Metal System Trace + Metal GPU Counters z wybranym zestawem liczników, np. Performance Limiters), a xctrace record --template ścieżka.tracetemplate nagrywa headless bench; eksport tabeli gpu-counter-value (parser z id/ref w scratchpadzie sesji, do przeniesienia do tools/). Następny krok: użytkownik zapisuje szablon; potem skrypt tools/profile-gpu.sh.
