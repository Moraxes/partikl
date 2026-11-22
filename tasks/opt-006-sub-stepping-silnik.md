---
id: opt-006
title: Sub-stepping silnik
summary: 'Sub-stepping: silnik i dt/K (sonnet)'
status: doing
priority: soon
branch: main
part_of: opt-001
blocked_by:
- opt-002
created: 2026-11-22T01:12:30
worked: 2026-11-22T01:58:23
edited: 2026-11-22T01:58:50
links:
- kind: commit
  target: 6a4abbb
  at: 2026-11-22T01:58:23
  note: 'feat(gpu): sub-stepping engine with dt/K and Verlet rescale'
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-54-55.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-55-00.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-55-07.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-07.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-10.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-15.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-38.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-44.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-50.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-57-55.parquet
  at: 2026-11-22T01:58:50
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-58-01.parquet
  at: 2026-11-22T01:58:51
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-58-06.parquet
  at: 2026-11-22T01:58:51
- kind: data
  target: .artifacts/bench/render-2026-11-22-01-55-35.parquet
  at: 2026-11-22T01:58:51
- kind: data
  target: .artifacts/bench/render-2026-11-22-01-56-05.parquet
  at: 2026-11-22T01:58:51
- kind: data
  target: .artifacts/bench/render-2026-11-22-01-56-34.parquet
  at: 2026-11-22T01:58:51
- kind: commit
  target: 6a4abbb
  at: 2026-11-22T01:58:51
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

## Wynik

Zrobione w 6a4abbb. `Substeps(u32)` (domyślnie 1, `MAX_SUBSTEPS` = 8, `Substeps::get()` obcina do 1..=8) jest `ExtractResource` do świata renderowania, jak `SamplingCap`. Sterowania UI nie ma (opt-007); do benchów i testów: `Harness::set_substeps`, `--substeps K` w benchach gpu i render.

Mapowanie liczników (decyzja):
- `GpuParams::step_count` liczy takty stałego kroku (`DELTA_TIME`); `count_step`, `StepBudget` (limit, taken/wanted, `SIM_SPEED` w HUD) i `MAX_STEPS_PER_FRAME` zostają w taktach bez zmian, więc prędkość symulacji w HUD jest poprawna.
- GPU wykonuje `takty × K` podkroków. `GpuProgress` liczy podkroki GPU (`Harness::step(n)` czeka na n·K); `step_index` rośnie co podkrok (`Plan::gpu_steps`), więc próbkowanie dekoryluje się w każdym podkroku.
- `RuleParams.dt` na GPU = `spec.params.dt / K` (`buffers::gpu_params`, zapis w `create` i w `upload_spec`, które teraz wyzwala też zmiana `Substeps`, nie tylko `ParticleSpec`). `ParticleSpec` zostaje nietknięty.
- Przeskalowanie Verleta: przebieg `rescale` (`grid_build.wesl`, `When::Rescale`, pierwszy w `GRID_PASSES`) liczy `last = pos − (pos − last)·dt_ratio`. `dt_ratio` (nowe pole `EngineUniforms`, układ nadal 56 B) jest ustawiane tylko w pierwszym kroku planu; `Plan` kumuluje `K_stare/K_nowe`, gdy K zmieni się kilka razy, zanim krok się wykona (pauza, kompilacja pipeline'u). Stan startowy zakłada dt = `DELTA_TIME` (K = 1); podmiana stanu (`set_state`) nie skaluje, bo stan z zewnątrz ma prędkości dla `DELTA_TIME`... ale przy K > 1 jego prędkości wyjdą K razy za duże: wywołujący musi je przeskalować (nie dotyczy UI, bo stan startuje przy K = 1).
- `When::runs` ma trzeci argument (`rescale`); nowy wpis w `timing_entries()`, więc benchowy parquet ma kolumnę `gpu_rescale_ns` (0 poza krokiem po zmianie K) i kolumnę `substeps`.
- Parametry „na krok" działają na podkrok, bez kompensacji: `crowding_smoothing` (wygładzanie rośnie K razy szybciej w czasie), połowa poprawki w RESOLVE (K razy częściej), tarcie `d/(1 + c·|d|)` jest nieliniowe w przemieszczeniu na podkrok, więc przy tej samej prędkości daje słabsze tłumienie na jednostkę czasu niż przy K = 1 (dla małych `c·|d|` praktycznie bez różnicy).

Testy (src/gpu/tests/engine.rs): K = 2 po 40 taktach: brak NaN, id 0..n, liczba cząstek zachowana, stosunek średnich prędkości K=2/K=1 w (0,5; 2); zmiana K = 4 w locie i powrót do 1 nie zmieniają średniej prędkości poza (0,5; 2) (bez `rescale` wyszłoby ok. 4×); `GpuProgress` = takty × K. `cargo test`: 73 przechodzą. `cargo clippy --all-targets`: bez nowych ostrzeżeń (9 lib, 10 lib test, tyle samo co przed zmianą).

## Pomiar (M5, headless)

A/B K = 1 (ab.sh 3 rundy, --quick 100k, `--seed 1`, przed 2a8e0a5, po drzewo robocze): krok GPU uniform ×1.002, clusters ×1.002, dense ×1.000; siły ×1.001 / ×1.003 / ×1.000; czas taktu (wall) ×1.004 / ×1.009 / ×1.006. Brak regresji przy K = 1 (przebieg `rescale` pomijany, nie zmienia kroku).

Koszt K = 2 / 4 (bench gpu, czas taktu `krok µs`, 100k --quick; uwaga: sceny mierzą krok recount, zadanie 074):
- uniform: 485 / 895 / 1716 µs (K=1/2/4; ×1,85 / ×3,5)
- clusters: 641 / 1214 / 2361 µs (×1,9 / ×3,7)
- dense: 673 / 1280 / 2512 µs (×1,9 / ×3,7)
Koszt GPU pojedynczego kroku jest od K niezależny (uniform 461 µs, clusters 600, dense 632); takt kosztuje K × krok, bez narzutu.

Duże n (bench gpu, uniform, takt µs, K = 1 / 2 / 4): 500k: 3047 / 6329 / 12781; 1M: 6100 / 12755 / 25176.

Kiedy StepBudget zwalnia symulację: takt stałego kroku trwa 10 ms, więc symulacja traci tempo, gdy `K × (czas kroku GPU + render)` przekracza 10 ms. Sam krok GPU kosztuje ok. 6 µs na 1000 cząstek (uniform), czyli bez rysowania granica to ok. 1,6M / K cząstek. Z renderem (bench render, tryb loop, uniform, prędkość symulacji z `StepBudget`):

| n | K=1 | K=2 | K=4 |
|---|---|---|---|
| 100k | 100% | 100% | 100% |
| 250k | 100% | 100% | 100% |
| 500k | 100% | 100% | 67% |
| 1M | 95% | 54% | 31% |

Czyli przy K = 1 zwalnianie zaczyna się dopiero przy ok. 1M, przy K = 2 między 500k a 1M, przy K = 4 między 250k a 500k (z grubsza n_max ≈ 1M / K). Regulator (limit kroków) zachowuje płynność obrazu (klatka 18,7 ms przy 1M K=2, 31,7 ms przy 1M K=4 w tym nieprotokowanym harnessie). Ocena dla opt-007: przy domyślnej skali (do ok. 250k) K = 4 jest bezpłatne w sensie tempa symulacji; przy 1M zalecane K = 1.

Dane: `.artifacts/bench/gpu-2026-11-22-01-54-55/55-00/55-07.parquet` (K=1/2/4 quick), `gpu-…-01-57-07/10/15` (500k/1M K=1/2/4), `gpu-…-01-57-38..58-06` (A/B), `render-…-01-55-35/56-05/56-34` (loop K=1/2/4); gałąź `artifacts`, commit 7bcf685.
