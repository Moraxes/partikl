---
id: 52
title: Stany syntetyczne benchu
summary: Jednostajny, skupiony i gęsty stan generowany wprost, niezależny od fizyki
status: done
priority: soon
branch: compute-shader-1
part_of: 50
blocked_by:
- 51
created: 2026-11-21T15:00:57
worked: 2026-11-21T15:08:31
edited: 2026-11-21T15:08:31
links:
- kind: commit
  target: 1504e2b
  at: 2026-11-21T15:08:26
  note: 'feat(bench): synthetic scenes with fixed density for the gpu bench'
---
Cel: mierzyć silnik w kontrolowanej gęstości niezależnie od reguł (M3 zmienia trajektorie): stany jednostajny, średnio skupiony i bardzo gęsty (zbite plamy, ponad próg próbkowania 512 kandydatów), generowane z ziarna. Kolumna density w wynikach; --quick: 100k × 3 gęstości w wariancie domyślnym aplikacji (average). Wynik (1504e2b): sceny uniform / clusters (¼ świata, ×4) / dense (1/16, ×16) w benches/gpu/scenes.rs plus evolved (dawny pomiar po rozbiegu). Gęstość względna, bo stała lokalna nie skalowała się z n (przy 100k jednostajny stan ma już 272 kandydatów). Stan resetowany przed każdym pomiarem (Harness::set_state), więc A/B liczy te same stany. Kolumny scene i candidates_mean (format 1.2). Quick (100k, spring+average; kandydaci / µs na krok): uniform 272 / 746, clusters 1221 / 1033, dense 2957 / 1105; evolved (512 kroków) ok. 1270 kandydatów. Dwa przebiegi zgodne do 0,5%. Gęsty stan niewiele droższy od skupisk: próbkowanie ogranicza pętlę sił.
