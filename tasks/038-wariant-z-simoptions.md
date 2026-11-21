---
id: 38
title: Wariant z SimOptions
summary: Pipeline'y kluczowane przez SimOptions; GpuParams zostaje licznikiem kroków (StepClock)
status: doing
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 36
created: 2026-11-21T12:39:02
worked: 2026-11-21T15:44:05
links:
- kind: commit
  target: eead1cd
  at: 2026-11-21T15:44:05
  note: 'refactor(gpu): key force pipelines by SimOptions'
---
Cel: zamiast (u8, bool), GpuParams.overlap i sync_options kluczować warianty sił bezpośrednio przez SimOptions (Hash, ExtractResource). Z GpuParams zostaje licznik kroków. Następny krok: forces_defs(&SimOptions).
