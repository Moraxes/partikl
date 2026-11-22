---
id: opt-006
title: Sub-stepping silnik
summary: 'Sub-stepping: silnik i dt/K (sonnet)'
status: todo
priority: soon
branch: main
part_of: opt-001
blocked_by:
- opt-002
created: 2026-11-22T01:12:30
---
Model: sonnet (plan kroków, dt w RuleParams, przeskalowanie Verleta, warunkowy przebieg; subtelne zależności).

Cel: K podkroków po dt/K na takt (sztywność, stabilność trybu RESOLVE). Dziś kilka kroków na klatkę już jest (StepBudget), ale zawsze z pełnym dt. Po opt fuzja siatki (mechanizm warunkowych przebiegów).

Zakres:
- Zasób `Substeps(u32)` (domyślnie 1) z ExtractResource do świata renderowania (wzór: GpuParams w src/gpu/mod.rs); sterowanie UI to osobne zadanie (haiku).
- compute/step.rs `plan`: liczba kroków GPU = takty × K. `step_index` rośnie co podkrok (dekorelacja próbkowania). `MAX_STEPS_PER_FRAME` i `StepBudget` liczą takty (prędkość symulacji w HUD zostaje poprawna). `GpuProgress` dalej liczy kroki GPU (testy czekają na kroki); zdecyduj i opisz w zadaniu, jak `GpuParams::step_count` mapuje się na takty.
- `RuleParams.dt = DELTA_TIME / K`, zapis przez `buffers::upload_spec` przy zmianie K (Substeps jako dodatkowe wyzwalacze, nie tylko `ParticleSpec`).
- Verlet trzyma prędkość jako `pos − last`, więc zmiana dt wymaga przeskalowania: jednorazowy przebieg silnika `rescale` (grid_build.wesl): `last = pos − (pos − last)·dt_new/dt_old`, uruchamiany w pierwszym kroku po zmianie K (mechanizm `when` z opt fuzja siatki).
- Tarcie jest niezależne od dt (`d/(1 + c·|d|)`). Parametry „na krok" (`crowding_smoothing`, połowa poprawki w RESOLVE) działają na podkrok: odnotować w zadaniu, bez kompensacji.

Testy: przy K=2 brak NaN, zachowane id i liczba cząstek, średnia prędkość w granicach szumu względem K=1 (progi bezpieczeństwa, bez asercji na wartości analogowe). Koszt: K razy więcej kroków na takt, więc zmierzyć, kiedy StepBudget zaczyna zwalniać symulację.
