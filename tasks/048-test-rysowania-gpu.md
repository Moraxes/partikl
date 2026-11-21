---
id: 48
title: Test rysowania GPU
summary: 'Render kilku cząstek poza ekranem: nic nie wychodzi poza ich promień'
status: todo
priority: soon
branch: compute-shader-1
relates:
- 28
created: 2026-11-21T13:33:33
worked: 2026-11-21T17:08:28
links:
- kind: commit
  target: eddcdcf
  at: 2026-11-21T17:08:28
  note: 'test(gpu): particles are drawn only near their positions'
---
Cel: błąd z vertex_index (cb39173, linie przez całe okno) przeszedł przez testy, bo sprawdzają tylko compute. Test headless renderuje kilka cząstek do tekstury i sprawdza, że zapalone piksele leżą tylko w promieniu cząstek (jakościowo, bez wartości analogowych). Następny krok: sprawdzić, jak w harnessie zrobić kamerę z render targetem i readback tekstury.
