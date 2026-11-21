---
id: 47
title: Czasy przebiegów GPU
summary: 'Timestamp queries w benchu GPU: osobne czasy count, prefix_sum, scatter, forces'
status: todo
priority: soon
branch: compute-shader-1
relates:
- 28
created: 2026-11-21T13:33:33
---
Cel: bench GPU mierzy dziś cały krok; plan (sekcja 2.5) chce osobnych czasów przebiegów z timestamp queries wgpu, zapisanych do Parquet obok step_ns. Potrzebne do optymalizacji w M2 (wiadomo, który przebieg dominuje). Następny krok: sprawdzić, czy adapter ma TIMESTAMP_QUERY_INSIDE_PASSES i jak bevy 0.20 udostępnia QuerySet w RenderGraph.
