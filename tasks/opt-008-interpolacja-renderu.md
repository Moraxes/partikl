---
id: opt-008
title: Interpolacja renderu
summary: Interpolacja pozycji w renderze (sonnet)
status: done
priority: soon
branch: main
part_of: opt-001
created: 2026-11-22T01:12:31
worked: 2026-11-22T02:09:08
edited: 2026-11-22T02:09:08
links:
- kind: commit
  target: ea67677
  at: 2026-11-22T02:08:57
  note: 'feat(gpu): render interpolation between ticks'
- kind: commit
  target: 13de01d
  at: 2026-11-22T02:09:00
  note: 'chore: opt-008 render bench runs and gallery'
- kind: data
  target: .artifacts/bench/render-2026-11-22-02-05-45.parquet
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: baseline HEAD, loop 500k/1M
- kind: data
  target: .artifacts/bench/render-2026-11-22-02-06-53.parquet
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: baseline HEAD
- kind: data
  target: .artifacts/bench/render-2026-11-22-02-07-08.parquet
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: baseline HEAD
- kind: data
  target: .artifacts/bench/render-2026-11-22-02-06-11.parquet
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: after opt-008
- kind: data
  target: .artifacts/bench/render-2026-11-22-02-06-26.parquet
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: after opt-008
- kind: data
  target: .artifacts/gallery/opt-008
  at: 2026-11-22T02:09:08
  rev: 13de01d
  note: galeria PNG
- kind: commit
  target: ea67677
  at: 2026-11-22T02:09:08
---
Model: sonnet (przekazanie ułamka taktu do świata renderowania, matematyka w vertex shaderze, ocena wyglądu).

Cel: tani gładki ruch przy fps ≠ 100 Hz i zmiennej liczbie kroków na klatkę. Render rysuje `mix(p.last, p.pos, α)`, gdzie α = `Time<Fixed>::overstep_fraction()` (ułamek taktu od ostatniego kroku). Obraz jest opóźniony o jeden krok, ale ruch równy; przy zwolnionej symulacji (StepBudget) α liczone względem faktycznie wykonanych kroków.

Zakres: materiał (src/gpu/material.rs: `params.w` jest wolne, x/y/z zajęte: promień, próg prędkości, AA), aktualizacja wartości co klatkę w systemie świata głównego (wzór: apply_render_quality w src/gpu/mod.rs), render.wesl `vertex`: pozycja z interpolacji; elipsa/obrót z prędkości (pos − last) bez zmian. Uwaga na torus: cząstka, która w tym kroku się zawinęła, ma `last` po drugiej stronie świata (patrz `wrap` w step_driver.wesl, przesuwa też `last`, więc `pos − last` jest poprawne); sprawdzić, czy interpolacja nie rysuje pasa przez cały ekran.

Weryfikacja: headless: bench render --modes loop (koszt render bez zmian, ±2%), galeria --gallery do PNG. Ocena płynności wymaga ruchu: zapytać użytkownika o zgodę na okno albo zostawić do jego oceny; bez zgody okna nie uruchamiać.

## Wynik (commit ea67677)

Implementacja: `params.w` materiału = cofnięcie pozycji w podkrokach `(1 − α)·K`; vertex shader rysuje `pos − w·(pos − last)` (dla K=1 to `mix(last, pos, α)`, dla K>1 obejmuje cały takt, bo `pos − last` to jeden podkrok). System `update_interpolation` (Update, src/gpu/mod.rs) liczy α z `Time<Fixed>::overstep_fraction()`; gdy w klatce jakiś takt przepadł (StepBudget, `dropped()`), w = 0 (rysunek ostatniego stanu, bez opóźnienia). Zasób `RenderInterpolation` (wyłączony w harnessie headless bez real_time, gdzie zegar stały nie biegnie). Elipsa/obrót z `pos − last` bez zmian.

Torus: wrap przesuwa też `last`, więc `pos − last` jest ciągłe; rysowana pozycja może wyjść poza świat o co najwyżej kilka długości kroku, bez pasa przez ekran (logika shadera, bez osobnego testu na zawinięciu).

Testy (src/gpu/tests/render.rs): środek ciężkości narysowanej cząstki dla α = 0, 0,5, 1 (K=1) i α=0 przy K=2; `interpolation_lag`. cargo test: 80 ok. cargo clippy --all-targets: bez nowych ostrzeżeń (12 przed i po).

Bench render --modes loop --sizes 500000,1000000, klatka ms (baseline HEAD x3 / po x2): 500k 3,03 / 3,34 / 3,14 vs 3,12 / 2,96; 1M 11,25 / 11,59 / 11,57 vs 11,71 / 10,89. Średnie 3,17 → 3,04 (500k) i 11,47 → 11,30 (1M): różnica w szumie, bez regresji >2%. Surowe dane w .artifacts/bench (linki data).

Galeria: .artifacts/gallery/opt-008 (PNG, offscreen bez interpolacji: nie pokazuje ruchu). Ocena płynności ruchu zostaje użytkownikowi (okna nie uruchamiano).
