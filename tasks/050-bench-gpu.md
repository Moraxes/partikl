---
id: 50
title: Bench GPU
summary: 'Pomiary GPU: czysty czas GPU per przebieg, kontrolowane stany, odchudzony harness'
status: todo
priority: soon
branch: compute-shader-1
created: 2026-11-21T15:00:57
---
Cel: bench, który mierzy pracę GPU (nie narzut klatki Bevy) w stanach rzadkich i gęstych, z A/B na tych samych stanach. Dziś step_ns przy małym n to głównie CPU (9000: ~1,5 ms na klatkę), jedna rzadka macierz po 512 krokach, brak rozbicia na przebiegi, A/B na różnych trajektoriach. Bevy RenderDiagnosticsPlugin na Metalu daje tylko czas CPU; adapter M5 ma TIMESTAMP_QUERY, bez INSIDE_PASSES (znaczniki tylko na granicach przebiegów). Do analizy przy M2 ręcznie: Xcode Instruments (Metal System Trace) i przechwycenie klatki. Następny krok: podzadania (tq tree).
