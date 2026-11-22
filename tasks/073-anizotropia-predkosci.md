---
id: 73
title: Anizotropia prędkości
summary: Nadreprezentacja prędkości wzdłuż osi (+25%) w dynamice; niewyjaśniona
status: doing
priority: later
branch: main
created: 2026-11-21T23:24:00
worked: 2026-11-22T17:19:05
---
Cel: wyjaśnić, czy to artefakt. W teście 1M, macierz cykliczna z odpychaniem wstecz (3 typy, ±500), udział cząstek z prędkością w ±2° od osi: 4,4% przy izotropii 3,6% (±5°: 11,1% vs 8,9%), od kroku 30 do 300. Ten sam wynik bez próbkowania (max_candidates 100000), więc nie próbkowanie; bez ujawnionej zależności od położenia w komórce (spójność dv na siatce = przesunięta). Hipotezy: anizotropia startowego rozkładu prędkości (kwadrat ±125), tarcie/reguły, zaokrąglenia dir. Następny krok: powtórzyć dla losowej macierzy i dla stanu startowego z izotropowymi prędkościami; nie wiadomo, czy w ogóle da się to zobaczyć.
