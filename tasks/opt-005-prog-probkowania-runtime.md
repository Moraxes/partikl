---
id: opt-005
title: Próg próbkowania runtime
summary: Próg próbkowania sterowany w runtime (haiku)
status: doing
priority: soon
branch: main
part_of: opt-001
relates:
- 66
created: 2026-11-22T01:12:30
worked: 2026-11-22T01:50:00
edited: 2026-11-22T01:49:56
links:
- kind: commit
  target: b734c6d
  at: 2026-11-22T01:49:39
  note: 'chore: opt-005 bench runs (sampling cap) and galleries'
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-05.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-11.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-17.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-22.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-28.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-33.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-47.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/bench/gpu-2026-11-22-01-48-56.parquet
  at: 2026-11-22T01:49:39
- kind: data
  target: .artifacts/gallery/opt-005-cap256
  at: 2026-11-22T01:49:40
- kind: data
  target: .artifacts/gallery/opt-005-cap512
  at: 2026-11-22T01:49:40
- kind: commit
  target: 2a8e0a5
  at: 2026-11-22T01:50:00
  note: 'feat(gpu): runtime sampling cap with key C and --max-candidates'
---
Model: haiku (zasób + klawisz + flaga + HUD według istniejącego wzorca; brak zmian w shaderach).

Cel: max_candidates sterowany w runtime. Dziś stała `MAX_CANDIDATES = 512` w src/gpu/compute/mod.rs, przekazywana co krok w `EngineUniforms::new`, więc zmiana działa od następnego kroku bez przekompilowania shadera. Przy 500k–1M koszt sił ∝ min(kandydaci, próg): próg 256 daje ok. 2× szybszy przebieg sił kosztem większej wariancji (losowe próbkowanie z wagą `every`). Zastępuje zadanie 066 (tam szczegóły pomiarów).

Kroki:
1. Zasób `SamplingCap(u32)`, domyślnie 512, `Resource + Clone + ExtractResource` z `#[extract_app(RenderApp)]` i `ExtractResourcePlugin` (wzór: `GpuParams` i `RenderQuality` w src/gpu/mod.rs). `EngineUniforms::new` (compute/mod.rs) bierze wartość z zasobu zamiast stałej; dostosuj wywołanie w src/gpu/compute/step.rs (`plan`).
2. Klawisz C (sprawdź, że nie koliduje z src/ui/mod.rs i kluczami opcji reguł `//@option ... key=`) przełącza cyklicznie 128 → 256 → 512 → 1024 → 128. Wzór: obsługa klawisza B/A/H/F w src/ui/mod.rs.
3. Linia w HUD z bieżącą wartością (wzór: linia 'sim speed' i wskaźniki jakości renderu w src/ui).
4. Flaga startowa `--max-candidates <N>` w src/args.rs (wzór: pozostałe flagi gpu); ta sama flaga w benchu gpu (benches/gpu/main.rs), żeby dało się mierzyć progi.
5. Testy: UI jakościowy w src/ui/tests.rs (klawisz C zmienia zasób; bez asercji na wartości analogowe) oraz test kroku GPU dla kilku wartości progu w src/gpu/tests (bez NaN, zachowane id i liczba cząstek).
6. Pomiar: bench gpu --sizes 500000,1000000 --max-candidates 256/512; zapisać liczby w tym zadaniu.

Domyślne zostaje 512. Zmiana domyślnego tylko po akceptacji wizualnej użytkownika: wygenerować galerię (cargo bench --bench render -- --gallery <katalog>) dla 256 i 512 i zgłosić ścieżkę. Okna nie uruchamiać.

## Wynik (haiku, 2026-11-22)

Zmiany: zasób `SamplingCap(u32)` (domyślnie 512, `DEFAULT_MAX_CANDIDATES`) z `ExtractResourcePlugin`; `EngineUniforms::new` bierze próg z zasobu, `plan` w step.rs go czyta; klawisz C cyklicznie 128 → 256 → 512 → 1024 (`SamplingCap::next`); linia `candidates N  C` w HUD; flaga `--max-candidates N` (≥1) w args.rs; `Harness::set_max_candidates`; `C` dodane do `RESERVED_KEYS` w options.rs (żadna reguła nie może przejąć klawisza); bench gpu: flaga `--max-candidates` i kolumna `max_candidates` (schemat 1.6, minor); bench render: `--max-candidates` tylko dla `--gallery`. Testy: `ui::tests` (klawisz C, cykl z wartości spoza listy, HUD), `gpu::tests::engine::sampling_cap_keeps_every_particle_finite_and_present` (progi 128–1024: bez NaN, te same id i liczba).

Testy: cargo test 70 zielonych (67 + 3 nowe). cargo clippy --all-targets: lib 9 ostrzeżeń (HEAD: 10), lib test 10 (HEAD: 11); bez nowych. Jedno ostrzeżenie `too_many_arguments` w `handle_keyboard_input` wyciszone atrybutem jak w `plan`.

Pomiar A/B, domyślny próg 512 (ab.sh 3 rundy, --quick 100k, `--seed 1`, przed baabcd6, po baabcd6-dirty): kroki GPU uniform ×1.001, clusters ×1.000, dense ×0.999 (CI 0.998–1.000); siły ×1.001 / ×1.000 / ×1.000. Brak zmiany przy domyślnym progu.

Duże n (bench gpu, --sizes 500000,1000000, --seed 1; czas GPU µs, próg 512 → 256; w nawiasie siły):
- 500k uniform: 3225 → 2078 (×0.64; siły 2618 → 1466, ×0.56)
- 500k clusters: 3202 → 2039 (×0.64; siły 2748 → 1569, ×0.57)
- 500k dense: 3375 → 2315 (×0.69; siły 2947 → 1889, ×0.64)
- 1M uniform: 7094 → 5255 (×0.74; siły 5414 → 3491, ×0.64)
- 1M clusters: 7374 → 4621 (×0.63; siły 6185 → 3525, ×0.57)
- 1M dense: 9842 → 7514 (×0.76; siły 8801 → 6505, ×0.74)
Kandydaci (`candidates_mean`) bez zmian między progami; krok siatki (`siatka`) bez zmiany (ok. 1,0–1,7 ms, jak przed zmianą, nie traktowane jako regresja).

Uwaga do oceny: plan zakładał ok. 2× szybszy przebieg sił przy 256. Zmierzone przyspieszenie sił (512/256) to 1,35–1,8× (najmniej: 1M dense 8801/6505 = 1,35×). Poniżej oczekiwanych ok. 2×, bez regresji; do decyzji użytkownika.

Galeria (LDR/HDR i AA, 1M, seed 1, 800 kroków rozbiegu), do oceny wizualnej przez użytkownika: `.artifacts/gallery/opt-005-cap256` i `.artifacts/gallery/opt-005-cap512`.

Dane: `.artifacts/bench/gpu-2026-11-22-01-48-05..33.parquet` (A/B), `…-01-48-47.parquet` (512), `…-01-48-56.parquet` (256), zapisane w gałęzi `artifacts` (commit b734c6d), podpięte `tq link -k data`.

Domyślny próg pozostaje 512. Zmiana domyślnego tylko po akceptacji wizualnej użytkownika.
