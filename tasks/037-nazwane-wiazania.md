---
id: 37
title: Nazwane wiązania
summary: Jedna struktura wiązań zamiast trzech list porządkowych (layout, bind group, @binding)
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
  target: 30047c1
  at: 2026-11-21T15:42:17
  note: 'refactor(gpu): one Binding enum for the bind group layout, group and WESL'
---
Cel: kolejność w BindGroupLayoutEntries::sequential, BindGroupEntries::sequential i @binding(n) w bindings.wesl musi dziś zgadzać się na oko. Jedna struktura z polami w kolejności wiązań i numerem w komentarzu. Wynik (30047c1): enum Binding w src/gpu/compute/bindings.rs (numer = wariant, layout i bind group z Binding::ALL, exhaustive match przy tworzeniu grupy); test porównuje numery, address space i nazwy z bindings.wesl (sprawdzone: zmiana nazwy zmiennej oblewa test). Bez zmiany zachowania i czasów (GPU 667 / 965 / 1031 µs).
