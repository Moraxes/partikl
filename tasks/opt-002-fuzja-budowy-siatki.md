---
id: opt-002
title: Fuzja budowy siatki
summary: Fuzja budowy siatki w przebiegu sił (sonnet)
status: doing
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:30
worked: 2026-11-22T01:36:17
edited: 2026-11-22T01:36:31
links:
- kind: commit
  target: baabcd6
  at: 2026-11-22T01:36:17
  note: 'perf(gpu): fuse grid histogram into the forces pass'
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

## Wynik (commit baabcd6)

Zrobione: histogram następnego kroku liczy przebieg sił (`next_counts`, `cell_ranks`), `clear` przejmuje liczniki, `scatter` bez atomics, `add_offsets` bez kursorów. `GRID_PASSES` ma warunek `When::{Always, Deterministic, Recount}`; `count` (z rangami) działa tylko w pierwszym kroku po `buffers::create` i po podmianie stanu (zmiana id bufora albo `GpuSim::state_epoch`, który zwiększa `Harness::set_state`; zapis w miejscu nie zmienia id bufora). Przy rekonstrukcji `next_counts` czyszczone `clear_buffer`. Kolumna `gpu_count_ns` zostaje (0 poza krokiem po wgraniu stanu). Test nowy: `swapping_state_between_steps_keeps_every_particle`; `timed_step_reports_every_pass` dopuszcza 0 dla `count`. cargo test 67/67, clippy bez nowych ostrzeżeń (10/11 jak przed).

Uwaga o benchu: sceny uniform/clusters/dense robią `set_state` przed każdym pomiarem, a czasy GPU pochodzą z pierwszego kroku klatki, więc mierzą przypadek rekonstrukcji (count + dodatkowy atomic w siłach), nie stan ustalony. Stan ustalony pokazuje scena `evolved`.

Pomiary 100k (ab.sh, 3 rundy, M5, mediana po/przed):
- evolved (stan ustalony): GPU krok 432.8 -> 418.2 µs (x0.966); siły 378.0 -> 378.6; scatter 28.5 -> 28.6; count 16.5 -> 0; siatka (krok - siły) ok. 54.8 -> 39.6 µs (-28%); krok (z klatką) x0.975.
- uniform/clusters/dense (z set_state, 1 krok z 16 to rekonstrukcja): `krok` x0.965 / x0.973 / x0.962; GPU krok x1.021 / x1.013 / x1.009 (pierwszy krok klatki = rekonstrukcja: count 17 -> 22 µs, siły bez zmian).
- Oczekiwano: siatka -35...40%, krok -5...8%. Jest siatka -28%, krok GPU -3.4% (stan ustalony), czyli poniżej oczekiwań, bez regresji sił (extra atomic ukryty).

Duże n (jeden przebieg, bez --quick; scena evolved zmienia trajektorię, więc liczby kandydatów różne i nie do porównania w siłach):
- baseline vs po, siatka µs: 500k uniform 533 -> 613, clusters 394 -> 468, dense 375 -> 444; 1M uniform 1616 -> 1775, clusters 962 -> 1129, dense 879 -> 1030 (przypadek rekonstrukcji; GPU µs z pierwszego kroku klatki).
- evolved siatka µs: 500k 418 -> 314 (-25%), 1M 891 -> 733 (-18%) (kandydaci 5902 -> 6559 i 10803 -> 13551, siły rosną z nimi).
Przebiegi: .artifacts/bench (commit aab82de); baseline 01-35-24 (default), 01-35-34 (evolved), po 01-35-51, 01-36-01.

Wniosek: zysk w stanie ustalonym (siatka -18...-28%), pomiar scen z resetem go nie widzi. Brak regresji w stanie ustalonym; do decyzji: czy bench ma mierzyć krok ustalony (np. czasy z drugiego kroku klatki) zamiast rekonstrukcji.
