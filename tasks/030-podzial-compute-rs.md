---
id: 30
title: podzial-compute-rs
summary: compute.rs na pipelines.rs, buffers.rs, step.rs; tabela przebiegów z nazwami entry pointów
status: todo
priority: soon
branch: compute-shader-1
part_of: 29
created: 2026-11-21T12:38:32
---
Cel: prepare robi dziś cztery rzeczy (leniwe bufory z pustą pierwszą klatką, cache wariantów, liczenie kroków, bind group). Rozbić na pipelines.rs, buffers.rs (osobny system przy pojawieniu się GpuSim) i step.rs z jedną tabelą przebiegów nazwanych jak entry pointy WESL. Bez zmiany zachowania. Następny krok: wydzielić tworzenie buforów z prepare.
