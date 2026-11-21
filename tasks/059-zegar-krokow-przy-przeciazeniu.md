---
id: 59
title: Zegar kroków przy przeciążeniu
summary: Gdy krok GPU > budżet czasu, symulacja zwalnia zamiast wydłużać klatkę do 140 ms
status: doing
priority: soon
branch: compute-shader-1
part_of: 54
relates:
- 55
created: 2026-11-21T16:44:13
worked: 2026-11-21T17:49:23
links:
- kind: commit
  target: a0e5260
  at: 2026-11-21T17:49:23
  note: 'feat(gpu): slow the simulation down instead of stretching frames'
---
Cel: przy 1M cząstek (świat 1800×1169, ok. 4700 kandydatów, próbkowanie do 512) krok kosztuje ok. 11 ms, a stały krok to 10 ms, więc fixed timestep goni zaległość (do MAX_STEPS_PER_FRAME = 16 kroków w klatce): klatka ok. 140 ms (7 fps), UI i obraz zacinają się. Lepsza polityka: budżet czasu na klatkę (kroki do limitu pomiarem czasu GPU/klatki), symulacja zwalnia (mniej kroków na sekundę), obraz płynny; wskaźnik 'x realtime' w HUD. Następny krok: zmierzyć krok przy 1M w aplikacji (GPU timestamp) i zaprojektować limit (np. MAX_STEPS_PER_FRAME zależny od czasu poprzedniej klatki).
