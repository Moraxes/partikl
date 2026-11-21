---
id: 39
title: Neighbour w WESL
summary: Jawny struct Neighbour z pack/unpack zamiast vec4 z bitcast i sztucznego Particle
status: doing
priority: soon
branch: compute-shader-1
part_of: 35
created: 2026-11-21T12:39:03
worked: 2026-11-21T12:56:28
---
Cel: sąsiad to dziś vec4 z bitcast i Particle(hot.xy, hot.xy, …, 0u, 0u); pakowanie skopiowane w scatter i sort_cells. Wprowadzić struct Neighbour { pos, species, crowding } z jedną parą pack/unpack; pair_interact dostaje Neighbour. To krok 3.1 planu (NeighborHot). Następny krok: pack_neighbour w grid_build.wesl.
