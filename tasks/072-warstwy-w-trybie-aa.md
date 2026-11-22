---
id: 72
title: Warstwy w trybie AA
summary: Tryb AA (blend) nie zapisuje głębokości; kolejność komórek znów widoczna
status: dropped
priority: later
branch: main
created: 2026-11-21T23:24:00
worked: 2026-11-22T17:26:14
---
Cel: sprawdzić, czy przy analytic_aa (klawisz A, alpha blending bez zapisu głębokości) wraca ślad siatki z zadania o warstwach, i jeśli tak, naprawić (np. losowa kolejność indeksów rysowania albo osobny przebieg opaque dla wnętrza). Następny krok: ten sam test co dla opaque: stan w kolejności id vs komórek, w trybie AA (gallery lub tymczasowy przykład); AA jest domyślnie wyłączone, więc niski priorytet.
