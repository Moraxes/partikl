---
id: 41
title: Bufor RuleParams
summary: SimUniforms na EngineUniforms i RuleParams w osobnym buforze
status: todo
priority: soon
branch: compute-shader-1
part_of: 35
blocked_by:
- 40
created: 2026-11-21T12:39:03
---
Cel (poziom A): geometria, step_index i max_candidates zostają w EngineUniforms; promienie, kernel, tarcie itd. idą do RuleParams w osobnym buforze. Następny krok: rozdzielić struktury w Ruście i types.wesl.
