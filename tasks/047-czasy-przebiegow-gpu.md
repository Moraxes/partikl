---
id: 47
title: Czasy przebiegów GPU
summary: 'Timestamp queries w benchu GPU: osobne czasy count, prefix_sum, scatter, forces'
status: doing
priority: soon
branch: compute-shader-1
part_of: 50
blocked_by:
- 51
relates:
- 28
created: 2026-11-21T13:33:33
worked: 2026-11-21T15:24:53
edited: 2026-11-21T15:00:57
links:
- kind: commit
  target: 4afdc9c
  at: 2026-11-21T15:24:53
  note: 'feat(gpu): measure GPU time per pass with timestamp queries'
---
Cel: czysty czas GPU na krok i na przebieg (budowa siatki: clear/count, suma prefiksowa, scatter; siły), bez narzutu klatki Bevy. Na M5 (Metal) adapter ma TIMESTAMP_QUERY, ale nie TIMESTAMP_QUERY_INSIDE_PASSES: znaczniki tylko jako timestamp_writes na granicach przebiegów compute. W trybie benchu każdy dispatch we własnym przebiegu ze znacznikami (lekko zmienia pomiar), w aplikacji jeden znacznik na krok. Kolumny gpu_*_ns w Parquet obok step_ns; potem ewentualnie HUD. Bevy RenderDiagnosticsPlugin odpada (na Metalu tylko czas CPU). Następny krok: QuerySet + resolve + readback w step::run, włączane zasobem tylko w harnessie.
