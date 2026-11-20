---
id: 3
title: Gizma zaznaczenia
summary: Jedna gizma zaznaczenia zamiast dwóch ukrytych dzieci na cząstkę
status: todo
priority: soon
branch: main
created: 2026-11-20T18:44:48
---
Każda cząstka ma dwoje ukrytych dzieci (`Selection`, `Highlight` w `src/render.rs`), czyli 3 encje na cząstkę i osobny materiał na każde dziecko. Następny krok: jedna gizma zaznaczenia i pula/instancje podświetleń dla sąsiadów (albo Bevy `Gizmos` rysowane co klatkę).

## Uwagi
- Benchmark `sim` tego nie zmierzy (bez renderingu). Zysk widać w czasie startu, pamięci i klatce renderingu; do pomiaru np. `bevy/trace_tracy` albo czas `init_particles`.
- Testy zaznaczenia (`src/ui/selection/tests.rs`) budują dzieci ręcznie, trzeba je przepisać jakościowo: klik zaznacza, sąsiedzi podświetleni, klik w tło chowa.
