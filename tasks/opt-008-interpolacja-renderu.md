---
id: opt-008
title: Interpolacja renderu
summary: Interpolacja pozycji w renderze (sonnet)
status: doing
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:31
worked: 2026-11-22T02:08:57
links:
- kind: commit
  target: ea67677
  at: 2026-11-22T02:08:57
  note: 'feat(gpu): render interpolation between ticks'
---
Model: sonnet (przekazanie ułamka taktu do świata renderowania, matematyka w vertex shaderze, ocena wyglądu).

Cel: tani gładki ruch przy fps ≠ 100 Hz i zmiennej liczbie kroków na klatkę. Render rysuje `mix(p.last, p.pos, α)`, gdzie α = `Time<Fixed>::overstep_fraction()` (ułamek taktu od ostatniego kroku). Obraz jest opóźniony o jeden krok, ale ruch równy; przy zwolnionej symulacji (StepBudget) α liczone względem faktycznie wykonanych kroków.

Zakres: materiał (src/gpu/material.rs: `params.w` jest wolne, x/y/z zajęte: promień, próg prędkości, AA), aktualizacja wartości co klatkę w systemie świata głównego (wzór: apply_render_quality w src/gpu/mod.rs), render.wesl `vertex`: pozycja z interpolacji; elipsa/obrót z prędkości (pos − last) bez zmian. Uwaga na torus: cząstka, która w tym kroku się zawinęła, ma `last` po drugiej stronie świata (patrz `wrap` w step_driver.wesl, przesuwa też `last`, więc `pos − last` jest poprawne); sprawdzić, czy interpolacja nie rysuje pasa przez cały ekran.

Weryfikacja: headless: bench render --modes loop (koszt render bez zmian, ±2%), galeria --gallery do PNG. Ocena płynności wymaga ruchu: zapytać użytkownika o zgodę na okno albo zostawić do jego oceny; bez zgody okna nie uruchamiać.
