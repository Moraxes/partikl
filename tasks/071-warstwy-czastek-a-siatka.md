---
id: 71
title: Warstwy cząstek a siatka
summary: Przykrywanie cząstek w kolejności komórek dawało prostokątne plamy; naprawione hashem id jako głębokością
status: done
priority: soon
branch: main
created: 2026-11-21T23:24:00
worked: 2026-11-21T23:24:00
---
Cel (zrobione): ślad siatki widoczny w ruchu przy 1M cząstek (rozciągnięte cząstki układające się w prostokątne plamy). Przyczyna: stan leży w kolejności komórek siatki (055), tryb opaque rysuje po kolei, wygrywa ostatnia, więc cząstka z późniejszej komórki zawsze przykrywa wcześniejsze: granice komórek to krawędzie plam; statyczne na ekranie, więc w ruchu widać siatkę. Wykluczone liczbami (1M, macierz cykliczna, krok 3–300): gęstość i prędkość po położeniu w komórce płaskie, spójność dv w komórce = na siatce przesuniętej o pół komórki, próbkowanie sąsiadów ma ten sam wynik co bez niego (max_candidates 100000). Test: ten sam stan rysowany w kolejności id (gładki) i komórek (prostokątne plamy). Poprawka: głębokość z hasha id w render.wesl (z = 0.5·layer), czas klatki bez zmian (500k: 4,4 vs 4,4 ms; 1M: 6,6 vs 6,9 ms). Nie obejmuje trybu analytic_aa (blend bez zapisu głębokości): tam plamy mogą zostać, zob. osobne zadanie.
