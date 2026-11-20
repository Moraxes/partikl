---
id: 13
title: Kształt bez trygonometrii
summary: update_shape bez sin/cos/atan2, raz na klatkę zamiast co krok; ok. −30% kroku przy typowych macierzach
status: todo
priority: now
branch: main
part_of: 12
created: 2026-11-20T20:29:44
---
Cel: usunąć koszt `update_shape` (ok. 170 µs na krok, 34–39% kroku przy macierzach 23 i 6). Następny krok: kwaternion obrotu wprost z wektora prędkości: znormalizowane `(0, 0, v.y, |v| + v.x)` (osobno przypadek `v.x ≈ −|v|`: obrót o π), bez `angle_to` i `from_rotation_z`; zmierzyć `--quick`.

## Dalsze warianty
- System do `Update` (raz na wyrenderowaną klatkę) zamiast `FixedUpdate` (100 Hz), równolegle (`par_iter_mut`).
- Skala bez `log2` albo z tańszym przybliżeniem (`log2` z wykładnika i mantysy).
- Najdalej: prędkość jako dana instancji, obrót i skala w shaderze; system znika z CPU.

## Uwagi
- Benchmark wywołuje `update_shape` w kroku; przy przeniesieniu do `Update` bench musi to odzwierciedlić (osobny pomiar albo wyłączenie z kroku), żeby porównanie było uczciwe.
- Wynik wizualny ma być taki sam (obrót co do błędu zaokrąglenia).
