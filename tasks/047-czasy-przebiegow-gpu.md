---
id: 47
title: Czasy przebiegów GPU
summary: 'Timestamp queries w benchu GPU: osobne czasy count, prefix_sum, scatter, forces'
status: done
priority: soon
branch: compute-shader-1
part_of: 50
blocked_by:
- 51
relates:
- 28
created: 2026-11-21T13:33:33
worked: 2026-11-21T15:25:00
edited: 2026-11-21T15:25:00
links:
- kind: commit
  target: 4afdc9c
  at: 2026-11-21T15:24:53
  note: 'feat(gpu): measure GPU time per pass with timestamp queries'
---
Cel: czysty czas GPU na krok i na przebieg (budowa siatki: clear/count, suma prefiksowa, scatter; siły), bez narzutu klatki Bevy. Na M5 (Metal) adapter ma TIMESTAMP_QUERY, ale nie TIMESTAMP_QUERY_INSIDE_PASSES: znaczniki tylko jako timestamp_writes na granicach przebiegów compute. W trybie benchu każdy dispatch we własnym przebiegu ze znacznikami (lekko zmienia pomiar), w aplikacji jeden znacznik na krok. Kolumny gpu_*_ns w Parquet obok step_ns; potem ewentualnie HUD. Bevy RenderDiagnosticsPlugin odpada (na Metalu tylko czas CPU). Wynik (4afdc9c): src/gpu/compute/timing.rs; znaczniki tylko w pierwszym kroku klatki (każdy przebieg ze znacznikami to ok. 20 µs narzutu na M5; z pomiarem wszystkich 16 kroków step_ns rósł 115 → 287 µs przy 9000), reszta kroków jednym przebiegiem: step_ns bez zmian. Harness::set_gpu_timing / take_gpu_times; odczyt poza czasem kroku. Na M5 pierwsza klatka z pomiarem ma zerowy znacznik ostatniego przebiegu (przyczyna nieustalona), taka próbka przepada. Quick 100k (krok / GPU / siły, µs): uniform 752 / 667 / 602, clusters 1040 / 969 / 903, dense 1109 / 1036 / 974; budowa siatki ok. 64 µs (scatter 33, count 22, suma pref. 8). Siły to ok. 90% czasu GPU, narzut klatki ok. 85 µs na krok. GPU krok powtarzalny do 0,6% między przebiegami.
