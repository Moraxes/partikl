---
id: 41
title: Bufor RuleParams
summary: SimUniforms na EngineUniforms i RuleParams w osobnym buforze
status: done
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 40
created: 2026-11-21T12:39:03
worked: 2026-11-21T22:28:42
edited: 2026-11-21T22:28:42
---
Cel (poziom A): geometria, step_index i max_candidates zostają w EngineUniforms; promienie, kernel, tarcie itd. idą do RuleParams w osobnym buforze. Następny krok: rozdzielić struktury w Ruście i types.wesl.

Wynik: `SimUniforms` rozdzielone na `EngineUniforms` (56 B, dynamiczny offset na krok; geometria, step_index, max_candidates, interaction_radius) i `RuleParams` (44 B, binding 8, osobny bufor uniformów tworzony raz w `buffers::create` z `RuleParams::default()`). Reguły dostają tylko `RuleParams`. 45 testów przechodzi. Zapis z pliku ron to 042.
