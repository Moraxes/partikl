---
id: 13
title: Kształt bez trygonometrii
summary: 'Obrót z wektora prędkości: update_shape ×0,15–0,19, krok ×0,72–0,87; równoległość odrzucona'
status: doing
priority: now
branch: main
part_of: 12
created: 2026-11-20T20:29:44
worked: 2026-11-20T20:43:58
edited: 2026-11-20T20:45:52
links:
- kind: commit
  target: 975035d
  at: 2026-11-20T20:43:58
  note: 'perf: compute particle rotation from velocity without trigonometry'
---
Wynik: obrót bez trygonometrii (`975035d`): `update_shape` ×0,15–0,19, krok ×0,76 / ×0,72 / ×0,87 (macierze 23 / 6 / 4), te same stany. Zrównoleglenie reszty odrzucone (wolniejsze). Przeniesienie do `Update` i shader niezrobione: zostało ok. 27 µs (ok. 8% kroku przy typowej macierzy), a przy ekranie 120 Hz `Update` byłby częściej niż krok fizyki (100 Hz).

## Wyniki (`--quick`, 3 rundy na przemian, po/przed [95% CI])
| wariant | macierz 23 | macierz 6 | macierz 4 |
|---|---|---|---|
| obrót z wektora prędkości (`975035d`): krok | 0,76 [0,75; 0,77] | 0,72 [0,71; 0,72] | 0,87 [0,87; 0,88] |
| to samo: `update_shape` | 0,19 | 0,16 | 0,15 |
| `par_iter_mut`, partie ≥ 1024, 1 na wątek: `update_shape` | 1,16 [1,13; 1,21] | 1,27 [1,26; 1,28] | 1,10 [1,08; 1,12] |
| to samo: krok | 0,98 [0,97; 1,00] | 1,01 [1,01; 1,02] | 1,01 [1,00; 1,01] |

Dane: etap w `.artifacts/perf-stages.json` (`quick`), raport `.artifacts/reports/perf-report.md`. Przebiegi wariantu równoległego (odrzucony, bez commita): `sim-2026-11-20-20-44-49` → `-20-44-57`, `-20-45-11` → `-20-45-04`, `-20-45-18` → `-20-45-25` w `.artifacts/bench/`.

## Uwagi
- Zakres pracy w puli Bevy kosztuje ok. 30 µs na wywołanie (`update_shape` po zrównolegleniu 30–32 µs przy ok. 3 ns pracy na cząstkę); dotyczy też planów dla `compute_forces` (zadanie 015).
- Przy macierzy 23 `compute_forces` po zmianie wyszło ×1,13 [1,11; 1,15] przy niezmienionym kodzie sił (rundy 1,18 / 1,16 / 1,03); w następnym porównaniu (te same binaria sił) ×0,96. Prawdopodobnie wątki puli głębiej usypiają, gdy seryjna część kroku jest krótsza; do obserwacji.

## Pierwotny plan
Cel: usunąć koszt `update_shape` (ok. 170 µs na krok, 34–39% kroku przy macierzach 23 i 6). Następny krok: kwaternion obrotu wprost z wektora prędkości: znormalizowane `(0, 0, v.y, |v| + v.x)` (osobno przypadek `v.x ≈ −|v|`: obrót o π), bez `angle_to` i `from_rotation_z`; zmierzyć `--quick`.

## Dalsze warianty
- System do `Update` (raz na wyrenderowaną klatkę) zamiast `FixedUpdate` (100 Hz), równolegle (`par_iter_mut`).
- Skala bez `log2` albo z tańszym przybliżeniem (`log2` z wykładnika i mantysy).
- Najdalej: prędkość jako dana instancji, obrót i skala w shaderze; system znika z CPU.

## Uwagi
- Benchmark wywołuje `update_shape` w kroku; przy przeniesieniu do `Update` bench musi to odzwierciedlić (osobny pomiar albo wyłączenie z kroku), żeby porównanie było uczciwe.
- Wynik wizualny ma być taki sam (obrót co do błędu zaokrąglenia).
