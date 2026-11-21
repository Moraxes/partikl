---
id: 48
title: Test rysowania GPU
summary: 'Render kilku cząstek poza ekranem: nic nie wychodzi poza ich promień'
status: done
priority: soon
branch: compute-shader-1
relates:
- 28
created: 2026-11-21T13:33:33
worked: 2026-11-21T17:08:28
edited: 2026-11-21T17:08:28
links:
- kind: commit
  target: eddcdcf
  at: 2026-11-21T17:08:28
  note: 'test(gpu): particles are drawn only near their positions'
---
Cel: błąd z vertex_index (cb39173, linie przez całe okno) przeszedł przez testy, bo sprawdzają tylko compute. Test headless renderuje kilka cząstek do tekstury i sprawdza, że zapalone piksele leżą tylko w promieniu cząstek (jakościowo, bez wartości analogowych). Wynik: test particles_are_drawn_only_near_their_positions (Harness::with_render + target_pixels): każdy jasny piksel w promieniu 3,5 jednostki od cząstki, każda cząstka ma jasne piksele; sprawdzone, że rozciągnięcie cząstek 8× oblewa test. Tło ma dithering ±1/255, więc próg jasności 40.
