---
id: 4
title: Dzielenie vs mnożenie
summary: Dzielenie i sqrt na M5 tanie; zostaje wariant z 3 dzieleniami
status: done
priority: soon
branch: main
relates:
- 10
created: 2026-11-20T18:45:00
worked: 2026-11-20T18:45:00
links:
- kind: commit
  target: ffe6723
  at: 2026-11-20T18:45:00
  note: rewizja pomiaru
- kind: commit
  target: 1c07039
  at: 2026-11-20T18:45:00
  note: poprawka splat, przez którą pierwszy przebieg był nieważny
---
Wynik: bez zmian w kodzie. B (1 dzielenie) daje ok. 2% `compute_forces` tylko przy małym n i nic w kroku ani w skupiskach; przybliżony `1/√x` (C1, C2) jest wolniejszy, a C1 też mniej dokładny. Zostaje A.

## Wynik
Na `ffe6723`. Czas `compute_forces` na kandydata względem A, średnia geometryczna po macierzach z 95% CI; błąd względny sił względem referencji f64 (mediana / max), n=3000, 5 stanów.

| wariant | `compute_forces`/kandydat vs A | krok vs A | błąd (mediana / max) |
|---|---|---|---|
| A: `dx/d`, `dy/d`, `/KERNEL_WIDTH` (3 dzielenia) | 1 | 1 | 1,7e-7 / 8,6e-4 |
| B: `s = siła/d`, `s·dx`, `s·dy` (1 dzielenie) | 0,978 [0,965; 0,991]; n=9000 0,993 [0,981; 1,004] | 0,994 [0,979; 1,009] | 1,8e-7 / 8,5e-4 |
| C1: `vrsqrteq_f32` + 1 krok Newtona | 1,034 [1,021; 1,046] | 1,017 | 1,8e-5 / 3,1e-3 |
| C2: jak C1, 2 kroki Newtona | 1,044 [1,029; 1,059] | 1,039 | 2,2e-7 / 9,0e-4 |

## Metoda
Warianty `Kernel::add` w `src/sim/forces.rs` przełączane stałą, pełny bench `sim` dla każdego tuż po sobie (przebiegi nie zachowane).

## Uwagi
- Pierwszy przebieg tego porównania (na `ec4d863`) jest nieważny: pętla stała wtedy na `memcpy` stałych z `Lanes::splat` (naprawione w `1c07039`).
- W `wide` 1.7.1 `recip` i `recip_sqrt` na NEON to dokładne dzielenie, nie przybliżenie.
