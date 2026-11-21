---
id: 39
title: Neighbour w WESL
summary: Jawny struct Neighbour z pack/unpack zamiast vec4 z bitcast i sztucznego Particle
status: done
priority: soon
branch: compute-shader-1
part_of: 35
created: 2026-11-21T12:39:03
worked: 2026-11-21T12:59:36
edited: 2026-11-21T12:59:36
links:
- kind: commit
  target: a41335f
  at: 2026-11-21T12:59:36
  note: 'refactor(gpu): give rules an explicit Neighbour instead of a fake Particle'
---
Cel: sąsiad to dziś vec4 z bitcast i Particle(hot.xy, hot.xy, …, 0u, 0u); pakowanie skopiowane w scatter i sort_cells. Wprowadzić struct Neighbour { pos, species, crowding } z jedną parą pack/unpack; pair_interact dostaje Neighbour. To krok 3.1 planu (NeighborHot). Wynik: struct Neighbour w types.wesl, układ vec4 schowany w store_neighbour/load_neighbour (bindings.wesl), pair_interact dostaje Neighbour. Bench 9000: 130,7 µs, 100k: 882 µs.
