---
id: 35
title: Refaktor GPU
summary: 'Czytelny silnik GPU i reguły jako dane (A, B, C): granica silnik/model we właściwym miejscu'
status: todo
priority: soon
branch: main
relates:
- 28
created: 2026-11-21T12:39:02
---
Cel: kod GPU zrozumiały dla człowieka i szybka iteracja nad regułami (poziomy A, B, C z plan_rozwoju_partikl_na_gpu.md). Dziś parametry fizyki siedzą w SimUniforms obok geometrii, crowding (pole reguł) w silnikowym Particle, a coeff i weight podaje regułom silnik. Następny krok: podzadania w kolejności blokad (tq tree).
