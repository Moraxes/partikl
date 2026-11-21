---
id: 36
title: Podział compute.rs
summary: pipelines.rs, buffers.rs, step.rs; tabela przebiegów z nazwami entry pointów
status: doing
priority: soon
branch: compute-shader-1
part_of: 35
created: 2026-11-21T12:39:02
worked: 2026-11-21T12:56:23
links:
- kind: commit
  target: 9fadb3f
  at: 2026-11-21T12:56:23
  note: 'refactor(gpu): split the compute step into pipelines, buffers and step'
---
Cel: prepare robi dziś cztery rzeczy (leniwe bufory z pustą pierwszą klatką, cache wariantów, liczenie kroków, bind group). Rozbić na pipelines.rs, buffers.rs (osobny system przy pojawieniu się GpuSim) i step.rs z jedną tabelą przebiegów nazwanych jak entry pointy WESL. Bez zmiany zachowania. Następny krok: wydzielić tworzenie buforów z prepare.
