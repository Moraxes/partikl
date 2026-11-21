---
id: 46
title: Schemat cząstki
summary: Minimalny silnikowy Particle; pola modelu shared/private w buforach reguł
status: done
priority: soon
branch: main
part_of: 35
blocked_by:
- 39
- 45
created: 2026-11-21T12:39:03
worked: 2026-11-22T00:17:54
edited: 2026-11-22T00:17:54
---
Cel (poziom C): Particle silnika to pos, last, species, id. Pola modelu (crowding i przyszłe) w buforach deklarowanych przez reguły: shared do Neighbour (kopiowane w scatterze), private tylko dla integrate i renderu. Rust zna rozmiar w bajtach, bufory alokowane generycznie; miejsce na planer pamięci z M3. Następny krok: crowding jako pierwsze pole private/shared.


Wynik: Particle silnika 24 B (pos, last, species, id). Reguły deklarują pola modelu `//@field nazwa: typ [shared]` (f32, u32, i32, vec2f) oraz w WESL Model, Neighbour, pack_neighbour/unpack_neighbour; RuleSchema liczy rozmiar (układ WGSL). Bufory model (ShaderBuffer w GpuSim, wspólny z Harness) i sorted_model alokowane generycznie, scatter i sort_cells przenoszą model z cząstką; shared ≤ 4 B (słowo w sorted_hot), więcej czeka na planer M3; zmiana pól w trakcie odrzucana (restart). Wiązania reguł w rules/bindings.wesl, uniforms w engine/uniforms.wesl (bez cykli importów). Harness: with_rules, set_model, run_with_model. Testy: model_stays_with_its_particle (też tryb deterministyczny), crowding w wyroczni. A/B vs 1731379 (2 rundy, --quick, n=100000): krok ×0,98–0,99, forces ×1,00, scatter ×1,11–1,15 (dodatkowy bufor), count ×0,78. Commit 7f04bed. Uwaga: tools/bench-ab/ab.sh pada na pustej tablicy bench_args (bash 3.2, set -u).
