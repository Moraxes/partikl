---
id: 63
title: Wariant PAIR_F16
summary: 'Shader sił: matematyka par w f16 jako wariant pipeline''u; zapasowo f32 bez SHADER_F16'
status: todo
priority: soon
branch: compute-shader-1
part_of: 62
created: 2026-11-21T19:19:16
---
Cel: wariant shadera PAIR_F16 (flaga w ForcesVariant obok SimOptions i TypeClass), włączany, gdy urządzenie ma SHADER_F16 i opcja (--f16, klawisz; domyślnie wył. do czasu zadania o dokładności). W pętli: delta z f32 konwertowana raz na kandydata (spike S3), test zasięgu, pair_interact w f16 dla wszystkich opcji (spring, resolve, adaptive, average). Stałe z uniformów konwertowane raz na wątek, współczynniki (TypeClass) w vec4<f16>. Akumulacja do porównania: (i) f16 jak w spike, (ii) f32 z konwersją na parę, (iii) f16 w porcjach po 4 i zrzut do f32 (mierzyć czas i błąd). 'enable f16;' po importach w default_rules.wesl i step_driver.wesl; Rust: ForcesVariant.f16, forces_defs, wybór po RenderDevice::features(). Bramka: przebieg sił ≥ 8% szybszy przy 3 typach (A/B ab.sh) i testy zgodności z wyrocznią w poluzowanej tolerancji; inaczej zatrzymać. Następny krok: przenieść pair_interact do f16 pod @if(PAIR_F16) i zmierzyć trzy warianty akumulacji.
