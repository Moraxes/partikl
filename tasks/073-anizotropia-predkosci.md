---
id: 73
title: Anizotropia prędkości
summary: Nadreprezentacja prędkości wzdłuż osi (+25%) w dynamice; niewyjaśniona
status: doing
priority: later
branch: main
created: 2026-11-21T23:24:00
worked: 2026-11-22T17:19:05
edited: 2026-11-22T17:20:02
---
Cel: wyjaśnić, czy to artefakt. W teście 1M, macierz cykliczna z odpychaniem wstecz (3 typy, ±500), udział cząstek z prędkością w ±2° od osi: 4,4% przy izotropii 3,6% (±5°: 11,1% vs 8,9%), od kroku 30 do 300. Ten sam wynik bez próbkowania (max_candidates 100000), więc nie próbkowanie; bez ujawnionej zależności od położenia w komórce (spójność dv na siatce = przesunięta). Hipotezy: anizotropia startowego rozkładu prędkości (kwadrat ±125), tarcie/reguły, zaokrąglenia dir. Następny krok: powtórzyć dla losowej macierzy i dla stanu startowego z izotropowymi prędkościami; nie wiadomo, czy w ogóle da się to zobaczyć.

Wynik (2026-11-22): to nie artefakt dynamiki, tylko błędna linia bazowa. Startowy rozkład prędkości to kwadrat ±125, w którym udział w ±2° / ±5° od osi wynosi 3,45% / 8,68% (tan α), a nie izotropowe 4,44% / 11,11%. Pomiar headless (200k cząstek, świat 1500×1000, 3 typy, macierz cykliczna ±500 i losowa): start kwadratowy relaksuje do izotropii już po 100 krokach (4,50% / 11,11%), a start kołowy (izotropowy) daje 4,4% / 11,0–11,2% od kroku 0 i przez 300 kroków. Zaobserwowane 4,4% / 11,1% to więc dokładnie izotropia. Brak zmian w kodzie; test pomiarowy był tymczasowy i usunięty. Ewentualnie: stan startowy mógłby być izotropowy (koło zamiast kwadratu), ale nie ma to wpływu na dynamikę po ok. 100 krokach.
