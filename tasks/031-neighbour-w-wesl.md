---
id: 31
title: neighbour-w-wesl
summary: Jawny struct Neighbour z pack/unpack zamiast vec4 z bitcast i sztucznego Particle
status: todo
priority: soon
branch: compute-shader-1
part_of: 29
created: 2026-11-21T12:38:33
---
Cel: sąsiad to dziś vec4 z bitcast i Particle(hot.xy, hot.xy, …, 0u, 0u); pakowanie skopiowane w scatter i sort_cells. Wprowadzić struct Neighbour { pos, species, crowding } z jedną parą pack/unpack; pair_interact dostaje Neighbour. To krok 3.1 planu (NeighborHot). Następny krok: pack_neighbour w grid_build.wesl.
