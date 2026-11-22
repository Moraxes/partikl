---
id: opt-002
title: Fuzja budowy siatki
summary: Fuzja budowy siatki w przebiegu sił (sonnet)
status: todo
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:30
---
Model: sonnet (zmiana układu buforów i przebiegów compute, ryzyko hazardów pamięci).

Cel: usunąć z budowy siatki osobny histogram i drugi atomic. Przebieg sił zna nową pozycję cząstki, więc od razu liczy histogram następnego kroku i zapamiętuje rangę w komórce; scatter nie potrzebuje atomics. Oczekiwany efekt: budowa siatki −35…40%, krok −5…8%.

Zakres:
- step_driver.wesl `forces`: po `wrap(integrate_particle(...))` wylicz `c = cell_index(new.pos)`, `rank = atomicAdd(&next_counts[c], 1u)`, zapisz `cell_ranks[j] = rank`.
- Bufor `cell_cursors` zmienia rolę na `next_counts`; nowy bufor `cell_ranks` (n×4 B). Pliki: compute/buffers.rs, compute/bindings.rs (`Binding::CellCursors` → `NextCounts`, plus `CellRanks`), engine/bindings.wesl.
- grid_build.wesl: `clear` staje się przejęciem liczników (`cell_counts[c] = atomicLoad(next_counts[c])`, potem `next_counts[c] = 0`); `scatter`: `dst = cell_offsets[cell_index(p.pos)] + cell_ranks[i]`, bez atomicAdd; dotychczasowy `count` zostaje tylko na pierwszy krok po wgraniu stanu.
- prefix_sum.wesl `add_offsets`: znika `atomicStore` kursorów.
- pipelines.rs `GRID_PASSES`: warunek wykonania przebiegu zamiast samego `deterministic_only` (np. `when: Always | Deterministic | Recount`). `step::plan` ustawia „recount" w pierwszym kroku po `buffers::create` i po każdej podmianie bufora stanu (zmiana `GpuSim`). Ten mechanizm warunkowych przebiegów jest potem używany w opt sub-stepping (rescale).
- Tryb deterministyczny bez zmian (`sort_cells` sortuje po id).
- timing.rs: `gpu_count_ns` będzie zerem albo zniknie; kolumnę zostawić, żeby nie psuć raportów.

Weryfikacja: testy GPU (wyrocznia f64, niezmienniki: zachowane id i liczba cząstek po scatterze, w tym przy podmianie stanu), bench gpu --quick uniform/clusters/dense i --sizes 500000,1000000; osobno czasy przebiegów (gpu_*_ns).
