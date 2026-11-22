---
id: opt-005
title: Próg próbkowania runtime
summary: Próg próbkowania sterowany w runtime (haiku)
status: todo
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:30
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
