---
id: 37
title: Nazwane wiązania
summary: Jedna struktura wiązań zamiast trzech list porządkowych (layout, bind group, @binding)
status: doing
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 36
created: 2026-11-21T12:39:02
worked: 2026-11-21T15:42:17
links:
- kind: commit
  target: 30047c1
  at: 2026-11-21T15:42:17
  note: 'refactor(gpu): one Binding enum for the bind group layout, group and WESL'
---
Cel: kolejność w BindGroupLayoutEntries::sequential, BindGroupEntries::sequential i @binding(n) w bindings.wesl musi dziś zgadzać się na oko. Jedna struktura z polami w kolejności wiązań i numerem w komentarzu. Następny krok: po podziale compute.rs.
