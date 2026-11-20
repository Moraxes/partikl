---
id: 21
title: Usypianie spokojnych cząstek
summary: Cząstki w stabilnych skupiskach przeliczają siły co k kroków
status: doing
priority: later
branch: main
part_of: 12
created: 2026-11-20T20:29:45
worked: 2026-11-20T21:38:10
---
Cel: mniej pracy przy statycznych skupiskach (szybkie macierze). Następny krok: zmierzyć, jaki odsetek cząstek przy macierzy 4 po offsecie 4096 ma prędkość poniżej `VELOCITY_THRESHOLD` i stały zbiór sąsiadów; jeśli duży, przeliczać ich siły co k kroków (siła z poprzedniego przeliczenia).

## Ryzyka
- Budzenie: cząstka uśpiona musi reagować na nadlatujące cząstki (np. przelicz, gdy w jej komórkach zmieniła się liczba cząstek).
- Nierówna praca między krokami (co k-ty krok droższy) może dawać szarpanie animacji.
