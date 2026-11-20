---
id: 27
title: Tarcie w kroku całkowania
summary: Tarcie z prędkości po kroku i dokładnym zanikiem kwadratowym zamiast prędkości z poprzedniego kroku
status: doing
priority: now
branch: main
created: 2026-11-20T23:24:58
worked: 2026-11-20T23:24:58
---
Cel: tarcie liczone z prędkości, którą cząstka miałaby po tym kroku, a nie z różnicy pozycji z poprzedniego kroku, i z dokładnym rozwiązaniem samego tarcia kwadratowego zamiast jawnego odjęcia. Następny krok: zmiana w `compute_friction` (bez nowych systemów), testy, `--quick` przed/po, sonda n=100000.

## Teraz
`a −= c·|v|·v` z `v = (x_t − x_{t−1})/dt`: przemieszczenie w kroku mnożone przez `1 − c·|d|` (d = x_t − x_{t−1}), siły z tego kroku nietłumione aż do następnego. Dla `c·|d| > 1` (|v| > 10000 j/s) przemieszczenie zmienia znak, dla > 2 rośnie: tarcie samo wzbudza drgania.

## Zmiana
- Przemieszczenie po kroku bez tarcia: `d̃ = d + a·dt²` (siły już policzone).
- Samo tarcie kwadratowe `v' = −c|v|v` ma rozwiązanie `|v(t)| = |v₀|/(1 + c|v₀|t)`, więc po kroku `d̃ / (1 + c·|d̃|)`: zawsze w (0, 1], nigdy nie odwraca kierunku, prędkość graniczna pod stałą siłą ta sama co w ciągłym modelu (√(a/c)).
- W `compute_friction` jako przyspieszenie zastępcze `(d̃·s − d)/dt²`, s = 1/(1 + c|d̃|): `integrate` bez zmian, systemy i bench bez zmian.
- Próg spoczynku bez zmian (prędkość poniżej `VELOCITY_THRESHOLD` zerowana).

## Pomiar
- `--quick`: koszt (trajektorie się zmienią: porównanie per kandydat).
- Sonda n=100000, `spec-2026-11-20-21-48-03.ron`: tłumione w tym samym kroku kopnięcia od odpychania mogą poprawić stabilność gwiazd.

## Testy
- Samo tarcie: zanik zgodny z 1/(1 + c|v₀|t).
- Bardzo duża prędkość: kierunek bez zmian, prędkość maleje.
- Stała siła: prędkość graniczna √(a/c).
