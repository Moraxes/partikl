---
id: 63
title: Wariant PAIR_F16
summary: 'Shader sił: matematyka par w f16 jako wariant pipeline''u; zapasowo f32 bez SHADER_F16'
status: todo
priority: later
branch: compute-shader-1
part_of: 62
created: 2026-11-21T19:19:16
edited: 2026-11-21T19:34:53
---
Cel: wariant shadera PAIR_F16 (flaga w ForcesVariant obok SimOptions i TypeClass), włączany, gdy urządzenie ma SHADER_F16 i opcja (--f16, klawisz; domyślnie wył. do czasu zadania o dokładności). W pętli: delta z f32 konwertowana raz na kandydata (spike S3), test zasięgu, pair_interact w f16 dla wszystkich opcji (spring, resolve, adaptive, average). Stałe z uniformów konwertowane raz na wątek, współczynniki (TypeClass) w vec4<f16>. Akumulacja do porównania: (i) f16 jak w spike, (ii) f32 z konwersją na parę, (iii) f16 w porcjach po 4 i zrzut do f32 (mierzyć czas i błąd). 'enable f16;' po importach w default_rules.wesl i step_driver.wesl; Rust: ForcesVariant.f16, forces_defs, wybór po RenderDevice::features(). Bramka: przebieg sił ≥ 8% szybszy przy 3 typach (A/B ab.sh) i testy zgodności z wyrocznią w poluzowanej tolerancji; inaczej zatrzymać. Następny krok: przenieść pair_interact do f16 pod @if(PAIR_F16) i zmierzyć trzy warianty akumulacji.

Przełączanie: wariant pipeline'u jak N/O/B/A (ForcesVariant.f16): kompilacja leniwa przy pierwszym użyciu, zmiana od następnego kroku, stan (pozycje f32) wspólny, więc bez nieciągłości przy przełączeniu w trakcie działania. Flaga startowa --f16 i klawisz (np. P, 'precision'), linia w HUD ('pair f16 on/off', 'n/a' bez SHADER_F16). Kod reguł pisać raz: alias Real = f16 | f32 pod @if(PAIR_F16) i pair_interact w terminach Real (literały abstrakcyjne WGSL konwertują się same; stałe z uniformów przez Real(...)), zamiast dwóch kopii funkcji. Testy zgodności z wyrocznią parametryzowane precyzją (czas testów GPU ok. ×1,6).

Zmiana (2026-11-21): zamiast klawisza P tylko flaga startowa --f16 (HUD pokazuje 'pair f16 on/off' i 'n/a' bez SHADER_F16). Wariant pipeline'u nadal w ForcesVariant (stały przez cały przebieg), więc nie ma przekompilowania w trakcie; reszta bez zmian.
